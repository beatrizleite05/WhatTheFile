# Database Schema

SQLite + sqlite-vec. Single file at `db/index.sqlite`.

---

## Connection Setup

Applied on every connection open, before any query:

```rust
conn.pragma_update(None, "busy_timeout", 5000)?;
conn.pragma_update(None, "foreign_keys", "ON")?;
conn.pragma_update(None, "journal_mode", "WAL")?;
conn.pragma_update(None, "synchronous", "NORMAL")?;
conn.pragma_update(None, "temp_store", "MEMORY")?;

// Register sqlite-vec extension (statically linked via `sqlite-vec` crate)
sqlite_vec::sqlite3_vec_init(conn.handle())?;
```

---

## Schema Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                              roots                                  │
│─────────────────────────────────────────────────────────────────────│
│ PK  id               INTEGER                                        │
│     path             TEXT  UNIQUE                                   │
│     label            TEXT  COLLATE NOCASE                           │
│     active           INTEGER  DEFAULT 1                             │
│     created_at       INTEGER                                        │
│     last_indexed_at  INTEGER                                        │
└────────────────┬────────────────────────────────────────────────────┘
                 │ 1
                 │
                 │ N
┌────────────────▼────────────────────────────────────────────────────┐
│                              files                                   │
│─────────────────────────────────────────────────────────────────────│
│ PK  id              INTEGER                                         │
│ FK  root_id         INTEGER → roots.id  ON DELETE CASCADE           │
│     rel_path        TEXT                                            │
│     filename         TEXT                                            │
│     media_type      TEXT    ('pdf'|'docx'|'xlsx'|'csv'|             │
│                              'txt'|'md'|'png'|'jpg')                │
│     size_bytes      INTEGER                                         │
│     mtime_ns        INTEGER                                         │
│     fingerprint      TEXT    (blake3 hex)                            │
│     model_version   TEXT    (embedding model id+version)            │
│     confidence       REAL    DEFAULT 1.0                             │
│     extracted_text  TEXT    DEFAULT ''                              │
│     structured_meta TEXT    (JSON)                                  │
│     lang_hint       TEXT    DEFAULT 'unknown'  (whatlang)           │
│     index_marker    INTEGER DEFAULT 0                               │
│     indexed_at      INTEGER                                         │
│     deleted_at      INTEGER (NULL = active, soft delete)            │
│     UNIQUE(root_id, rel_path)                                       │
└──────┬────────────────────────────────────────────────────────────┬─┘
       │ 1                                                          │ 1
       │                                                            │
       │ N                                                          │ N
┌──────▼──────────────────┐             ┌───────────────────────────▼─┐
│         chunks          │             │        files_fts (FTS5)      │
│─────────────────────────│             │─────────────────────────────│
│ PK  id          INTEGER │             │  filename                    │
│ FK  file_id      INTEGER │             │  rel_path                   │
│          → files.id      │             │  extracted_text             │
│        ON DELETE CASCADE│             │  content='files'             │
│     chunk_index INTEGER │             │  content_rowid='id'         │
│     text        TEXT    │             │  tokenize='unicode61        │
│     UNIQUE(file_id,      │             │    remove_diacritics 2'     │
│            chunk_index) │             │  (kept in sync via triggers)│
└──────┬──────────────────┘             └─────────────────────────────┘
       │ 1
       │
       │ 1
┌──────▼──────────────────┐
│      chunks_vec (vec0)  │
│─────────────────────────│
│ PK  chunk_id   INTEGER  │  ← maps to chunks.id
│     embedding  FLOAT[768]│  ← stored as little-endian f32 blob
│  (KNN via MATCH / k =N) │
└─────────────────────────┘


