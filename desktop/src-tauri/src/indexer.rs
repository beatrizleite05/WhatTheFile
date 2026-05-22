use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};
use walkdir::WalkDir;
use tauri::Emitter;
use crate::{chunker, db, errors::AppError, extractor, llm};

const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(250);

// ── helpers ──────────────────────────────────────────────────────────────────

// Configurable policy (allowedExtensions, excludeGlobs, maxFileSizeBytes, includeHidden) is
// hardcoded here pending DB schema + Rust implementation. The intended contract is specified
// in src/core/policy.ts and enforced by the tests in test/policy.spec.ts.
const MAX_FILE_SIZE_BYTES: i64 = 100 * 1024 * 1024; // 100 MB

// Directories whose contents are never useful to index. Matched by directory name only.
const JUNK_DIRS: &[&str] = &[
    "node_modules", ".git", ".svn", ".hg",
    "dist", "build", "out", "target", "coverage",
    "vendor", ".cache", "__pycache__",
    ".next", ".nuxt", ".svelte-kit",
    "venv", ".venv", "env", "site-packages",
    ".eggs", ".pytest_cache", ".mypy_cache",
];

// Presence of any of these files/dirs in a directory marks it as a code repository root.
// Files within any ancestor that is a code repo receive a reduced confidence score.
const REPO_MARKERS: &[&str] = &[
    ".git", "package.json", "Cargo.toml", "go.mod",
    "pyproject.toml", "setup.py", "composer.json",
    "pom.xml", "build.gradle", "build.gradle.kts",
    "Makefile", "CMakeLists.txt",
];

// Confidence multiplier applied to files found inside a code repository tree.
// A README.md in a repo still appears in results but ranks below standalone documents.
const REPO_CONFIDENCE_FACTOR: f32 = 0.5;

fn detect_media_type(filename: &str) -> Option<&'static str> {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    match ext.as_deref() {
        Some("pdf")  => Some("pdf"),
        Some("docx") => Some("docx"),
        Some("xlsx") => Some("xlsx"),
        Some("xlsm") => Some("xlsx"),
        Some("csv")  => Some("csv"),
        Some("txt")  => Some("txt"),
        Some("md")   => Some("md"),
        Some("png")  => Some("png"),
        Some("jpg") | Some("jpeg") => Some("jpg"),
        Some("webp") => Some("webp"),
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

/// Returns true if `dir` contains any file or directory that is a repo marker.
fn is_repo_root(dir: &Path) -> bool {
    REPO_MARKERS.iter().any(|marker| dir.join(marker).exists())
}

/// Walks ancestors of `file_path` up to (but not including) `root` and returns
/// true if any ancestor directory contains a repo marker.
fn file_is_in_code_repo(file_path: &Path, root: &Path) -> bool {
    let mut dir = file_path.parent().unwrap_or(root);
    loop {
        if is_repo_root(dir) {
            return true;
        }
        if dir == root || dir.parent().is_none() {
            break;
        }
        // Unwrap is safe: we checked parent().is_none() above.
        dir = dir.parent().unwrap();
        // Stop if we've gone above the root.
        if !dir.starts_with(root) {
            break;
        }
    }
    false
}

struct ProgressEmitter<'a> {
    job_id: i64,
    root_id: i64,
    emit: &'a dyn Fn(&str, &serde_json::Value),
    last_emit: Instant,
    extraction_total: i64,
    extraction_done: i64,
    last_current_file: Option<String>,
}

impl<'a> ProgressEmitter<'a> {
    fn new(job_id: i64, root_id: i64, emit: &'a dyn Fn(&str, &serde_json::Value)) -> Self {
        Self {
            job_id,
            root_id,
            emit,
            last_emit: Instant::now() - PROGRESS_EMIT_INTERVAL,
            extraction_total: 0,
            extraction_done: 0,
            last_current_file: None,
        }
    }

