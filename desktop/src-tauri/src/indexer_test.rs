use super::*;
use crate::indexer_progress::{Phase, test_helpers::{CaptureSender, no_throttle_reporter}};
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::{Arc, atomic::AtomicBool};
use tempfile::tempdir;

fn setup_db() -> (tempfile::TempDir, Connection) {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.sqlite");
    let conn = db::open_and_migrate(&db_path).unwrap();
    (dir, conn)
}

fn write_file(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).unwrap();
}

fn no_emit(_: &str, _: &serde_json::Value) {}
fn no_cancel() -> Arc<AtomicBool> { Arc::new(AtomicBool::new(false)) }

fn scan(conn: &Connection, root_id: i64, root_dir: &std::path::Path) -> Result<i64, crate::errors::AppError> {
    let capture = CaptureSender::default();
    let reporter = no_throttle_reporter(capture, 0, root_id);
    run_scan(conn, root_id, root_dir, "http://localhost:11434", &no_cancel(), &no_emit, reporter)
}

// ── existing behaviour tests (unchanged logic, updated call sites) ─────────────

#[test]
fn test_first_run_indexes_all_txt_files() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "a.txt", "hello");
    write_file(&root_dir, "b.txt", "world");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    let job_id = scan(&conn, root.id, &root_dir).unwrap();

    assert!(job_id > 0);
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE root_id = ?1 AND deleted_at IS NULL",
        params![root.id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 2);

    let status: String = conn.query_row(
        "SELECT status FROM index_jobs WHERE id = ?1",
        params![job_id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(status, "completed");
}

#[test]
fn test_first_run_files_added_count_matches() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "a.txt", "a");
    write_file(&root_dir, "b.md", "b");
    write_file(&root_dir, "c.csv", "c");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    let job_id = scan(&conn, root.id, &root_dir).unwrap();

    let added: i64 = conn.query_row(
        "SELECT files_added FROM index_jobs WHERE id = ?1",
        params![job_id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(added, 3);
}

#[test]
fn test_second_run_with_no_changes_skips_all_files() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "a.txt", "hello");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    scan(&conn, root.id, &root_dir).unwrap();

    let job_id2 = scan(&conn, root.id, &root_dir).unwrap();
    let (added, updated): (i64, i64) = conn.query_row(
        "SELECT files_added, files_updated FROM index_jobs WHERE id = ?1",
        params![job_id2],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap();
    assert_eq!(added, 0);
    assert_eq!(updated, 0);
}

#[test]
fn test_changed_file_is_reindexed() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "a.txt", "original content");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    scan(&conn, root.id, &root_dir).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(10));
    write_file(&root_dir, "a.txt", "changed content");

    let job_id2 = scan(&conn, root.id, &root_dir).unwrap();
    let updated: i64 = conn.query_row(
        "SELECT files_updated FROM index_jobs WHERE id = ?1",
        params![job_id2],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(updated, 1);
}

#[test]
fn test_deleted_file_is_soft_deleted() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "a.txt", "hello");
    write_file(&root_dir, "b.txt", "world");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    scan(&conn, root.id, &root_dir).unwrap();

    std::fs::remove_file(root_dir.join("b.txt")).unwrap();

    let job_id2 = scan(&conn, root.id, &root_dir).unwrap();
    let deleted: i64 = conn.query_row(
        "SELECT files_deleted FROM index_jobs WHERE id = ?1",
        params![job_id2],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(deleted, 1);

    let active: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE root_id = ?1 AND deleted_at IS NULL",
        params![root.id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(active, 1);
}

#[test]
fn test_renamed_file_detected_as_move() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "old.txt", "stable content");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    scan(&conn, root.id, &root_dir).unwrap();

    std::fs::rename(root_dir.join("old.txt"), root_dir.join("new.txt")).unwrap();

    let job_id2 = scan(&conn, root.id, &root_dir).unwrap();
    let (moved, added, deleted): (i64, i64, i64) = conn.query_row(
        "SELECT files_moved, files_added, files_deleted FROM index_jobs WHERE id = ?1",
        params![job_id2],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).unwrap();
    assert_eq!(moved, 1);
    assert_eq!(added, 0);
    assert_eq!(deleted, 0);
}

