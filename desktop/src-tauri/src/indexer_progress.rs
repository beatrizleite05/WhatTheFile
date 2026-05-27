use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Phases that map 1-to-1 with the frontend `IndexingJob.phase` field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Discovering,
    Fingerprinting,
    Extracting,
}

/// Snapshot of the latest progress tick. Read by the frontend via `get_indexing_progress`.
///
/// Why a snapshot+polling transport instead of `tauri::ipc::Channel`:
/// the patched tao/wry crates needed to boot on macOS 26 (see `patches/`) leave the
/// tao event loop unable to dispatch `WebviewMessage::EvaluateScript` user events.
/// That breaks every Rust→JS push path (Channel, app.emit, webview.eval) silently.
/// JS→Rust invokes still work, so we expose a poll command instead.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressSnapshot {
    pub job_id: i64,
    pub root_id: i64,
    pub seq: u64,
    pub phase: Phase,
    pub files_total: i64,
    pub files_done: i64,
    pub files_added: i64,
    pub files_updated: i64,
    pub files_moved: i64,
    pub files_deleted: i64,
    pub error_count: i64,
    /// Relative path of the file currently being extracted (None outside extraction phase).
    pub current_file: Option<String>,
    pub extraction_total: i64,
    pub extraction_done: i64,
    /// True once the job has finished (success or cancel) so the frontend can stop polling.
    pub is_complete: bool,
}

/// Shared handle. The Tauri command writes here; the polling command reads from here.
pub type ProgressStore = Arc<Mutex<Option<ProgressSnapshot>>>;

pub fn new_store() -> ProgressStore {
    Arc::new(Mutex::new(None))
}

// ── trait so tests can inject a fake sink ─────────────────────────────────────

pub trait ProgressSink: Send + 'static {
    fn write(&self, snapshot: ProgressSnapshot);
    /// Mark the last snapshot complete in-place (preserves all counts/file).
    /// Used at job end so the frontend stops polling without losing final counts.
    fn mark_complete(&self);
}

/// Production sink — writes into the shared `ProgressStore`.
pub struct StoreSink(pub ProgressStore);

impl ProgressSink for StoreSink {
    fn write(&self, snapshot: ProgressSnapshot) {
        *self.0.lock().unwrap() = Some(snapshot);
    }
    fn mark_complete(&self) {
        if let Some(s) = self.0.lock().unwrap().as_mut() {
            s.is_complete = true;
        }
    }
}

// ── ProgressReporter ──────────────────────────────────────────────────────────

const DEFAULT_THROTTLE: Duration = Duration::from_millis(150);

pub struct ProgressReporter {
    sink: Box<dyn ProgressSink>,
    job_id: i64,
    root_id: i64,
    seq: u64,
    last_sent: Instant,
    throttle: Duration,
    extraction_total: i64,
    extraction_done: i64,
}

impl ProgressReporter {
    pub fn new(sink: impl ProgressSink, job_id: i64, root_id: i64) -> Self {
        Self {
            sink: Box::new(sink),
            job_id,
            root_id,
            seq: 0,
            last_sent: Instant::now() - DEFAULT_THROTTLE * 2,
            throttle: DEFAULT_THROTTLE,
            extraction_total: 0,
            extraction_done: 0,
        }
    }

    pub fn job_id(&self) -> i64 {
        self.job_id
    }

    pub fn set_job_id(&mut self, job_id: i64) {
        self.job_id = job_id;
    }

    pub fn set_extraction_total(&mut self, total: i64) {
        self.extraction_total = total;
        self.extraction_done = 0;
    }

    pub fn advance_extraction(&mut self) {
        self.extraction_done += 1;
    }

    /// Throttled write — fires only if the throttle interval has elapsed.
    pub fn tick(
        &mut self,
        phase: Phase,
        counts: &crate::db::JobCounts,
        current_file: Option<&str>,
    ) {
        if self.last_sent.elapsed() >= self.throttle {
            self.write(phase, counts, current_file, false);
        }
    }

    /// Unconditional write — use at phase transitions and job start.
    pub fn force(&mut self, phase: Phase, counts: &crate::db::JobCounts) {
        self.write(phase, counts, None, false);
    }

    /// Mark the last snapshot complete in-place. Use at job exit so the frontend
    /// stops polling. Preserves the most recent counts/phase/file.
    pub fn finish_with_last(&mut self) {
        self.sink.mark_complete();
    }

    fn write(
        &mut self,
        phase: Phase,
        counts: &crate::db::JobCounts,
        current_file: Option<&str>,
        is_complete: bool,
    ) {
        self.seq += 1;
        let snapshot = ProgressSnapshot {
            job_id: self.job_id,
            root_id: self.root_id,
            seq: self.seq,
            phase,
            files_total: counts.files_total,
            files_done: counts.files_done,
            files_added: counts.files_added,
            files_updated: counts.files_updated,
            files_moved: counts.files_moved,
            files_deleted: counts.files_deleted,
            error_count: counts.error_count,
            current_file: current_file.map(|s| s.to_string()),
            extraction_total: self.extraction_total,
            extraction_done: self.extraction_done,
            is_complete,
        };
        self.sink.write(snapshot);
        self.last_sent = Instant::now();
    }
}

// ── test helpers ──────────────────────────────────────────────────────────────

#[cfg(test)]
pub mod test_helpers {
    use super::*;

    /// Fake sink that captures every snapshot for test assertions.
    #[derive(Clone, Default)]
    pub struct CaptureSink(pub Arc<Mutex<Vec<ProgressSnapshot>>>);

    impl ProgressSink for CaptureSink {
        fn write(&self, snapshot: ProgressSnapshot) {
            self.0.lock().unwrap().push(snapshot);
        }
        fn mark_complete(&self) {
            if let Some(last) = self.0.lock().unwrap().last_mut() {
                last.is_complete = true;
            }
        }
    }

    pub fn no_throttle_reporter(
        sink: CaptureSink,
        job_id: i64,
        root_id: i64,
    ) -> ProgressReporter {
        let mut r = ProgressReporter::new(sink, job_id, root_id);
        r.throttle = Duration::ZERO;
        r
    }
}
