use super::*;
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

#[test]
fn test_first_run_indexes_all_txt_files() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "a.txt", "hello");
    write_file(&root_dir, "b.txt", "world");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    let job_id = run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

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
    let job_id = run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

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
    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

    let job_id2 = run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();
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
    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(10));
    write_file(&root_dir, "a.txt", "changed content");

    let job_id2 = run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();
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
    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

    std::fs::remove_file(root_dir.join("b.txt")).unwrap();

    let job_id2 = run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();
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
    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

    std::fs::rename(root_dir.join("old.txt"), root_dir.join("new.txt")).unwrap();

    let job_id2 = run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();
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
    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

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
    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

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

    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

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
    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

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
    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

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
    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

    conn.execute(
        "UPDATE files SET extracted_text = 'sentinel' WHERE root_id = ?1",
        params![root.id],
    ).unwrap();

    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

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
    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

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
    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE root_id = ?1 AND deleted_at IS NULL",
        params![root.id],
        |r| r.get(0),
    ).unwrap();
    // Only README.txt and src/main.txt should be indexed; node_modules and dist are excluded.
    assert_eq!(count, 2, "expected 2 files; node_modules and dist must be skipped");
}

#[test]
fn test_file_in_code_repo_gets_reduced_confidence() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    let repo_dir = root_dir.join("myrepo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    // Mark repo_dir as a code repository root via package.json.
    write_file(&repo_dir, "package.json", r#"{"name":"test"}"#);
    write_file(&repo_dir, "README.txt", "project readme");

    // A standalone file at the root level (no repo marker in ancestors).
    write_file(&root_dir, "standalone.txt", "standalone document");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

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

#[derive(Default)]
struct CapturedEvents {
    inner: std::sync::Mutex<Vec<(String, serde_json::Value)>>,
}

impl CapturedEvents {
    fn push(&self, event: String, payload: serde_json::Value) {
        self.inner.lock().unwrap().push((event, payload));
    }

    fn progress(&self) -> Vec<serde_json::Value> {
        self.inner
            .lock()
            .unwrap()
            .iter()
            .filter(|(e, _)| e == "indexing://progress")
            .map(|(_, p)| p.clone())
            .collect()
    }
}

fn recording_emit() -> (impl Fn(&str, &serde_json::Value), Arc<CapturedEvents>) {
    let events = Arc::new(CapturedEvents::default());
    let captured = events.clone();
    let f = move |event: &str, payload: &serde_json::Value| {
        captured.push(event.to_string(), payload.clone());
    };
    (f, events)
}

#[test]
fn test_index_jobs_table_records_total_and_done_for_small_run() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "a.txt", "alpha");
    write_file(&root_dir, "b.md", "bravo");
    write_file(&root_dir, "c.csv", "charlie");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    let job_id = run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

    let (total, done): (i64, i64) = conn.query_row(
        "SELECT files_total, files_done FROM index_jobs WHERE id = ?1",
        params![job_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap();

    assert_eq!(total, 3);
    assert_eq!(done, 3);
}

#[test]
fn test_progress_event_includes_current_file_during_extraction() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "alpha.txt", "alpha content");
    write_file(&root_dir, "bravo.md", "bravo content");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    let (emit, events) = recording_emit();
    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &emit).unwrap();

    let extracting: Vec<_> = events.progress().into_iter()
        .filter(|p| p.get("phase").and_then(|v| v.as_str()) == Some("extracting"))
        .collect();

    assert!(!extracting.is_empty(), "extraction phase must emit progress");
    let has_current_file = extracting.iter().any(|p| {
        matches!(p.get("currentFile").and_then(|v| v.as_str()), Some("alpha.txt") | Some("bravo.md"))
    });
    assert!(has_current_file, "extraction events must carry currentFile, got: {extracting:?}");
}

#[test]
fn test_extraction_phase_writes_cursor_path_to_index_jobs() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "notes.txt", "content for extraction");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    let job_id = run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &no_emit).unwrap();

    let cursor: Option<String> = conn.query_row(
        "SELECT cursor_path FROM index_jobs WHERE id = ?1",
        params![job_id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(cursor.as_deref(), Some("notes.txt"));
}

#[test]
fn test_progress_events_include_error_count_field() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    write_file(&root_dir, "a.txt", "hello");

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    let (emit, events) = recording_emit();
    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &emit).unwrap();

    let progress = events.progress();
    assert!(!progress.is_empty());
    for evt in &progress {
        assert!(evt.get("errorCount").is_some(), "missing errorCount in {evt:?}");
    }
}

#[test]
fn test_initial_event_reflects_real_total_before_extraction() {
    let (tmp, conn) = setup_db();
    let root_dir = tmp.path().join("root");
    std::fs::create_dir(&root_dir).unwrap();
    for i in 0..5 {
        write_file(&root_dir, &format!("{i}.txt"), &format!("file {i}"));
    }

    let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
    let (emit, events) = recording_emit();
    run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_cancel(), &emit).unwrap();

    let first_extracting = events.progress().into_iter()
        .find(|p| p.get("phase").and_then(|v| v.as_str()) == Some("extracting"))
        .expect("at least one extracting event");
    let total = first_extracting.get("filesTotal").and_then(|v| v.as_i64()).unwrap_or(-1);
    assert_eq!(total, 5, "filesTotal must be set by the time extraction starts, got: {first_extracting:?}");
}
