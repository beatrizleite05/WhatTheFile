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

pub fn insert_root(_conn: &Connection, _path: &str) -> Result<Root, AppError> { todo!() }
pub fn find_root_by_path(_conn: &Connection, _path: &str) -> Result<Option<Root>, AppError> { todo!() }
pub fn find_root_by_id(_conn: &Connection, _id: i64) -> Result<Option<Root>, AppError> { todo!() }
pub fn update_root_last_indexed(_conn: &Connection, _root_id: i64, _ts: i64) -> Result<(), AppError> { todo!() }
pub fn find_file_by_path(_conn: &Connection, _root_id: i64, _rel_path: &str) -> Result<Option<FileRecord>, AppError> { todo!() }
pub fn find_file_by_fingerprint(_conn: &Connection, _root_id: i64, _fingerprint: &str) -> Result<Option<FileRecord>, AppError> { todo!() }
pub fn upsert_file_metadata(_conn: &Connection, _root_id: i64, _rel_path: &str, _filename: &str, _media_type: &str, _size_bytes: i64, _mtime_ns: i64, _fingerprint: &str, _model_version: &str, _index_marker: i64, _indexed_at: i64) -> Result<i64, AppError> { todo!() }
pub fn stamp_index_marker(_conn: &Connection, _file_id: i64, _marker: i64, _now: i64) -> Result<(), AppError> { todo!() }
pub fn move_file(_conn: &Connection, _file_id: i64, _new_rel_path: &str, _new_filename: &str, _new_mtime_ns: i64, _marker: i64, _now: i64) -> Result<(), AppError> { todo!() }
pub fn sweep_deleted_files(_conn: &Connection, _root_id: i64, _marker: i64, _deleted_at: i64) -> Result<i64, AppError> { todo!() }
pub fn insert_job(_conn: &Connection, _root_id: i64, _marker: i64, _now: i64) -> Result<i64, AppError> { todo!() }
pub fn update_job_phase(_conn: &Connection, _job_id: i64, _phase: &str, _now: i64) -> Result<(), AppError> { todo!() }
pub fn update_job_counts(_conn: &Connection, _job_id: i64, _counts: &JobCounts, _now: i64) -> Result<(), AppError> { todo!() }
pub fn complete_job(_conn: &Connection, _job_id: i64, _counts: &JobCounts, _now: i64) -> Result<(), AppError> { todo!() }
pub fn log_activity(_conn: &Connection, _event_type: &str, _root_id: Option<i64>, _file_id: Option<i64>, _job_id: Option<i64>, _detail: Option<&str>, _created_at: i64) -> Result<(), AppError> { todo!() }

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
}
