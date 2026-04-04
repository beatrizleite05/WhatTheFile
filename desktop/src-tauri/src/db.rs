use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use crate::errors::AppError;

// ── helpers ──────────────────────────────────────────────────────────────────

pub(crate) fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ── types ─────────────────────────────────────────────────────────────────────

pub struct Root {
    pub id: i64,
    pub path: String,
    pub label: String,
    pub active: bool,
    pub created_at: i64,
    pub last_indexed_at: Option<i64>,
}

pub struct FileRecord {
    pub id: i64,
    pub root_id: i64,
    pub rel_path: String,
    pub filename: String,
    pub fingerprint: String,
    pub model_version: String,
    pub mtime_ns: i64,
    pub size_bytes: i64,
    pub index_marker: i64,
}

#[derive(Default, Clone)]
pub struct JobCounts {
    pub files_total: i64,
    pub files_done: i64,
    pub files_added: i64,
    pub files_updated: i64,
    pub files_moved: i64,
    pub files_deleted: i64,
    pub error_count: i64,
}

// ── DDL ───────────────────────────────────────────────────────────────────────

const MIGRATION_001: &str = r#"
CREATE TABLE IF NOT EXISTS roots (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  path            TEXT    NOT NULL UNIQUE,
  label           TEXT    NOT NULL DEFAULT '' COLLATE NOCASE,
  active          INTEGER NOT NULL DEFAULT 1,
  created_at      INTEGER NOT NULL,
  last_indexed_at INTEGER
);

CREATE TABLE IF NOT EXISTS files (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  root_id         INTEGER NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
  rel_path        TEXT    NOT NULL,
  filename        TEXT    NOT NULL,
  media_type      TEXT    NOT NULL,
  size_bytes      INTEGER NOT NULL,
  mtime_ns        INTEGER NOT NULL,
  fingerprint     TEXT    NOT NULL,
  model_version   TEXT    NOT NULL DEFAULT '',
  confidence      REAL    NOT NULL DEFAULT 1.0,
  extracted_text  TEXT    NOT NULL DEFAULT '',
  structured_meta TEXT,
  lang_hint       TEXT    NOT NULL DEFAULT 'unknown',
  index_marker    INTEGER NOT NULL DEFAULT 0,
  indexed_at      INTEGER NOT NULL,
  deleted_at      INTEGER,
  UNIQUE(root_id, rel_path)
);

CREATE INDEX IF NOT EXISTS idx_files_root         ON files(root_id);
CREATE INDEX IF NOT EXISTS idx_files_fingerprint  ON files(fingerprint);
CREATE INDEX IF NOT EXISTS idx_files_media_type   ON files(media_type);
CREATE INDEX IF NOT EXISTS idx_files_index_marker ON files(index_marker);
CREATE INDEX IF NOT EXISTS idx_files_deleted_at   ON files(deleted_at);
CREATE INDEX IF NOT EXISTS idx_files_filename     ON files(filename);

CREATE TABLE IF NOT EXISTS chunks (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  file_id     INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
  chunk_index INTEGER NOT NULL,
  text        TEXT    NOT NULL,
  UNIQUE(file_id, chunk_index)
);

CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
  filename,
  rel_path,
  extracted_text,
  content='files',
  content_rowid='id',
  tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS files_fts_insert AFTER INSERT ON files BEGIN
  INSERT INTO files_fts(rowid, filename, rel_path, extracted_text)
  VALUES (new.id, new.filename, new.rel_path, new.extracted_text);
END;

CREATE TRIGGER IF NOT EXISTS files_fts_delete AFTER DELETE ON files BEGIN
  INSERT INTO files_fts(files_fts, rowid, filename, rel_path, extracted_text)
  VALUES ('delete', old.id, old.filename, old.rel_path, old.extracted_text);
END;

CREATE TRIGGER IF NOT EXISTS files_fts_update AFTER UPDATE ON files BEGIN
  INSERT INTO files_fts(files_fts, rowid, filename, rel_path, extracted_text)
  VALUES ('delete', old.id, old.filename, old.rel_path, old.extracted_text);
  INSERT INTO files_fts(rowid, filename, rel_path, extracted_text)
  VALUES (new.id, new.filename, new.rel_path, new.extracted_text);
END;

