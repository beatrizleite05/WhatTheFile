use super::*;
use crate::db::{open_and_migrate, unix_now, insert_root, JobCounts};
use rusqlite::params;

fn setup() -> rusqlite::Connection {
    open_and_migrate(std::path::Path::new(":memory:")).unwrap()
}

#[test]
fn test_insert_and_complete_job() {
    let conn = setup();
    let root = insert_root(&conn, "/tmp/test-root").unwrap();
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
    let root = insert_root(&conn, "/tmp/test-root").unwrap();
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
    let root = insert_root(&conn, "/tmp/test-root").unwrap();
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
    let status: String = conn.query_row(
        "SELECT status FROM index_jobs WHERE id = ?1",
        params![job_id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(status, "running");
}

#[test]
fn test_log_activity_inserts_rows() {
    let conn = setup();
    let root = insert_root(&conn, "/tmp/test-root").unwrap();
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
