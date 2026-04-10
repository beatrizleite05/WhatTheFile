use rusqlite::{Connection, params};
use crate::errors::AppError;
use super::FileRecord;

pub fn find_file_by_path(
    conn: &Connection,
    root_id: i64,
    rel_path: &str,
) -> Result<Option<FileRecord>, AppError> {
    use rusqlite::OptionalExtension;
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
    use rusqlite::OptionalExtension;
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
    chunks: &[(usize, &str, Option<&[u8]>)],
) -> Result<(), AppError> {
    conn.execute_batch("SAVEPOINT replace_chunks")?;
    let result = (|| -> Result<(), AppError> {
        // Delete from chunks_vec first (no cascade from chunks virtual table).
        conn.execute(
            "DELETE FROM chunks_vec WHERE chunk_id IN (SELECT id FROM chunks WHERE file_id = ?1)",
            params![file_id],
        )?;
        conn.execute("DELETE FROM chunks WHERE file_id = ?1", params![file_id])?;

        let mut chunk_stmt = conn.prepare(
            "INSERT INTO chunks (file_id, chunk_index, text) VALUES (?1, ?2, ?3)",
        )?;
        let mut vec_stmt = conn.prepare(
            "INSERT INTO chunks_vec (chunk_id, embedding) VALUES (?1, ?2)",
        )?;
        for (idx, text, emb) in chunks {
            chunk_stmt.execute(params![file_id, *idx as i64, text])?;
            if let Some(emb_bytes) = emb {
                let chunk_id = conn.last_insert_rowid();
                vec_stmt.execute(params![chunk_id, emb_bytes])?;
            }
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

#[cfg(test)]
#[path = "db_files_test.rs"]
mod tests;