CREATE TABLE IF NOT EXISTS index_jobs (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  root_id       INTEGER REFERENCES roots(id) ON DELETE CASCADE,
  status        TEXT    NOT NULL,
  phase         TEXT,
  index_marker  INTEGER NOT NULL,
  files_total   INTEGER NOT NULL DEFAULT 0,
  files_done    INTEGER NOT NULL DEFAULT 0,
  files_added   INTEGER NOT NULL DEFAULT 0,
  files_updated INTEGER NOT NULL DEFAULT 0,
  files_moved   INTEGER NOT NULL DEFAULT 0,
  files_deleted INTEGER NOT NULL DEFAULT 0,
  error_count   INTEGER NOT NULL DEFAULT 0,
  cursor_path   TEXT,
  started_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL,
  completed_at  INTEGER
);

CREATE INDEX IF NOT EXISTS idx_index_jobs_status     ON index_jobs(status);
CREATE INDEX IF NOT EXISTS idx_index_jobs_updated_at ON index_jobs(updated_at);

CREATE TABLE IF NOT EXISTS activity_log (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  event_type TEXT    NOT NULL,
  root_id    INTEGER REFERENCES roots(id),
  file_id    INTEGER REFERENCES files(id),
  job_id     INTEGER REFERENCES index_jobs(id),
  detail     TEXT,
  created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_activity_log_created ON activity_log(created_at DESC);
"#;

pub(crate) fn run_migrations(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);",
    )?;
    let version: Option<i64> = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |r| r.get(0))
        .optional()?;
    if version.is_none() {
        conn.execute_batch(MIGRATION_001)?;
        conn.execute("INSERT INTO schema_version VALUES (1)", [])?;
    }
    Ok(())
}

pub fn open_and_migrate(path: &Path) -> Result<Connection, AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "PRAGMA busy_timeout = 5000;
         PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA temp_store = MEMORY;",
    )?;
    run_migrations(&conn)?;
    Ok(conn)
}

pub fn recover_interrupted_jobs(conn: &Connection) -> Result<(), AppError> {
    let now = unix_now();
    conn.execute(
        "UPDATE index_jobs SET status = 'interrupted', updated_at = ?1 WHERE status = 'running'",
        params![now],
    )?;
    Ok(())
}

// ── stubs for Tasks 6–8 ───────────────────────────────────────────────────────

pub fn insert_root(conn: &Connection, path: &str) -> Result<Root, AppError> {
    let label = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .to_string();
    let now = unix_now();
    conn.execute(
        "INSERT INTO roots (path, label, active, created_at) VALUES (?1, ?2, 1, ?3)",
        params![path, label, now],
    )?;
    Ok(Root {
        id: conn.last_insert_rowid(),
        path: path.to_string(),
        label,
        active: true,
        created_at: now,
        last_indexed_at: None,
    })
}

