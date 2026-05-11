use super::*;

fn setup() -> rusqlite::Connection {
    open_and_migrate(std::path::Path::new(":memory:")).unwrap()
}

#[test]
fn test_migrations_run_on_fresh_db() {
    let conn = setup();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('roots','files','chunks','index_jobs','activity_log')",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 5);
    let vec_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='chunks_vec'",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(vec_exists, 1);
}

#[test]
fn test_migrations_are_idempotent() {
    let conn = setup();
    run_migrations(&conn).unwrap();
}

#[test]
fn test_startup_recovery_marks_running_jobs_interrupted() {
    let conn = setup();
    let now = unix_now();
    conn.execute(
        "INSERT INTO index_jobs (status, index_marker, started_at, updated_at) VALUES ('running', 1, ?1, ?1)",
        rusqlite::params![now],
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
        rusqlite::params![now],
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
fn test_clear_index_removes_all_indexed_data_and_preserves_roots() {
    let conn = setup();
    let now = unix_now();

    conn.execute(
        "INSERT INTO roots (path, label, active, created_at) VALUES ('/test', 'Test', 1, ?1)",
        rusqlite::params![now],
    ).unwrap();
    let root_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO files (root_id, rel_path, filename, media_type, size_bytes, mtime_ns, fingerprint, index_marker, indexed_at)
         VALUES (?1, 'a.txt', 'a.txt', 'text/plain', 0, 0, 'fp', 1, ?2)",
        rusqlite::params![root_id, now],
    ).unwrap();
    let file_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO index_jobs (root_id, status, index_marker, started_at, updated_at)
         VALUES (?1, 'completed', 1, ?2, ?2)",
        rusqlite::params![root_id, now],
    ).unwrap();
    let job_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO activity_log (event_type, file_id, job_id, created_at) VALUES ('indexed', ?1, ?2, ?3)",
        rusqlite::params![file_id, job_id, now],
    ).unwrap();

    clear_index(&conn).unwrap();

    let file_count: i64 = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0)).unwrap();
    let job_count: i64 = conn.query_row("SELECT COUNT(*) FROM index_jobs", [], |r| r.get(0)).unwrap();
    let log_count: i64 = conn.query_row("SELECT COUNT(*) FROM activity_log", [], |r| r.get(0)).unwrap();
    let root_count: i64 = conn.query_row("SELECT COUNT(*) FROM roots", [], |r| r.get(0)).unwrap();

    assert_eq!(file_count, 0);
    assert_eq!(job_count, 0);
    assert_eq!(log_count, 0);
    assert_eq!(root_count, 1, "roots must survive a clear_index");
}

#[test]
fn test_clear_root_index_removes_root_data_and_preserves_other_roots() {
    let conn = setup();
    let now = unix_now();

    for path in ["/root/a", "/root/b"] {
        conn.execute(
            "INSERT INTO roots (path, label, active, created_at) VALUES (?1, ?1, 1, ?2)",
            rusqlite::params![path, now],
        ).unwrap();
    }
    let root_a: i64 = conn.query_row("SELECT id FROM roots WHERE path = '/root/a'", [], |r| r.get(0)).unwrap();
    let root_b: i64 = conn.query_row("SELECT id FROM roots WHERE path = '/root/b'", [], |r| r.get(0)).unwrap();

    for (root_id, rel) in [(root_a, "a.txt"), (root_b, "b.txt")] {
        conn.execute(
            "INSERT INTO files (root_id, rel_path, filename, media_type, size_bytes, mtime_ns, fingerprint, index_marker, indexed_at)
             VALUES (?1, ?2, ?2, 'text/plain', 0, 0, ?2, 1, ?3)",
            rusqlite::params![root_id, rel, now],
        ).unwrap();
    }

    clear_root_index(&conn, root_a).unwrap();

    let a_files: i64 = conn.query_row("SELECT COUNT(*) FROM files WHERE root_id = ?1", rusqlite::params![root_a], |r| r.get(0)).unwrap();
    let b_files: i64 = conn.query_row("SELECT COUNT(*) FROM files WHERE root_id = ?1", rusqlite::params![root_b], |r| r.get(0)).unwrap();
    let root_count: i64 = conn.query_row("SELECT COUNT(*) FROM roots", [], |r| r.get(0)).unwrap();

    assert_eq!(a_files, 0);
    assert_eq!(b_files, 1, "other root's files must survive");
    assert_eq!(root_count, 2, "roots must survive clear_root_index");
}

#[test]
fn test_migration_003_schema_version_and_no_embedding_column() {
    let conn = setup();
    let cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(chunks)").unwrap();
        stmt.query_map([], |r| r.get::<_, String>(1)).unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert!(!cols.contains(&"embedding".to_string()), "embedding column must be dropped by migration 003");
    let version: i64 = conn.query_row(
        "SELECT version FROM schema_version LIMIT 1", [], |r| r.get(0),
    ).unwrap();
    assert_eq!(version, 3);
}
