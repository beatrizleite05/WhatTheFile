use rusqlite::{Connection, params};
use crate::errors::AppError;
use super::JobCounts;

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

#[cfg(test)]
#[path = "db_jobs_test.rs"]
mod tests;
