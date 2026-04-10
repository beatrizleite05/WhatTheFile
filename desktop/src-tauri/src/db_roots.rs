use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use crate::errors::AppError;
use super::{Root, unix_now};

pub fn insert_root(conn: &Connection, path: &str) -> Result<Root, AppError> {
    let label = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AppError::Config(format!("root path has no valid filename component: {path}")))?
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

#[cfg(test)]
#[path = "db_roots_test.rs"]
mod tests;

