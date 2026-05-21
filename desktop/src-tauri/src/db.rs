use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use crate::errors::AppError;

#[path = "db_roots.rs"]
mod db_roots;
#[path = "db_files.rs"]
mod db_files;
#[path = "db_jobs.rs"]
mod db_jobs;

pub use db_roots::{insert_root, find_root_by_path, find_root_by_id, update_root_last_indexed};
pub use db_files::{find_file_by_path, find_file_by_fingerprint, upsert_file_metadata,
                   stamp_index_marker, move_file, sweep_deleted_files, update_file_content,
                   replace_chunks, find_files_needing_extraction};
pub use db_jobs::{insert_job, update_job_phase, update_job_counts, update_job_progress,
                   complete_job, log_activity, list_completed_jobs, CompletedJob};

// ── helpers ──────────────────────────────────────────────────────────────────

pub(crate) fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is before UNIX epoch — cannot compute timestamps")
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

#[allow(dead_code)]
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

const MIGRATION_002: &str = r#"
ALTER TABLE chunks ADD COLUMN embedding BLOB;
UPDATE schema_version SET version = 2;
"#;

/// Introduce the sqlite-vec KNN virtual table.
/// Migrates existing embeddings from chunks.embedding → chunks_vec, then
/// drops the now-redundant BLOB column (requires SQLite 3.35+, bundled).
///
/// FLOAT[768]: nomic-embed-text-v2-moe outputs 768-dim by architecture.
/// Must stay in sync with `llm::embeddings::EXPECTED_DIM`.
const MIGRATION_003: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS chunks_vec USING vec0(
  chunk_id  INTEGER PRIMARY KEY,
  embedding FLOAT[768]
);

INSERT OR IGNORE INTO chunks_vec(chunk_id, embedding)
SELECT id, embedding FROM chunks WHERE embedding IS NOT NULL;

ALTER TABLE chunks DROP COLUMN embedding;

UPDATE schema_version SET version = 3;
"#;

pub(crate) fn run_migrations(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);",
    )?;
    let version: Option<i64> = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |r| r.get(0))
        .optional()?;
    match version {
        None => {
            conn.execute_batch(MIGRATION_001)?;
            conn.execute("INSERT INTO schema_version VALUES (1)", [])?;
            conn.execute_batch(MIGRATION_002)?;
            conn.execute_batch(MIGRATION_003)?;
        }
        Some(1) => {
            conn.execute_batch(MIGRATION_002)?;
            conn.execute_batch(MIGRATION_003)?;
        }
        Some(2) => {
            conn.execute_batch(MIGRATION_003)?;
        }
        Some(_) => {}
    }
    Ok(())
}

pub fn open_and_migrate(path: &Path) -> Result<Connection, AppError> {
    // Register sqlite-vec for every subsequent connection opened in this process.
    // sqlite3_auto_extension deduplicates by function pointer, so repeated calls
    // are a no-op.  Must run before Connection::open so the extension is live
    // before run_migrations tries to CREATE VIRTUAL TABLE … USING vec0.
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(
            std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ()),
        ));
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
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

pub fn clear_index(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "BEGIN;
         DELETE FROM activity_log;
         DELETE FROM chunks_vec;
         DELETE FROM files;
         DELETE FROM index_jobs;
         COMMIT;",
    )?;
    Ok(())
}

pub fn clear_root_index(conn: &Connection, root_id: i64) -> Result<(), AppError> {
    conn.execute_batch("SAVEPOINT clear_root")?;
    let result = (|| -> Result<(), AppError> {
        // 1. Drop all activity_log rows that reference anything owned by this root.
        conn.execute(
            "DELETE FROM activity_log
             WHERE root_id = ?1
                OR job_id  IN (SELECT id FROM index_jobs WHERE root_id = ?1)
                OR file_id IN (SELECT id FROM files      WHERE root_id = ?1)",
            params![root_id],
        )?;
        // 2. Drop vector embeddings before their parent chunks are removed.
        conn.execute(
            "DELETE FROM chunks_vec WHERE chunk_id IN (
               SELECT c.id FROM chunks c JOIN files f ON c.file_id = f.id WHERE f.root_id = ?1
             )",
            params![root_id],
        )?;
        // 3. Drop chunks explicitly (files ON DELETE CASCADE would also do this,
        //    but being explicit avoids any ordering ambiguity with the vec table).
        conn.execute(
            "DELETE FROM chunks WHERE file_id IN (SELECT id FROM files WHERE root_id = ?1)",
            params![root_id],
        )?;
        // 4. Drop jobs before files — activity_log.job_id has no CASCADE.
        conn.execute("DELETE FROM index_jobs WHERE root_id = ?1", params![root_id])?;
        // 5. Drop files last; no remaining FK references point at them.
        conn.execute("DELETE FROM files WHERE root_id = ?1", params![root_id])?;
        Ok(())
    })();
    if result.is_ok() {
        conn.execute_batch("RELEASE clear_root")?;
    } else {
        conn.execute_batch("ROLLBACK TO clear_root")?;
    }
    result
}

#[cfg(test)]
#[path = "db_test.rs"]
mod tests;