    fn set_extraction_total(&mut self, total: i64) {
        self.extraction_total = total;
        self.extraction_done = 0;
    }

    fn advance_extraction(&mut self) {
        self.extraction_done += 1;
    }

    fn clear_current_file(&mut self) {
        self.last_current_file = None;
    }

    fn build_payload(&self, phase: &str, counts: &db::JobCounts) -> serde_json::Value {
        serde_json::json!({
            "jobId": self.job_id,
            "rootId": self.root_id,
            "phase": phase,
            "filesTotal": counts.files_total,
            "filesDone": counts.files_done,
            "filesAdded": counts.files_added,
            "filesUpdated": counts.files_updated,
            "filesMoved": counts.files_moved,
            "filesDeleted": counts.files_deleted,
            "errorCount": counts.error_count,
            "currentFile": self.last_current_file,
            "extractionTotal": self.extraction_total,
            "extractionDone": self.extraction_done,
        })
    }

    fn emit_now(
        &mut self,
        conn: &Connection,
        phase: &str,
        counts: &db::JobCounts,
        current_file: Option<&str>,
    ) -> Result<(), AppError> {
        if let Some(path) = current_file {
            self.last_current_file = Some(path.to_string());
        }
        db::update_job_progress(conn, self.job_id, counts, self.last_current_file.as_deref(), db::unix_now())?;
        log::debug!(
            "[progress-emit] job={} phase={} total={} done={} extTotal={} extDone={} currentFile={:?}",
            self.job_id, phase, counts.files_total, counts.files_done,
            self.extraction_total, self.extraction_done, self.last_current_file,
        );
        (self.emit)("indexing://progress", &self.build_payload(phase, counts));
        self.last_emit = Instant::now();
        Ok(())
    }

    fn tick(
        &mut self,
        conn: &Connection,
        phase: &str,
        counts: &db::JobCounts,
        current_file: Option<&str>,
    ) -> Result<(), AppError> {
        if let Some(path) = current_file {
            self.last_current_file = Some(path.to_string());
        }
        if self.last_emit.elapsed() >= PROGRESS_EMIT_INTERVAL {
            self.emit_now(conn, phase, counts, None)?;
        }
        Ok(())
    }
}

// ── public API ────────────────────────────────────────────────────────────────

pub fn run(
    app: &tauri::AppHandle,
    db_path: &Path,
    root_id: i64,
    ollama_url: &str,
    cancel: &Arc<AtomicBool>,
) -> Result<i64, AppError> {
    let conn = db::open_and_migrate(db_path)?;
    let root = db::find_root_by_id(&conn, root_id)?
        .ok_or_else(|| AppError::Indexer(format!("root {root_id} not found")))?;
    let root_path = PathBuf::from(&root.path);
    db::clear_root_index(&conn, root_id)?;
    let app = app.clone();
    run_scan(&conn, root_id, &root_path, ollama_url, cancel, &|event, payload| {
        let _ = app.emit(event, payload);
    })
}

// ── internal (testable) ───────────────────────────────────────────────────────

pub fn run_scan(
    conn: &Connection,
    root_id: i64,
    root_path: &Path,
    ollama_url: &str,
    cancel: &Arc<AtomicBool>,
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

    log::info!("[run_scan] job={job_id} root={root_id} starting");
    match run_scan_inner(conn, job_id, root_id, root_path, marker, ollama_url, cancel, emit) {
        Ok(()) => {
            log::info!("[run_scan] job={job_id} completed");
            Ok(job_id)
        }
        Err(e) => {
            let cancelled = cancel.load(Ordering::Relaxed) || is_cancelled_error(&e);
            let final_phase = if cancelled { "cancelled" } else { "interrupted" };
            log::warn!(
                "[run_scan] job={job_id} ended in error path: cancelled={cancelled} final_phase={final_phase} err={e:?}"
            );
            let now = db::unix_now();
            let _ = db::update_job_phase(conn, job_id, final_phase, now);
            if cancelled {
                emit("indexing://cancelled", &serde_json::json!({ "jobId": job_id, "rootId": root_id }));
            }
            emit("index://changed", &serde_json::Value::Null);
            Err(e)
        }
    }
}

