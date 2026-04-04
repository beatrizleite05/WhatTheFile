use rusqlite::Connection;
#[cfg(test)]
use rusqlite::params;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use tauri::Emitter;
use crate::{chunker, db, errors::AppError, extractor};

// ── helpers ──────────────────────────────────────────────────────────────────

const MAX_FILE_SIZE_BYTES: i64 = 100 * 1024 * 1024; // 100 MB

fn detect_media_type(filename: &str) -> Option<&'static str> {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    match ext.as_deref() {
        Some("pdf")  => Some("pdf"),
        Some("docx") => Some("docx"),
        Some("xlsx") => Some("xlsx"),
        Some("csv")  => Some("csv"),
        Some("txt")  => Some("txt"),
        Some("md")   => Some("md"),
        Some("png")  => Some("png"),
        Some("jpg") | Some("jpeg") => Some("jpg"),
        _ => None,
    }
}

fn get_mtime_ns(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

struct FileCandidate {
    path: PathBuf,
    rel_path: String,
    filename: String,
    media_type: &'static str,
    size_bytes: i64,
    mtime_ns: i64,
    previously_existed: bool,
}

// ── public API ────────────────────────────────────────────────────────────────

pub fn run(
    app: &tauri::AppHandle,
    db_path: &Path,
    root_id: i64,
    ollama_url: &str,
) -> Result<i64, AppError> {
    let conn = db::open_and_migrate(db_path)?;
    let root = db::find_root_by_id(&conn, root_id)?
        .ok_or_else(|| AppError::Indexer(format!("root {root_id} not found")))?;
    let root_path = PathBuf::from(&root.path);
    let app = app.clone();
    run_scan(&conn, root_id, &root_path, ollama_url, &|event, payload| {
        let _ = app.emit(event, payload);
    })
}

// ── internal (testable) ───────────────────────────────────────────────────────

fn run_scan(
    conn: &Connection,
    root_id: i64,
    root_path: &Path,
    ollama_url: &str,
    emit: &dyn Fn(&str, &serde_json::Value),
) -> Result<i64, AppError> {
    // 1. Startup recovery
    db::recover_interrupted_jobs(conn)?;

    // 2. Create index job
    let now = db::unix_now();
    // Use nanosecond precision for the marker so that two back-to-back runs
    // within the same wall-clock second still get distinct marker values,
    // which is required for the soft-delete sweep to work correctly.
    let marker = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i64;
    let job_id = db::insert_job(conn, root_id, marker, now)?;
    db::log_activity(conn, "job_started", Some(root_id), None, Some(job_id), None, now)?;

    let mut counts = db::JobCounts::default();

    emit("indexing://progress", &serde_json::json!({
        "jobId": job_id, "rootId": root_id, "phase": "discovering",
        "filesTotal": 0, "filesDone": 0, "filesAdded": 0,
        "filesUpdated": 0, "filesMoved": 0, "filesDeleted": 0,
    }));

    // 3. Fast pass — collect candidates that need fingerprinting
    let mut candidates: Vec<FileCandidate> = Vec::new();

    for entry in WalkDir::new(root_path)
        .into_iter()
        .filter_entry(|e| {
            // Skip hidden directories (but allow walking from root itself)
            if e.depth() > 0 {
                if let Some(name) = e.file_name().to_str() {
                    if name.starts_with('.') {
                        return false;
                    }
                }
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path().to_path_buf();
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Skip hidden files
        if filename.starts_with('.') {
            continue;
        }

        // Skip unknown media types
        let media_type = match detect_media_type(&filename) {
            Some(mt) => mt,
            None => continue,
        };

        // Metadata
        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => { counts.error_count += 1; continue; }
        };
        let size_bytes = metadata.len() as i64;

        // Skip oversized files
        if size_bytes > MAX_FILE_SIZE_BYTES {
            continue;
        }

        let mtime_ns = get_mtime_ns(&metadata);
        let rel_path = path
            .strip_prefix(root_path)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        counts.files_total += 1;

        // Fast pass: mtime + size check
        let existing_record = db::find_file_by_path(conn, root_id, &rel_path)?;
        if let Some(ref existing) = existing_record {
            if existing.mtime_ns == mtime_ns && existing.size_bytes == size_bytes {
                db::stamp_index_marker(conn, existing.id, marker, db::unix_now())?;
                counts.files_done += 1;
                if counts.files_done % 50 == 0 {
                    db::update_job_counts(conn, job_id, &counts, db::unix_now())?;
                    emit("indexing://progress", &serde_json::json!({
                        "jobId": job_id, "rootId": root_id, "phase": "discovering",
                        "filesTotal": counts.files_total, "filesDone": counts.files_done,
                        "filesAdded": counts.files_added, "filesUpdated": counts.files_updated,
                        "filesMoved": counts.files_moved, "filesDeleted": counts.files_deleted,
                    }));
                }
                continue;
            }
        }
        let previously_existed = existing_record.is_some();

        candidates.push(FileCandidate { path, rel_path, filename, media_type, size_bytes, mtime_ns, previously_existed });
    }

    // Update phase
    db::update_job_phase(conn, job_id, "fingerprinting", db::unix_now())?;
    emit("indexing://progress", &serde_json::json!({
        "jobId": job_id, "rootId": root_id, "phase": "fingerprinting",
        "filesTotal": counts.files_total, "filesDone": counts.files_done,
        "filesAdded": counts.files_added, "filesUpdated": counts.files_updated,
        "filesMoved": counts.files_moved, "filesDeleted": counts.files_deleted,
    }));

    // 4. Fingerprint pass
    for candidate in candidates {
        let bytes = match std::fs::read(&candidate.path) {
            Ok(b) => b,
            Err(_) => { counts.error_count += 1; continue; }
        };
        let fingerprint = blake3::hash(&bytes).to_hex().to_string();
        let now = db::unix_now();

        // Move detection: fingerprint found at a different rel_path
        if let Some(existing_fp) = db::find_file_by_fingerprint(conn, root_id, &fingerprint)? {
            if existing_fp.rel_path != candidate.rel_path {
                db::move_file(
                    conn,
                    existing_fp.id,
                    &candidate.rel_path,
                    &candidate.filename,
                    candidate.mtime_ns,
                    marker,
                    now,
                )?;
                counts.files_moved += 1;
                counts.files_done += 1;
                if counts.files_done % 50 == 0 {
                    db::update_job_counts(conn, job_id, &counts, now)?;
                }
                continue;
            }
            // Same path, same content (mtime drifted) — stamp so sweep keeps it.
            // If model_version is empty, extraction has not run yet; the extraction pass will pick this file up via find_files_needing_extraction.
            db::stamp_index_marker(conn, existing_fp.id, marker, now)?;
            counts.files_done += 1;
            continue;
        }

        // New or changed file
        let previously_existed = candidate.previously_existed;
        db::upsert_file_metadata(
            conn,
            root_id,
            &candidate.rel_path,
            &candidate.filename,
            candidate.media_type,
            candidate.size_bytes,
            candidate.mtime_ns,
            &fingerprint,
            "", // model_version: filled by Phase C
            marker,
            now,
        )?;

        if previously_existed {
            counts.files_updated += 1;
        } else {
            counts.files_added += 1;
        }
        counts.files_done += 1;

        if counts.files_done % 50 == 0 {
            db::update_job_counts(conn, job_id, &counts, now)?;
            emit("indexing://progress", &serde_json::json!({
                "jobId": job_id, "rootId": root_id, "phase": "fingerprinting",
                "filesTotal": counts.files_total, "filesDone": counts.files_done,
                "filesAdded": counts.files_added, "filesUpdated": counts.files_updated,
                "filesMoved": counts.files_moved, "filesDeleted": counts.files_deleted,
            }));
        }
    }

    // 5. Extraction pass — process files whose model_version is still empty.
    db::update_job_phase(conn, job_id, "extracting", db::unix_now())?;
    emit("indexing://progress", &serde_json::json!({
        "jobId": job_id, "rootId": root_id, "phase": "extracting",
        "filesTotal": counts.files_total, "filesDone": counts.files_done,
        "filesAdded": counts.files_added, "filesUpdated": counts.files_updated,
        "filesMoved": counts.files_moved, "filesDeleted": counts.files_deleted,
    }));

    let pending = db::find_files_needing_extraction(conn, root_id)?;
    for (file_id, rel_path, _media_type) in pending {
        let abs_path = root_path.join(&rel_path);
        let now = db::unix_now();
        match extractor::extract(&abs_path, ollama_url) {
            Ok(result) => {
                if result.text.is_empty() {
                    // OCR below confidence threshold — leave model_version empty so
                    // a future run can retry (e.g. after Tesseract data is updated).
                    counts.error_count += 1;
                    continue;
                }
                let chunks = chunker::chunk_text(
                    &result.text,
                    chunker::CHUNK_SIZE,
                    chunker::CHUNK_OVERLAP,
                );
                let chunk_pairs: Vec<(usize, &str)> = chunks
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i, c.text.as_str()))
                    .collect();
                db::update_file_content(
                    conn,
                    file_id,
                    &result.text,
                    result.confidence,
                    &result.lang_hint,
                    "ext-v1",
                    now,
                )?;
                db::replace_chunks(conn, file_id, &chunk_pairs)?;
            }
            Err(_) => {
                counts.error_count += 1;
            }
        }
    }

    // 6. Soft-delete sweep
    let now = db::unix_now();
    let deleted = db::sweep_deleted_files(conn, root_id, marker, now)?;
    counts.files_deleted = deleted;

    // 7. Complete job
    db::complete_job(conn, job_id, &counts, now)?;
    db::update_root_last_indexed(conn, root_id, now)?;
    db::log_activity(conn, "job_completed", Some(root_id), None, Some(job_id), None, now)?;

    emit("indexing://completed", &serde_json::json!({
        "jobId": job_id, "rootId": root_id,
        "filesTotal": counts.files_total,
        "filesAdded": counts.files_added,
        "filesUpdated": counts.files_updated,
        "filesMoved": counts.files_moved,
        "filesDeleted": counts.files_deleted,
        "errorCount": counts.error_count,
    }));

    Ok(job_id)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn test_first_run_indexes_all_txt_files() {
        let (tmp, conn) = setup_db();
        let root_dir = tmp.path().join("root");
        std::fs::create_dir(&root_dir).unwrap();
        write_file(&root_dir, "a.txt", "hello");
        write_file(&root_dir, "b.txt", "world");

        let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
        let job_id = run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();

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
        let job_id = run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();

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
        run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();

        // Second run
        let job_id2 = run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();
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
        run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();

        // Overwrite with new content (OS will update mtime)
        std::thread::sleep(std::time::Duration::from_millis(10));
        write_file(&root_dir, "a.txt", "changed content");

        let job_id2 = run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();
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
        run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();

        std::fs::remove_file(root_dir.join("b.txt")).unwrap();

        let job_id2 = run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();
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
        run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();

        std::fs::rename(root_dir.join("old.txt"), root_dir.join("new.txt")).unwrap();

        let job_id2 = run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();
        let (moved, added, deleted): (i64, i64, i64) = conn.query_row(
            "SELECT files_moved, files_added, files_deleted FROM index_jobs WHERE id = ?1",
            params![job_id2],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).unwrap();
        assert_eq!(moved, 1);
        assert_eq!(added, 0);   // not treated as a new file
        assert_eq!(deleted, 0); // not treated as deleted
    }

    #[test]
    fn test_unknown_extension_is_skipped() {
        // Files with unknown extensions (e.g. .bin) are filtered out by
        // detect_media_type; only files with recognised extensions are indexed.
        let (tmp, conn) = setup_db();
        let root_dir = tmp.path().join("root");
        std::fs::create_dir(&root_dir).unwrap();
        write_file(&root_dir, "a.txt", "hello");
        // Create a .bin file (unknown media type — skipped)
        write_file(&root_dir, "a.bin", "binary");

        let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
        run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM files WHERE root_id = ?1 AND deleted_at IS NULL",
            params![root.id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1); // only a.txt
    }

    #[test]
    fn test_hidden_file_is_skipped() {
        let (tmp, conn) = setup_db();
        let root_dir = tmp.path().join("root");
        std::fs::create_dir(&root_dir).unwrap();
        write_file(&root_dir, "visible.txt", "hello");
        write_file(&root_dir, ".hidden.txt", "secret");

        let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
        run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();

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
        // Simulate a crashed previous job
        let now = db::unix_now();
        conn.execute(
            "INSERT INTO index_jobs (root_id, status, index_marker, started_at, updated_at) VALUES (?1, 'running', 1, ?2, ?2)",
            params![root.id, now],
        ).unwrap();

        run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();

        let interrupted: i64 = conn.query_row(
            "SELECT COUNT(*) FROM index_jobs WHERE status = 'interrupted'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(interrupted, 1);
    }

    // ── Phase C extraction integration tests ─────────────────────────────────

    #[test]
    fn test_txt_files_have_extracted_text_after_scan() {
        let (tmp, conn) = setup_db();
        let root_dir = tmp.path().join("root");
        std::fs::create_dir(&root_dir).unwrap();
        write_file(&root_dir, "notes.txt", "the quick brown fox jumps over the lazy dog");

        let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
        run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();

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
        // Write enough content to produce at least one chunk.
        write_file(&root_dir, "notes.txt", "word ".repeat(10).trim());

        let root = db::insert_root(&conn, root_dir.to_str().unwrap()).unwrap();
        run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();

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
        run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();

        // Overwrite extracted_text to a sentinel so we can detect if re-extraction ran.
        conn.execute(
            "UPDATE files SET extracted_text = 'sentinel' WHERE root_id = ?1",
            params![root.id],
        ).unwrap();

        // Second scan — file unchanged, should NOT re-extract.
        run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();

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
        run_scan(&conn, root.id, &root_dir, "http://localhost:11434", &no_emit).unwrap();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM files WHERE root_id = ?1 AND deleted_at IS NULL",
            params![root.id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 500);
    }
}
