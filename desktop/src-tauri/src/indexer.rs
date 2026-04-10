use rusqlite::Connection;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use tauri::Emitter;
use crate::{chunker, db, errors::AppError, extractor, llm};

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
    db::recover_interrupted_jobs(conn)?;

    let now = db::unix_now();
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

    let candidates = run_discovery(conn, root_id, root_path, marker, job_id, &mut counts, emit)?;

    db::update_job_phase(conn, job_id, "fingerprinting", db::unix_now())?;
    emit("indexing://progress", &serde_json::json!({
        "jobId": job_id, "rootId": root_id, "phase": "fingerprinting",
        "filesTotal": counts.files_total, "filesDone": counts.files_done,
        "filesAdded": counts.files_added, "filesUpdated": counts.files_updated,
        "filesMoved": counts.files_moved, "filesDeleted": counts.files_deleted,
    }));

    run_fingerprinting(conn, root_id, marker, job_id, candidates, &mut counts, emit)?;

    db::update_job_phase(conn, job_id, "extracting", db::unix_now())?;
    emit("indexing://progress", &serde_json::json!({
        "jobId": job_id, "rootId": root_id, "phase": "extracting",
        "filesTotal": counts.files_total, "filesDone": counts.files_done,
        "filesAdded": counts.files_added, "filesUpdated": counts.files_updated,
        "filesMoved": counts.files_moved, "filesDeleted": counts.files_deleted,
    }));

    run_extraction(conn, root_id, root_path, ollama_url, &mut counts)?;

    let now = db::unix_now();
    counts.files_deleted = db::sweep_deleted_files(conn, root_id, marker, now)?;

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

    // Release VRAM immediately after indexing completes.
    match llm::runtime::unload_model("nomic-embed-text-v2-moe", ollama_url) {
        Ok(()) => log::info!("unloaded embedding model (nomic-embed-text-v2-moe)"),
        Err(e) => log::warn!("unload embedding model failed (will remain in VRAM): {e}"),
    }

    Ok(job_id)
}

/// Walk the filesystem and collect files that need fingerprinting.
/// Files whose mtime+size are unchanged are stamped and skipped immediately.
fn run_discovery(
    conn: &Connection,
    root_id: i64,
    root_path: &Path,
    marker: i64,
    job_id: i64,
    counts: &mut db::JobCounts,
    emit: &dyn Fn(&str, &serde_json::Value),
) -> Result<Vec<FileCandidate>, AppError> {
    let mut candidates: Vec<FileCandidate> = Vec::new();

    for entry in WalkDir::new(root_path)
        .into_iter()
        .filter_entry(|e| {
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

        if filename.starts_with('.') {
            continue;
        }

        let media_type = match detect_media_type(&filename) {
            Some(mt) => mt,
            None => continue,
        };

        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => { counts.error_count += 1; continue; }
        };
        let size_bytes = metadata.len() as i64;

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

        let existing_record = db::find_file_by_path(conn, root_id, &rel_path)?;
        if let Some(ref existing) = existing_record {
            if existing.mtime_ns == mtime_ns && existing.size_bytes == size_bytes {
                db::stamp_index_marker(conn, existing.id, marker, db::unix_now())?;
                counts.files_done += 1;
                if counts.files_done % 50 == 0 {
                    db::update_job_counts(conn, job_id, counts, db::unix_now())?;
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

    Ok(candidates)
}

/// Hash each candidate and upsert/move records as appropriate.
fn run_fingerprinting(
    conn: &Connection,
    root_id: i64,
    marker: i64,
    job_id: i64,
    candidates: Vec<FileCandidate>,
    counts: &mut db::JobCounts,
    emit: &dyn Fn(&str, &serde_json::Value),
) -> Result<(), AppError> {
    for candidate in candidates {
        let bytes = match std::fs::read(&candidate.path) {
            Ok(b) => b,
            Err(_) => { counts.error_count += 1; continue; }
        };
        let fingerprint = blake3::hash(&bytes).to_hex().to_string();
        let now = db::unix_now();

        if let Some(existing_fp) = db::find_file_by_fingerprint(conn, root_id, &fingerprint)? {
            if existing_fp.rel_path != candidate.rel_path {
                db::move_file(conn, existing_fp.id, &candidate.rel_path, &candidate.filename, candidate.mtime_ns, marker, now)?;
                counts.files_moved += 1;
                counts.files_done += 1;
                if counts.files_done % 50 == 0 {
                    db::update_job_counts(conn, job_id, counts, now)?;
                }
                continue;
            }
            // Same path, same content (mtime drifted) — stamp so sweep keeps it.
            db::stamp_index_marker(conn, existing_fp.id, marker, now)?;
            counts.files_done += 1;
            continue;
        }

        db::upsert_file_metadata(
            conn,
            root_id,
            &candidate.rel_path,
            &candidate.filename,
            candidate.media_type,
            candidate.size_bytes,
            candidate.mtime_ns,
            &fingerprint,
            "", // model_version: filled by extraction phase
            marker,
            now,
        )?;

        if candidate.previously_existed {
            counts.files_updated += 1;
        } else {
            counts.files_added += 1;
        }
        counts.files_done += 1;

        if counts.files_done % 50 == 0 {
            db::update_job_counts(conn, job_id, counts, now)?;
            emit("indexing://progress", &serde_json::json!({
                "jobId": job_id, "rootId": root_id, "phase": "fingerprinting",
                "filesTotal": counts.files_total, "filesDone": counts.files_done,
                "filesAdded": counts.files_added, "filesUpdated": counts.files_updated,
                "filesMoved": counts.files_moved, "filesDeleted": counts.files_deleted,
            }));
        }
    }
    Ok(())
}

/// Extract text and embed chunks for files whose model_version is still empty.
fn run_extraction(
    conn: &Connection,
    root_id: i64,
    root_path: &Path,
    ollama_url: &str,
    counts: &mut db::JobCounts,
) -> Result<(), AppError> {
    let pending = db::find_files_needing_extraction(conn, root_id)?;
    for (file_id, rel_path, _media_type) in pending {
        let abs_path = root_path.join(&rel_path);
        let now = db::unix_now();
        match extractor::extract(&abs_path, ollama_url) {
            Ok(result) => {
                if result.text.is_empty() {
                    counts.error_count += 1;
                    continue;
                }
                let chunks = chunker::chunk_text(&result.text, chunker::CHUNK_SIZE, chunker::CHUNK_OVERLAP);
                let chunk_texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
                let embeddings_result = llm::embeddings::embed_texts(&chunk_texts, ollama_url);
                let embedding_blobs: Vec<Option<Vec<u8>>> = match embeddings_result {
                    Ok(vecs) => vecs.into_iter().map(|v| Some(llm::embeddings::embedding_to_bytes(&v))).collect(),
                    Err(_) => vec![None; chunks.len()],
                };
                let chunk_pairs: Vec<(usize, &str, Option<&[u8]>)> = chunks
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i, c.text.as_str(), embedding_blobs[i].as_deref()))
                    .collect();

                db::update_file_content(conn, file_id, &result.text, result.confidence, &result.lang_hint, "ext-v1", now)?;
                db::replace_chunks(conn, file_id, &chunk_pairs)?;
            }
            Err(_) => {
                counts.error_count += 1;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "indexer_test.rs"]
mod tests;