┌─────────────────────────────────────────────────────────────────────┐
│                           index_jobs                                │
│─────────────────────────────────────────────────────────────────────│
│ PK  id             INTEGER                                          │
│ FK  root_id        INTEGER → roots.id  (NULL = all roots)           │
│     status         TEXT  ('running'|'completed'|'paused'|           │
│                           'cancelled'|'interrupted')                │
│     phase          TEXT  ('discovering'|'extracting'|'embedding')   │
│     index_marker   INTEGER                                          │
│     files_total     INTEGER  DEFAULT 0                               │
│     files_done      INTEGER  DEFAULT 0                               │
│     files_added     INTEGER  DEFAULT 0                               │
│     files_updated   INTEGER  DEFAULT 0                               │
│     files_moved     INTEGER  DEFAULT 0                               │
│     files_deleted   INTEGER  DEFAULT 0                               │
│     error_count    INTEGER  DEFAULT 0                               │
│     cursor_path    TEXT    (last processed path)                    │
│     started_at     INTEGER                                          │
│     updated_at     INTEGER                                          │
│     completed_at   INTEGER                                          │
└─────────────────────────────────────────────────────────────────────┘


┌─────────────────────────────────────────────────────────────────────┐
│                          activity_log                               │
│─────────────────────────────────────────────────────────────────────│
│ PK  id          INTEGER                                             │
│     event_type  TEXT  ('file_indexed'|'file_reindexed'|               │
│                        'file_removed'|'job_started'|                 │
│                        'job_completed'|'index_deleted')             │
│ FK  root_id     INTEGER → roots.id                                  │
│ FK  file_id      INTEGER → files.id                                   │
│ FK  job_id      INTEGER → index_jobs.id                             │
│     detail      TEXT    (JSON)                                      │
│     created_at  INTEGER                                             │
└─────────────────────────────────────────────────────────────────────┘
```

---

## DDL

```sql
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
  model_version   TEXT    NOT NULL,
  confidence      REAL    NOT NULL DEFAULT 1.0,
  extracted_text  TEXT    NOT NULL DEFAULT '',
  structured_meta TEXT,
  lang_hint       TEXT    NOT NULL DEFAULT 'unknown',  -- detected via whatlang at index time; informational only in v1
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

CREATE VIRTUAL TABLE IF NOT EXISTS chunks_vec USING vec0(
  chunk_id  INTEGER PRIMARY KEY,
  embedding FLOAT[768]
);

CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
  filename,
  rel_path,
  extracted_text,
  content='files',
  content_rowid='id',
  tokenize='unicode61 remove_diacritics 2'
);

-- FTS sync triggers
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
```

---

## Key Patterns

### index_marker (deletion detection)

At the start of each index job, a new `index_marker` integer is generated (e.g. unix timestamp). Every file touched during the job is stamped with this marker. At job completion:

```sql
-- Files not seen this run = deleted from disk → soft-delete them
UPDATE files
SET deleted_at = ?1
WHERE root_id = ?2
  AND index_marker != ?3
  AND deleted_at IS NULL;
```

No in-memory "seen set" required.

### Rename/move detection

```sql
-- Fingerprint exists at a different path → move, not re-index
SELECT id, rel_path FROM files
WHERE fingerprint = ?1 AND root_id = ?2 AND deleted_at IS NULL;
```

If found: `UPDATE files SET rel_path = ?, filename = ?, index_marker = ?` — no re-extraction, no re-embedding.

### model_version cache key

```sql
-- Skip file if fingerprint AND model version are unchanged
SELECT id FROM files
WHERE root_id = ?1 AND rel_path = ?2
  AND fingerprint = ?3 AND model_version = ?4
  AND deleted_at IS NULL;
```

Upgrading `nomic-embed-text` changes `model_version` → forces re-embedding on next index run.

### sqlite-vec embedding serialization

```rust
fn serialize_embedding(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn deserialize_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect()
}
```

### KNN query (sqlite-vec syntax)

```sql
-- MATCH + k= is required to activate the KNN index — do not use WHERE distance <
SELECT cv.chunk_id, cv.distance, c.file_id
FROM chunks_vec cv
JOIN chunks c ON c.id = cv.chunk_id
WHERE cv.embedding MATCH ?1 AND k = 50
ORDER BY cv.distance;
```

Score blending (BM25 + cosine) happens in `search.rs`, not in SQL — sqlite-vec KNN must be the outermost scan.

### Startup recovery

```sql
UPDATE index_jobs
SET status = 'interrupted', updated_at = ?1
WHERE status = 'running';
```

Run once on app open before starting any new index job.