fn is_cancelled_error(e: &AppError) -> bool {
    matches!(e, AppError::Indexer(msg) if msg == "cancelled")
}

fn run_scan_inner(
    conn: &Connection,
    job_id: i64,
    root_id: i64,
    root_path: &Path,
    marker: i64,
    ollama_url: &str,
    cancel: &Arc<AtomicBool>,
    emit: &dyn Fn(&str, &serde_json::Value),
) -> Result<(), AppError> {
    let mut counts = db::JobCounts::default();
    let mut progress = ProgressEmitter::new(job_id, root_id, emit);

    progress.emit_now(conn, "discovering", &counts, None)?;
    let candidates = run_discovery(conn, root_id, root_path, marker, &mut counts, cancel, &mut progress)?;
    progress.clear_current_file();
    progress.emit_now(conn, "discovering", &counts, None)?;

    check_cancel(cancel)?;

    db::update_job_phase(conn, job_id, "fingerprinting", db::unix_now())?;
    progress.emit_now(conn, "fingerprinting", &counts, None)?;
    run_fingerprinting(conn, root_id, marker, candidates, &mut counts, cancel, &mut progress)?;
    progress.clear_current_file();
    progress.emit_now(conn, "fingerprinting", &counts, None)?;

    check_cancel(cancel)?;

    db::update_job_phase(conn, job_id, "extracting", db::unix_now())?;
    progress.emit_now(conn, "extracting", &counts, None)?;
    run_extraction(conn, root_id, root_path, ollama_url, cancel, &mut counts, &mut progress)?;
    progress.emit_now(conn, "extracting", &counts, None)?;

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
    emit("index://changed", &serde_json::Value::Null);

    match llm::runtime::unload_model("nomic-embed-text-v2-moe", ollama_url) {
        Ok(()) => log::info!("unloaded embedding model (nomic-embed-text-v2-moe)"),
        Err(e) => log::warn!("unload embedding model failed (will remain in VRAM): {e}"),
    }

    Ok(())
}

fn check_cancel(cancel: &Arc<AtomicBool>) -> Result<(), AppError> {
    if cancel.load(Ordering::Relaxed) {
        Err(AppError::Indexer("cancelled".into()))
    } else {
        Ok(())
    }
}

fn run_discovery(
    conn: &Connection,
    root_id: i64,
    root_path: &Path,
    marker: i64,
    counts: &mut db::JobCounts,
    cancel: &Arc<AtomicBool>,
    progress: &mut ProgressEmitter<'_>,
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
                    if e.file_type().is_dir() && JUNK_DIRS.contains(&name) {
                        return false;
                    }
                }
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        if cancel.load(Ordering::Relaxed) {
            log::info!("[indexer] cancel detected in discovery loop — stopping");
            return Err(AppError::Indexer("cancelled".into()));
        }

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
                progress.tick(conn, "discovering", counts, Some(&rel_path))?;
                continue;
            }
        }
        let previously_existed = existing_record.is_some();

        progress.tick(conn, "discovering", counts, Some(&rel_path))?;

        candidates.push(FileCandidate { path, rel_path, filename, media_type, size_bytes, mtime_ns, previously_existed });
    }

    Ok(candidates)
}

