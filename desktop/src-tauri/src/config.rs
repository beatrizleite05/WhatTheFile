use rusqlite::Connection;
use serde::Serialize;
use crate::{db::{self, Root}, errors::AppError};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RootPayload {
    pub id: i64,
    pub path: String,
    pub label: String,
    pub active: bool,
    pub created_at: i64,
    pub last_indexed_at: Option<i64>,
}

impl From<Root> for RootPayload {
    fn from(r: Root) -> Self {
        Self {
            id: r.id,
            path: r.path,
            label: r.label,
            active: r.active,
            created_at: r.created_at,
            last_indexed_at: r.last_indexed_at,
        }
    }
}

pub fn add_root(conn: &Connection, path: &str) -> Result<RootPayload, AppError> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|_| AppError::Config(format!("path does not exist or is not accessible: {path}")))?;
    if !canonical.is_dir() {
        return Err(AppError::Config(format!("path is not a directory: {path}")));
    }
    let path_str = canonical.to_string_lossy().to_string();

    if let Some(existing) = db::find_root_by_path(conn, &path_str)? {
        if !existing.active {
            conn.execute("UPDATE roots SET active = 1 WHERE id = ?1", [existing.id])?;
            let reactivated = db::find_root_by_id(conn, existing.id)?
                .ok_or_else(|| AppError::Config("root vanished after reactivation".into()))?;
            return Ok(reactivated.into());
        }
        return Ok(existing.into());
    }
    Ok(db::insert_root(conn, &path_str)?.into())
}

pub fn list_roots(conn: &Connection) -> Result<Vec<RootPayload>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, path, label, active, created_at, last_indexed_at
         FROM roots WHERE active = 1 ORDER BY created_at ASC",
    )?;
    let roots = stmt
        .query_map([], |row| {
            Ok(Root {
                id: row.get(0)?,
                path: row.get(1)?,
                label: row.get(2)?,
                active: row.get::<_, i64>(3).map(|v| v != 0)?,
                created_at: row.get(4)?,
                last_indexed_at: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(roots.into_iter().map(Into::into).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use tempfile::tempdir;

    // NOTE: config tests use tempfile to get a real DB path for open_and_migrate
    // (open_and_migrate requires a real file path — cannot use ":memory:" directly)

    #[test]
    fn test_add_root_returns_correct_payload() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.sqlite");
        let conn = db::open_and_migrate(&db_path).unwrap();

        let root_dir = dir.path().join("docs");
        std::fs::create_dir(&root_dir).unwrap();
        let payload = add_root(&conn, root_dir.to_str().unwrap()).unwrap();

        assert!(payload.id > 0);
        assert!(payload.path.contains("docs"));
        assert_eq!(payload.label, "docs");
        assert!(payload.active);
        assert!(payload.last_indexed_at.is_none());
    }

    #[test]
    fn test_add_root_is_idempotent() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.sqlite");
        let conn = db::open_and_migrate(&db_path).unwrap();

        let root_dir = dir.path().join("docs");
        std::fs::create_dir(&root_dir).unwrap();
        let first = add_root(&conn, root_dir.to_str().unwrap()).unwrap();
        let second = add_root(&conn, root_dir.to_str().unwrap()).unwrap();
        assert_eq!(first.id, second.id);

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM roots", [], |r| r.get(0)
        ).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_add_root_reactivated_root_returns_active_payload() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.sqlite");
        let conn = db::open_and_migrate(&db_path).unwrap();

        let root_dir = dir.path().join("docs");
        std::fs::create_dir(&root_dir).unwrap();
        let first = add_root(&conn, root_dir.to_str().unwrap()).unwrap();
        // Simulate remove_root deactivating it.
        conn.execute("UPDATE roots SET active = 0 WHERE id = ?1", [first.id]).unwrap();
        // Re-adding must return active: true with the correct id.
        let reactivated = add_root(&conn, root_dir.to_str().unwrap()).unwrap();
        assert_eq!(reactivated.id, first.id);
        assert!(reactivated.active, "reactivated root must have active: true");
    }

    #[test]
    fn test_add_root_returns_error_for_nonexistent_path() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.sqlite");
        let conn = db::open_and_migrate(&db_path).unwrap();
        let result = add_root(&conn, "/nonexistent/path/that/does/not/exist");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("config error"));
    }

    #[test]
    fn test_add_root_returns_error_for_file_not_dir() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.sqlite");
        let conn = db::open_and_migrate(&db_path).unwrap();
        let file_path = dir.path().join("a.txt");
        std::fs::write(&file_path, "hello").unwrap();
        let result = add_root(&conn, file_path.to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_list_roots_returns_active_roots() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.sqlite");
        let conn = db::open_and_migrate(&db_path).unwrap();
        let root_dir = dir.path().join("docs");
        std::fs::create_dir(&root_dir).unwrap();
        add_root(&conn, root_dir.to_str().unwrap()).unwrap();
        let roots = list_roots(&conn).unwrap();
        assert_eq!(roots.len(), 1);
        assert!(roots[0].active);
        assert_eq!(roots[0].label, "docs");
    }
}