#[test]
fn test_unknown_extension_is_skipped() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "a.txt", "hello");
    write_file(&root_dir, "a.bin", "binary");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    scan(&conn, root.id, &root_dir).unwrap();

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE root_id = ?1 AND deleted_at IS NULL",
        params![root.id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn test_hidden_file_is_skipped() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "visible.txt", "hello");
    write_file(&root_dir, ".hidden.txt", "secret");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    scan(&conn, root.id, &root_dir).unwrap();

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE root_id = ?1 AND deleted_at IS NULL",
        params![root.id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn test_interrupted_job_marked_on_next_run() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    let now = db::unix_now();
    conn.execute(
        "INSERT INTO index_jobs (root_id, status, index_marker, started_at, updated_at) VALUES (?1, 'running', 1, ?2, ?2)",
        params![root.id, now],
    ).unwrap();

    scan(&conn, root.id, &root_dir).unwrap();

    let interrupted: i64 = conn.query_row(
        "SELECT COUNT(*) FROM index_jobs WHERE status = 'interrupted'",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(interrupted, 1);
}

#[test]
fn test_txt_files_have_extracted_text_after_scan() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "notes.txt", "the quick brown fox jumps over the lazy dog");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    scan(&conn, root.id, &root_dir).unwrap();

    let (text, model_ver): (String, String) = conn.query_row(
        "SELECT extracted_text, model_version FROM files WHERE root_id = ?1 AND deleted_at IS NULL",
        params![root.id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap();
    assert!(!text.is_empty(), "extracted_text should be populated after scan");
    assert_eq!(model_ver, "ext-v1", "model_version should be set after extraction");
}

#[test]
fn test_chunks_populated_for_txt_file() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "notes.txt", "word ".repeat(10).trim());

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    scan(&conn, root.id, &root_dir).unwrap();

    let file_id: i64 = conn.query_row(
        "SELECT id FROM files WHERE root_id = ?1 AND deleted_at IS NULL",
        params![root.id],
        |r| r.get(0),
    ).unwrap();
    let chunk_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM chunks WHERE file_id = ?1",
        params![file_id],
        |r| r.get(0),
    ).unwrap();
    assert!(chunk_count >= 1, "at least one chunk should be stored");
}

#[test]
fn test_second_scan_skips_extraction_for_unchanged_file() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "a.txt", "stable content");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    scan(&conn, root.id, &root_dir).unwrap();

    conn.execute(
        "UPDATE files SET extracted_text = 'sentinel' WHERE root_id = ?1",
        params![root.id],
    ).unwrap();

    scan(&conn, root.id, &root_dir).unwrap();

    let text: String = conn.query_row(
        "SELECT extracted_text FROM files WHERE root_id = ?1 AND deleted_at IS NULL",
        params![root.id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(text, "sentinel", "extraction should not re-run for unchanged file");
}

#[test]
fn test_large_batch_completes_without_panic() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    for i in 0..500 {
        write_file(&root_dir, &format!("{i}.txt"), &format!("content {i}"));
    }

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    scan(&conn, root.id, &root_dir).unwrap();

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE root_id = ?1 AND deleted_at IS NULL",
        params![root.id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 500);
}

#[test]
fn test_junk_directories_are_not_indexed() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir_all(root_dir.join("node_modules/lodash")).unwrap();
    std::fs::create_dir_all(root_dir.join("dist")).unwrap();
    std::fs::create_dir_all(root_dir.join("src")).unwrap();
    write_file(&root_dir, "README.txt", "top-level readme");
    write_file(&root_dir.join("node_modules/lodash"), "index.txt", "lodash source");
    write_file(&root_dir.join("dist"), "bundle.txt", "minified bundle");
    write_file(&root_dir.join("src"), "main.txt", "application source");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    scan(&conn, root.id, &root_dir).unwrap();

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE root_id = ?1 AND deleted_at IS NULL",
        params![root.id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 2, "expected 2 files; node_modules and dist must be skipped");
}

#[test]
fn test_file_in_code_repo_gets_reduced_confidence() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    let repo_dir = root_dir.join("myrepo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    write_file(&repo_dir, "package.json", r#"{"name":"test"}"#);
    write_file(&repo_dir, "README.txt", "project readme");
    write_file(&root_dir, "standalone.txt", "standalone document");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    scan(&conn, root.id, &root_dir).unwrap();

    let rows: Vec<(String, f32)> = {
        let mut stmt = conn.prepare(
            "SELECT rel_path, confidence FROM files WHERE root_id = ?1 AND deleted_at IS NULL ORDER BY rel_path",
        ).unwrap();
        stmt.query_map(params![root.id], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };

    let readme = rows.iter().find(|(p, _)| p.contains("README")).expect("README.txt must be indexed");
    let standalone = rows.iter().find(|(p, _)| p.contains("standalone")).expect("standalone.txt must be indexed");

    assert!(
        readme.1 < standalone.1,
        "README in repo (confidence={}) must be lower than standalone doc (confidence={})",
        readme.1, standalone.1,
    );
    assert!(
        (standalone.1 - 1.0_f32).abs() < 1e-5,
        "standalone.txt confidence must be 1.0, got {}",
        standalone.1,
    );
}

// ── NEW: progress channel integration tests ────────────────────────────────────

/// Build a mix of files, run scan with a captured channel, and assert the
/// progress stream has the required shape.
#[test]
fn test_progress_stream_has_event_per_phase() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "a.txt", "alpha content");
    write_file(&root_dir, "b.md", "beta content");
    write_file(&root_dir, "c.csv", "col1,col2\n1,2");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();

    let capture = CaptureSender::default();
    let events_ref = capture.0.clone();
    let reporter = no_throttle_reporter(capture, 0, root.id);
    run_scan(&conn, root.id, &root_dir, "http://localhost:19999", &no_cancel(), &no_emit, reporter).unwrap();

    let events = events_ref.lock().unwrap().clone();

    // At least one event per phase.
    let has_discovering = events.iter().any(|e| e.phase == Phase::Discovering);
    let has_fingerprinting = events.iter().any(|e| e.phase == Phase::Fingerprinting);
    let has_extracting = events.iter().any(|e| e.phase == Phase::Extracting);
    assert!(has_discovering, "expected at least one Discovering event");
    assert!(has_fingerprinting, "expected at least one Fingerprinting event");
    assert!(has_extracting, "expected at least one Extracting event");
}