fn run_fingerprinting(
    conn: &Connection,
    root_id: i64,
    marker: i64,
    candidates: Vec<FileCandidate>,
    counts: &mut db::JobCounts,
    cancel: &Arc<AtomicBool>,
    progress: &mut ProgressEmitter<'_>,
) -> Result<(), AppError> {
    for candidate in candidates {
        if cancel.load(Ordering::Relaxed) {
            log::info!("[indexer] cancel detected in fingerprinting loop — stopping");
            return Err(AppError::Indexer("cancelled".into()));
        }
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
                progress.tick(conn, "fingerprinting", counts, Some(&candidate.rel_path))?;
                continue;
            }
            db::stamp_index_marker(conn, existing_fp.id, marker, now)?;
            counts.files_done += 1;
            progress.tick(conn, "fingerprinting", counts, Some(&candidate.rel_path))?;
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
            "",
            marker,
            now,
        )?;

        if candidate.previously_existed {
            counts.files_updated += 1;
        } else {
            counts.files_added += 1;
        }
        counts.files_done += 1;
        progress.tick(conn, "fingerprinting", counts, Some(&candidate.rel_path))?;
    }
    Ok(())
}

fn run_extraction(
    conn: &Connection,
    root_id: i64,
    root_path: &Path,
    ollama_url: &str,
    cancel: &Arc<AtomicBool>,
    counts: &mut db::JobCounts,
    progress: &mut ProgressEmitter<'_>,
) -> Result<(), AppError> {
    let pending = db::find_files_needing_extraction(conn, root_id)?;
    let pending_total = pending.len() as i64;
    log::info!("[indexer] extraction phase: {} files pending", pending_total);

    progress.set_extraction_total(pending_total);
    progress.emit_now(conn, "extracting", counts, None)?;

    for (file_id, rel_path, _media_type) in pending {
        if cancel.load(Ordering::Relaxed) {
            log::info!("[indexer] cancel detected in extraction loop — stopping");
            return Err(AppError::Indexer("cancelled".into()));
        }
        log::info!("[indexer] extracting: {}", rel_path);
        progress.emit_now(conn, "extracting", counts, Some(&rel_path))?;

        let abs_path = root_path.join(&rel_path);
        let now = db::unix_now();
        match extractor::extract(&abs_path, ollama_url, cancel) {
            Ok(result) => {
                if cancel.load(Ordering::Relaxed) {
                    log::info!("[indexer] cancel detected after extraction — stopping");
                    return Err(AppError::Indexer("cancelled".into()));
                }
                if result.text.is_empty() {
                    counts.error_count += 1;
                    progress.advance_extraction();
                    progress.tick(conn, "extracting", counts, Some(&rel_path))?;
                    continue;
                }
                let chunks = chunker::chunk_text(&result.text, chunker::CHUNK_SIZE, chunker::CHUNK_OVERLAP);
                let chunk_texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
                let embeddings_result = llm::embeddings::embed_texts(&chunk_texts, ollama_url, cancel);
                if cancel.load(Ordering::Relaxed) {
                    log::info!("[indexer] cancel detected after embedding — stopping");
                    return Err(AppError::Indexer("cancelled".into()));
                }
                let embedding_blobs: Vec<Option<Vec<u8>>> = match embeddings_result {
                    Ok(vecs) => vecs.into_iter().map(|v| Some(llm::embeddings::embedding_to_bytes(&v))).collect(),
                    Err(_) => vec![None; chunks.len()],
                };
                let chunk_pairs: Vec<(usize, &str, Option<&[u8]>)> = chunks
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i, c.text.as_str(), embedding_blobs[i].as_deref()))
                    .collect();

                let confidence = if file_is_in_code_repo(&abs_path, root_path) {
                    result.confidence * REPO_CONFIDENCE_FACTOR
                } else {
                    result.confidence
                };
                db::update_file_content(conn, file_id, &result.text, confidence, &result.lang_hint, "ext-v1", now)?;
                db::replace_chunks(conn, file_id, &chunk_pairs)?;
            }
            Err(_) => {
                counts.error_count += 1;
            }
        }
        progress.advance_extraction();
        progress.tick(conn, "extracting", counts, Some(&rel_path))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "indexer_test.rs"]
mod tests;