pub fn find_root_by_path(conn: &Connection, path: &str) -> Result<Option<Root>, AppError> {
    conn.query_row(
        "SELECT id, path, label, active, created_at, last_indexed_at FROM roots WHERE path = ?1",
        params![path],
        |row| {
            Ok(Root {
                id: row.get(0)?,
                path: row.get(1)?,
                label: row.get(2)?,
                active: row.get::<_, i64>(3).map(|v| v != 0)?,
                created_at: row.get(4)?,
                last_indexed_at: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

pub fn find_root_by_id(conn: &Connection, id: i64) -> Result<Option<Root>, AppError> {
    conn.query_row(
        "SELECT id, path, label, active, created_at, last_indexed_at FROM roots WHERE id = ?1",
        params![id],
        |row| {
            Ok(Root {
                id: row.get(0)?,
                path: row.get(1)?,
                label: row.get(2)?,
                active: row.get::<_, i64>(3).map(|v| v != 0)?,
                created_at: row.get(4)?,
                last_indexed_at: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

pub fn update_root_last_indexed(conn: &Connection, root_id: i64, ts: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE roots SET last_indexed_at = ?1 WHERE id = ?2",
        params![ts, root_id],
    )?;
    Ok(())
}
pub fn find_file_by_path(
    conn: &Connection,
    root_id: i64,
    rel_path: &str,
) -> Result<Option<FileRecord>, AppError> {
    conn.query_row(
        "SELECT id, root_id, rel_path, filename, fingerprint, model_version, mtime_ns, size_bytes, index_marker
         FROM files WHERE root_id = ?1 AND rel_path = ?2 AND deleted_at IS NULL",
        params![root_id, rel_path],
        |row| Ok(FileRecord {
            id: row.get(0)?,
            root_id: row.get(1)?,
            rel_path: row.get(2)?,
            filename: row.get(3)?,
            fingerprint: row.get(4)?,
            model_version: row.get(5)?,
            mtime_ns: row.get(6)?,
            size_bytes: row.get(7)?,
            index_marker: row.get(8)?,
        }),
    )
    .optional()
    .map_err(Into::into)
}

pub fn find_file_by_fingerprint(
    conn: &Connection,
    root_id: i64,
    fingerprint: &str,
) -> Result<Option<FileRecord>, AppError> {
    conn.query_row(
        "SELECT id, root_id, rel_path, filename, fingerprint, model_version, mtime_ns, size_bytes, index_marker
         FROM files WHERE root_id = ?1 AND fingerprint = ?2 AND deleted_at IS NULL LIMIT 1",
        params![root_id, fingerprint],
        |row| Ok(FileRecord {
            id: row.get(0)?,
            root_id: row.get(1)?,
            rel_path: row.get(2)?,
            filename: row.get(3)?,
            fingerprint: row.get(4)?,
            model_version: row.get(5)?,
            mtime_ns: row.get(6)?,
            size_bytes: row.get(7)?,
            index_marker: row.get(8)?,
        }),
    )
    .optional()
    .map_err(Into::into)
}

pub fn upsert_file_metadata(
    conn: &Connection,
    root_id: i64,
    rel_path: &str,
    filename: &str,
    media_type: &str,
    size_bytes: i64,
    mtime_ns: i64,
    fingerprint: &str,
    model_version: &str,
    index_marker: i64,
    indexed_at: i64,
) -> Result<i64, AppError> {
    let id: i64 = conn.query_row(
        "INSERT INTO files
           (root_id, rel_path, filename, media_type, size_bytes, mtime_ns,
            fingerprint, model_version, index_marker, indexed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(root_id, rel_path) DO UPDATE SET
           filename      = excluded.filename,
           media_type    = excluded.media_type,
           size_bytes    = excluded.size_bytes,
           mtime_ns      = excluded.mtime_ns,
           fingerprint   = excluded.fingerprint,
           model_version = excluded.model_version,
           index_marker  = excluded.index_marker,
           indexed_at    = excluded.indexed_at,
           deleted_at    = NULL
         RETURNING id",
        params![root_id, rel_path, filename, media_type, size_bytes, mtime_ns,
                fingerprint, model_version, index_marker, indexed_at],
        |row| row.get(0),
    )?;
    Ok(id)
}

pub fn stamp_index_marker(
    conn: &Connection,
    file_id: i64,
    marker: i64,
    now: i64,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE files SET index_marker = ?1, indexed_at = ?2 WHERE id = ?3",
        params![marker, now, file_id],
    )?;
    Ok(())
}

pub fn move_file(
    conn: &Connection,
    file_id: i64,
    new_rel_path: &str,
    new_filename: &str,
    new_mtime_ns: i64,
    marker: i64,
    now: i64,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE files SET rel_path = ?1, filename = ?2, mtime_ns = ?3,
                          index_marker = ?4, indexed_at = ?5, deleted_at = NULL
         WHERE id = ?6",
        params![new_rel_path, new_filename, new_mtime_ns, marker, now, file_id],
    )?;
    Ok(())
}

pub fn sweep_deleted_files(
    conn: &Connection,
    root_id: i64,
    marker: i64,
    deleted_at: i64,
) -> Result<i64, AppError> {
    let count = conn.execute(
        "UPDATE files SET deleted_at = ?1
         WHERE root_id = ?2 AND index_marker != ?3 AND deleted_at IS NULL",
        params![deleted_at, root_id, marker],
    )?;
    Ok(count as i64)
}
pub fn insert_job(
    conn: &Connection,
    root_id: i64,
    marker: i64,
    now: i64,
) -> Result<i64, AppError> {
    conn.execute(
        "INSERT INTO index_jobs (root_id, status, phase, index_marker, started_at, updated_at)
         VALUES (?1, 'running', 'discovering', ?2, ?3, ?3)",
        params![root_id, marker, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_job_phase(
    conn: &Connection,
    job_id: i64,
    phase: &str,
    now: i64,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE index_jobs SET phase = ?1, updated_at = ?2 WHERE id = ?3",
        params![phase, now, job_id],
    )?;
    Ok(())
}

pub fn update_job_counts(
    conn: &Connection,
    job_id: i64,
    counts: &JobCounts,
    now: i64,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE index_jobs SET
           files_total = ?1, files_done = ?2, files_added = ?3,
           files_updated = ?4, files_moved = ?5, files_deleted = ?6,
           error_count = ?7, updated_at = ?8
         WHERE id = ?9",
        params![
            counts.files_total, counts.files_done, counts.files_added,
            counts.files_updated, counts.files_moved, counts.files_deleted,
            counts.error_count, now, job_id
        ],
    )?;
    Ok(())
}

pub fn complete_job(
    conn: &Connection,
    job_id: i64,
    counts: &JobCounts,
    now: i64,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE index_jobs SET
           status = 'completed', phase = NULL,
           files_total = ?1, files_done = ?2, files_added = ?3,
           files_updated = ?4, files_moved = ?5, files_deleted = ?6,
           error_count = ?7, updated_at = ?8, completed_at = ?8
         WHERE id = ?9",
        params![
            counts.files_total, counts.files_done, counts.files_added,
            counts.files_updated, counts.files_moved, counts.files_deleted,
            counts.error_count, now, job_id
        ],
    )?;
    Ok(())
}

pub fn log_activity(
    conn: &Connection,
    event_type: &str,
    root_id: Option<i64>,
    file_id: Option<i64>,
    job_id: Option<i64>,
    detail: Option<&str>,
    created_at: i64,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO activity_log (event_type, root_id, file_id, job_id, detail, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![event_type, root_id, file_id, job_id, detail, created_at],
    )?;
    Ok(())
}

// ── Phase C: extraction content helpers ──────────────────────────────────────

/// Update the extracted content fields for a file that has been processed by
/// the extraction pipeline.  Called after `extractor::extract` succeeds.
pub fn update_file_content(
    conn: &Connection,
    file_id: i64,
    extracted_text: &str,
    confidence: f32,
    lang_hint: &str,
    model_version: &str,
    now: i64,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE files SET extracted_text = ?1, confidence = ?2, lang_hint = ?3,
                          model_version = ?4, indexed_at = ?5
         WHERE id = ?6",
        params![extracted_text, confidence, lang_hint, model_version, now, file_id],
    )?;
    Ok(())
}

/// Atomically replace all chunks for `file_id` with the provided slice.
/// Runs DELETE + INSERT inside a savepoint so the table is never partially
/// updated if an error occurs mid-way.
pub fn replace_chunks(
    conn: &Connection,
    file_id: i64,
    chunks: &[(usize, &str)],
) -> Result<(), AppError> {
    conn.execute_batch("SAVEPOINT replace_chunks")?;
    let result = (|| -> Result<(), AppError> {
        conn.execute("DELETE FROM chunks WHERE file_id = ?1", params![file_id])?;
        let mut stmt = conn.prepare(
            "INSERT INTO chunks (file_id, chunk_index, text) VALUES (?1, ?2, ?3)",
        )?;
        for (idx, text) in chunks {
            stmt.execute(params![file_id, *idx as i64, text])?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            conn.execute_batch("RELEASE replace_chunks")?;
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK TO replace_chunks");
            Err(e)
        }
    }
}

/// Return files that have not yet been processed by the extraction pipeline
/// (i.e. `model_version` is still the empty string set by Phase B).
pub fn find_files_needing_extraction(
    conn: &Connection,
    root_id: i64,
) -> Result<Vec<(i64, String, String)>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, rel_path, media_type FROM files
         WHERE root_id = ?1 AND model_version = '' AND deleted_at IS NULL",
    )?;
    let rows = stmt.query_map(params![root_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn test_migrations_run_on_fresh_db() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('roots','files','chunks','index_jobs','activity_log')",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 5);
    }

    #[test]
    fn test_migrations_are_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap(); // must not fail
    }

    #[test]
    fn test_startup_recovery_marks_running_jobs_interrupted() {
        let conn = setup();
        let now = unix_now();
        conn.execute(
            "INSERT INTO index_jobs (status, index_marker, started_at, updated_at) VALUES ('running', 1, ?1, ?1)",
            params![now],
        ).unwrap();
        recover_interrupted_jobs(&conn).unwrap();
        let status: String = conn.query_row(
            "SELECT status FROM index_jobs LIMIT 1",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(status, "interrupted");
    }

    #[test]
    fn test_startup_recovery_leaves_completed_jobs_alone() {
        let conn = setup();
        let now = unix_now();
        conn.execute(
            "INSERT INTO index_jobs (status, index_marker, started_at, updated_at, completed_at) VALUES ('completed', 1, ?1, ?1, ?1)",
            params![now],
        ).unwrap();
        recover_interrupted_jobs(&conn).unwrap();
        let status: String = conn.query_row(
            "SELECT status FROM index_jobs LIMIT 1",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(status, "completed");
    }

    #[test]
    fn test_insert_root_returns_correct_fields() {
        let conn = setup();
        let root = insert_root(&conn, "/tmp/test-docs").unwrap();
        assert_eq!(root.path, "/tmp/test-docs");
        assert_eq!(root.label, "test-docs");
        assert!(root.active);
        assert!(root.id > 0);
        assert!(root.created_at > 0);
        assert!(root.last_indexed_at.is_none());
    }

    #[test]
    fn test_find_root_by_path_returns_none_for_missing() {
        let conn = setup();
        let result = find_root_by_path(&conn, "/nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_find_root_by_path_returns_existing() {
        let conn = setup();
        let inserted = insert_root(&conn, "/tmp/docs").unwrap();
        let found = find_root_by_path(&conn, "/tmp/docs").unwrap().unwrap();
        assert_eq!(found.id, inserted.id);
        assert_eq!(found.path, "/tmp/docs");
    }

    #[test]
    fn test_find_root_by_id() {
        let conn = setup();
        let inserted = insert_root(&conn, "/tmp/docs").unwrap();
        let found = find_root_by_id(&conn, inserted.id).unwrap().unwrap();
        assert_eq!(found.path, "/tmp/docs");
        assert!(find_root_by_id(&conn, 9999).unwrap().is_none());
    }

    #[test]
    fn test_update_root_last_indexed() {
        let conn = setup();
        let root = insert_root(&conn, "/tmp/docs").unwrap();
        update_root_last_indexed(&conn, root.id, 1_700_000_000).unwrap();
        let found = find_root_by_id(&conn, root.id).unwrap().unwrap();
        assert_eq!(found.last_indexed_at, Some(1_700_000_000));
    }

    fn insert_test_root(conn: &Connection) -> Root {
        insert_root(conn, "/tmp/test-root").unwrap()
    }

    #[test]
    fn test_upsert_file_inserts_new_record() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        let id = upsert_file_metadata(
            &conn, root.id, "docs/notes.txt", "notes.txt", "txt",
            1024, 1_700_000_000_000_000, "abc123", "", 1, now,
        ).unwrap();
        assert!(id > 0);
        let record = find_file_by_path(&conn, root.id, "docs/notes.txt").unwrap().unwrap();
        assert_eq!(record.fingerprint, "abc123");
        assert_eq!(record.size_bytes, 1024);
    }

    #[test]
    fn test_upsert_file_updates_existing_record() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        upsert_file_metadata(&conn, root.id, "a.txt", "a.txt", "txt", 100, 1000, "fp1", "", 1, now).unwrap();
        upsert_file_metadata(&conn, root.id, "a.txt", "a.txt", "txt", 200, 2000, "fp2", "", 2, now).unwrap();
        let record = find_file_by_path(&conn, root.id, "a.txt").unwrap().unwrap();
        assert_eq!(record.fingerprint, "fp2");
        assert_eq!(record.size_bytes, 200);
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM files WHERE root_id = ?1",
            params![root.id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_find_file_by_fingerprint() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        upsert_file_metadata(&conn, root.id, "a.txt", "a.txt", "txt", 100, 1000, "unique_fp", "", 1, now).unwrap();
        let found = find_file_by_fingerprint(&conn, root.id, "unique_fp").unwrap().unwrap();
        assert_eq!(found.rel_path, "a.txt");
        assert!(find_file_by_fingerprint(&conn, root.id, "nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_stamp_index_marker_updates_marker() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        let id = upsert_file_metadata(&conn, root.id, "a.txt", "a.txt", "txt", 100, 1000, "fp", "", 1, now).unwrap();
        stamp_index_marker(&conn, id, 99, now).unwrap();
        let record = find_file_by_path(&conn, root.id, "a.txt").unwrap().unwrap();
        assert_eq!(record.index_marker, 99);
    }

    #[test]
    fn test_move_file_updates_path() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        let id = upsert_file_metadata(&conn, root.id, "old/a.txt", "a.txt", "txt", 100, 1000, "fp", "", 1, now).unwrap();
        move_file(&conn, id, "new/a.txt", "a.txt", 2000, 2, now).unwrap();
        assert!(find_file_by_path(&conn, root.id, "old/a.txt").unwrap().is_none());
        let moved = find_file_by_path(&conn, root.id, "new/a.txt").unwrap().unwrap();
        assert_eq!(moved.index_marker, 2);
    }

    #[test]
    fn test_sweep_deleted_files_soft_deletes_unseen() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        upsert_file_metadata(&conn, root.id, "keep.txt", "keep.txt", "txt", 100, 1000, "fp1", "", 1, now).unwrap();
        upsert_file_metadata(&conn, root.id, "gone.txt", "gone.txt", "txt", 100, 1000, "fp2", "", 0, now).unwrap();
        let deleted = sweep_deleted_files(&conn, root.id, 1, now).unwrap();
        assert_eq!(deleted, 1);
        let active: i64 = conn.query_row(
            "SELECT COUNT(*) FROM files WHERE root_id = ?1 AND deleted_at IS NULL",
            params![root.id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(active, 1);
        let deleted_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM files WHERE root_id = ?1 AND deleted_at IS NOT NULL",
            params![root.id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(deleted_count, 1);
    }

    #[test]
    fn test_soft_deleted_file_not_found_by_fingerprint() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        let id = upsert_file_metadata(&conn, root.id, "a.txt", "a.txt", "txt", 100, 1000, "fp", "", 0, now).unwrap();
        sweep_deleted_files(&conn, root.id, 1, now).unwrap();
        assert!(find_file_by_fingerprint(&conn, root.id, "fp").unwrap().is_none());
        let deleted: Option<i64> = conn.query_row(
            "SELECT deleted_at FROM files WHERE id = ?1",
            params![id],
            |r| r.get(0),
        ).unwrap();
        assert!(deleted.is_some());
    }

    #[test]
    fn test_insert_and_complete_job() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        let job_id = insert_job(&conn, root.id, 42, now).unwrap();
        assert!(job_id > 0);
        let status: String = conn.query_row(
            "SELECT status FROM index_jobs WHERE id = ?1",
            params![job_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(status, "running");

        let counts = JobCounts {
            files_total: 10, files_done: 10, files_added: 8,
            files_updated: 1, files_moved: 1, files_deleted: 0, error_count: 0,
        };
        complete_job(&conn, job_id, &counts, now).unwrap();
        let (status, added, phase, completed_at): (String, i64, Option<String>, Option<i64>) = conn.query_row(
            "SELECT status, files_added, phase, completed_at FROM index_jobs WHERE id = ?1",
            params![job_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).unwrap();
        assert_eq!(status, "completed");
        assert_eq!(added, 8);
        assert!(phase.is_none(), "phase should be NULL after completion");
        assert!(completed_at.is_some(), "completed_at should be set after completion");
    }

    #[test]
    fn test_update_job_phase() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        let job_id = insert_job(&conn, root.id, 1, now).unwrap();
        update_job_phase(&conn, job_id, "fingerprinting", now).unwrap();
        let phase: String = conn.query_row(
            "SELECT phase FROM index_jobs WHERE id = ?1",
            params![job_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(phase, "fingerprinting");
    }

    #[test]
    fn test_update_job_counts() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        let job_id = insert_job(&conn, root.id, 1, now).unwrap();
        let counts = JobCounts {
            files_total: 50, files_done: 20, files_added: 15,
            files_updated: 3, files_moved: 2, files_deleted: 0, error_count: 1,
        };
        update_job_counts(&conn, job_id, &counts, now).unwrap();
        let (total, done, added, updated, moved, errors): (i64, i64, i64, i64, i64, i64) = conn.query_row(
            "SELECT files_total, files_done, files_added, files_updated, files_moved, error_count
             FROM index_jobs WHERE id = ?1",
            params![job_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        ).unwrap();
        assert_eq!(total, 50);
        assert_eq!(done, 20);
        assert_eq!(added, 15);
        assert_eq!(updated, 3);
        assert_eq!(moved, 2);
        assert_eq!(errors, 1);
        // Job must still be 'running' (update_job_counts does not complete the job)
        let status: String = conn.query_row(
            "SELECT status FROM index_jobs WHERE id = ?1",
            params![job_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(status, "running");
    }

    // ── Phase C db helpers ────────────────────────────────────────────────────

    #[test]
    fn test_update_file_content_persists_fields() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        let id = upsert_file_metadata(
            &conn, root.id, "a.txt", "a.txt", "txt",
            100, 1000, "fp", "", 1, now,
        ).unwrap();
        update_file_content(&conn, id, "hello world", 0.95, "en", "ext-v1", now).unwrap();
        let (text, confidence, lang, mv): (String, f64, String, String) = conn.query_row(
            "SELECT extracted_text, confidence, lang_hint, model_version FROM files WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).unwrap();
        assert_eq!(text, "hello world");
        assert!((confidence - 0.95).abs() < 1e-6);
        assert_eq!(lang, "en");
        assert_eq!(mv, "ext-v1");
    }

    #[test]
    fn test_replace_chunks_inserts_all() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        let id = upsert_file_metadata(
            &conn, root.id, "a.txt", "a.txt", "txt",
            100, 1000, "fp", "", 1, now,
        ).unwrap();
        let chunks = vec![(0usize, "chunk zero"), (1, "chunk one"), (2, "chunk two")];
        replace_chunks(&conn, id, &chunks).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM chunks WHERE file_id = ?1",
            params![id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_replace_chunks_deletes_stale_on_reindex() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        let id = upsert_file_metadata(
            &conn, root.id, "a.txt", "a.txt", "txt",
            100, 1000, "fp", "", 1, now,
        ).unwrap();
        replace_chunks(&conn, id, &[(0, "old chunk a"), (1, "old chunk b")]).unwrap();
        // Re-index: new extraction produces only one chunk
        replace_chunks(&conn, id, &[(0, "new single chunk")]).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM chunks WHERE file_id = ?1",
            params![id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);
        let text: String = conn.query_row(
            "SELECT text FROM chunks WHERE file_id = ?1",
            params![id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(text, "new single chunk");
    }

    #[test]
    fn test_chunks_cascade_deleted_with_file() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        let id = upsert_file_metadata(
            &conn, root.id, "a.txt", "a.txt", "txt",
            100, 1000, "fp", "", 1, now,
        ).unwrap();
        replace_chunks(&conn, id, &[(0, "chunk")]).unwrap();
        // Hard-delete the file (bypassing soft-delete, for test purposes)
        conn.execute("DELETE FROM files WHERE id = ?1", params![id]).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM chunks WHERE file_id = ?1",
            params![id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 0, "chunks must be cascade-deleted when the parent file is deleted");
    }

    #[test]
    fn test_find_files_needing_extraction_returns_only_unprocessed() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        // Not yet extracted (model_version = '')
        let id_pending = upsert_file_metadata(
            &conn, root.id, "pending.txt", "pending.txt", "txt",
            100, 1000, "fp1", "", 1, now,
        ).unwrap();
        // Already extracted (model_version = 'ext-v1')
        upsert_file_metadata(
            &conn, root.id, "done.txt", "done.txt", "txt",
            100, 1000, "fp2", "ext-v1", 1, now,
        ).unwrap();
        let pending = find_files_needing_extraction(&conn, root.id).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].0, id_pending);
        assert_eq!(pending[0].1, "pending.txt");
        assert_eq!(pending[0].2, "txt");
    }

    #[test]
    fn test_find_files_needing_extraction_excludes_soft_deleted() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        let id = upsert_file_metadata(
            &conn, root.id, "a.txt", "a.txt", "txt",
            100, 1000, "fp", "", 0, now,
        ).unwrap();
        sweep_deleted_files(&conn, root.id, 1, now).unwrap();
        // Confirm soft-deleted
        let deleted_at: Option<i64> = conn.query_row(
            "SELECT deleted_at FROM files WHERE id = ?1",
            params![id],
            |r| r.get(0),
        ).unwrap();
        assert!(deleted_at.is_some());
        let pending = find_files_needing_extraction(&conn, root.id).unwrap();
        assert!(pending.is_empty());
    }

    #[test]
    fn test_log_activity_inserts_rows() {
        let conn = setup();
        let root = insert_test_root(&conn);
        let now = unix_now();
        let job_id = insert_job(&conn, root.id, 1, now).unwrap();
        log_activity(&conn, "job_started", Some(root.id), None, Some(job_id), None, now).unwrap();
        log_activity(&conn, "job_completed", Some(root.id), None, Some(job_id), None, now).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM activity_log WHERE job_id = ?1",
            params![job_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 2);
    }
}