#[test]
fn test_progress_stream_sequence_numbers_are_strictly_increasing() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "a.txt", "hello");
    write_file(&root_dir, "b.txt", "world");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();

    let capture = CaptureSender::default();
    let events_ref = capture.0.clone();
    let reporter = no_throttle_reporter(capture, 0, root.id);
    run_scan(&conn, root.id, &root_dir, "http://localhost:19999", &no_cancel(), &no_emit, reporter).unwrap();

    let events = events_ref.lock().unwrap().clone();
    assert!(!events.is_empty(), "expected at least one event");

    let seqs: Vec<u64> = events.iter().map(|e| e.seq).collect();
    for window in seqs.windows(2) {
        assert!(window[1] > window[0], "seq must be strictly increasing: {:?}", window);
    }
}

#[test]
fn test_progress_stream_extraction_events_carry_current_file() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "notes.txt", "some text content");
    write_file(&root_dir, "readme.md", "# readme");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();

    let capture = CaptureSender::default();
    let events_ref = capture.0.clone();
    let reporter = no_throttle_reporter(capture, 0, root.id);
    // Port 19999 is guaranteed unreachable — embeddings will fail but extraction proceeds.
    run_scan(&conn, root.id, &root_dir, "http://localhost:19999", &no_cancel(), &no_emit, reporter).unwrap();

    let events = events_ref.lock().unwrap().clone();

    // Every Extracting event that has a Some(current_file) should name one of our files.
    let extracting_with_file: Vec<_> = events.iter()
        .filter(|e| e.phase == Phase::Extracting && e.current_file.is_some())
        .collect();

    assert!(
        !extracting_with_file.is_empty(),
        "expected at least one Extracting event with current_file set"
    );

    for ev in &extracting_with_file {
        let cf = ev.current_file.as_deref().unwrap();
        assert!(
            cf.contains("notes") || cf.contains("readme"),
            "current_file '{}' should match one of the fixture files",
            cf
        );
    }
}

#[test]
fn test_progress_stream_extraction_total_populated_before_extraction() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "a.txt", "first");
    write_file(&root_dir, "b.txt", "second");
    write_file(&root_dir, "c.txt", "third");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();

    let capture = CaptureSender::default();
    let events_ref = capture.0.clone();
    let reporter = no_throttle_reporter(capture, 0, root.id);
    run_scan(&conn, root.id, &root_dir, "http://localhost:19999", &no_cancel(), &no_emit, reporter).unwrap();

    let events = events_ref.lock().unwrap().clone();

    // The first Extracting event must have extraction_total > 0.
    let first_extracting = events.iter().find(|e| e.phase == Phase::Extracting);
    assert!(first_extracting.is_some(), "expected at least one Extracting event");
    assert!(
        first_extracting.unwrap().extraction_total > 0,
        "extraction_total must be set before the first extracting event"
    );
}

#[test]
fn test_progress_stream_small_folder_still_produces_events() {
    // Regression: the old % 50 throttle meant folders < 50 files got zero events.
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    // Only 3 files — well below the old % 50 threshold.
    write_file(&root_dir, "one.txt", "one");
    write_file(&root_dir, "two.txt", "two");
    write_file(&root_dir, "three.txt", "three");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();

    let capture = CaptureSender::default();
    let events_ref = capture.0.clone();
    let reporter = no_throttle_reporter(capture, 0, root.id);
    run_scan(&conn, root.id, &root_dir, "http://localhost:19999", &no_cancel(), &no_emit, reporter).unwrap();

    let events = events_ref.lock().unwrap();
    assert!(
        events.len() >= 3,
        "expected at least 3 progress events for 3 files, got {}",
        events.len()
    );
}

#[test]
fn test_progress_stream_final_event_has_correct_counts() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "a.txt", "alpha");
    write_file(&root_dir, "b.txt", "beta");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();

    let capture = CaptureSender::default();
    let events_ref = capture.0.clone();
    let reporter = no_throttle_reporter(capture, 0, root.id);
    run_scan(&conn, root.id, &root_dir, "http://localhost:19999", &no_cancel(), &no_emit, reporter).unwrap();

    let events = events_ref.lock().unwrap().clone();

    // The last Extracting event's extraction_done should equal extraction_total.
    let last_extracting = events.iter().filter(|e| e.phase == Phase::Extracting).last();
    if let Some(last) = last_extracting {
        assert_eq!(
            last.extraction_done, last.extraction_total,
            "final extracting event: extraction_done ({}) should equal extraction_total ({})",
            last.extraction_done, last.extraction_total,
        );
    }
}
