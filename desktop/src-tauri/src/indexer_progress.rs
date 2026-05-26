use std::time::{Duration, Instant};
use tauri::ipc::Channel;

/// Phases that map 1-to-1 with the frontend `IndexingJob.phase` field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Discovering,
    Fingerprinting,
    Extracting,
}

/// One tick payload delivered to the frontend via the IPC channel.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub job_id: i64,
    pub root_id: i64,
    /// Monotonic sequence number — frontend drops frames where seq <= last seen seq.
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
    /// Total files that need extraction (set at extraction-phase start).
    pub extraction_total: i64,
    /// Files whose extraction has finished (advances per-file).
    pub extraction_done: i64,
}

// ── trait so tests can inject a fake sender ────────────────────────────────────

pub trait ProgressSender: Send + 'static {
    fn send(&self, event: ProgressEvent);
}

/// Production sender — wraps `tauri::ipc::Channel<ProgressEvent>`.
pub struct ChannelSender(pub Channel<ProgressEvent>);

impl ProgressSender for ChannelSender {
    fn send(&self, event: ProgressEvent) {
        let _ = self.0.send(event);
    }
}

// ── ProgressReporter ──────────────────────────────────────────────────────────

const DEFAULT_THROTTLE: Duration = Duration::from_millis(150);

pub struct ProgressReporter {
    sender: Box<dyn ProgressSender>,
    job_id: i64,
    root_id: i64,
    seq: u64,
    last_sent: Instant,
    throttle: Duration,
    extraction_total: i64,
    extraction_done: i64,
}

impl ProgressReporter {
    pub fn new(sender: impl ProgressSender, job_id: i64, root_id: i64) -> Self {
        Self {
            sender: Box::new(sender),
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

    /// Set total number of files that need extraction (call at extraction-phase start).
    pub fn set_extraction_total(&mut self, total: i64) {
        self.extraction_total = total;
        self.extraction_done = 0;
    }

    /// Increment the extraction-done counter (call after each file completes).
    pub fn advance_extraction(&mut self) {
        self.extraction_done += 1;
    }

    /// Throttled emit — fires only if the throttle interval has elapsed.
    pub fn tick(
        &mut self,
        phase: Phase,
        counts: &crate::db::JobCounts,
        current_file: Option<&str>,
    ) {
        if self.last_sent.elapsed() >= self.throttle {
            self.emit(phase, counts, current_file);
        }
    }

    /// Unconditional emit — use at phase transitions and job start.
    pub fn force(
        &mut self,
        phase: Phase,
        counts: &crate::db::JobCounts,
    ) {
        self.emit(phase, counts, None);
    }

    fn emit(
        &mut self,
        phase: Phase,
        counts: &crate::db::JobCounts,
        current_file: Option<&str>,
    ) {
        self.seq += 1;
        let event = ProgressEvent {
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
        };
        self.sender.send(event);
        self.last_sent = Instant::now();
    }
}

// ── test helpers ──────────────────────────────────────────────────────────────

#[cfg(test)]
pub mod test_helpers {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Fake sender that captures every `ProgressEvent` for test assertions.
    #[derive(Clone, Default)]
    pub struct CaptureSender(pub Arc<Mutex<Vec<ProgressEvent>>>);

    impl ProgressSender for CaptureSender {
        fn send(&self, event: ProgressEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    /// Build a `ProgressReporter` with zero throttle so every `tick` fires.
    pub fn no_throttle_reporter(
        sender: CaptureSender,
        job_id: i64,
        root_id: i64,
    ) -> ProgressReporter {
        let mut r = ProgressReporter::new(sender, job_id, root_id);
        r.throttle = Duration::ZERO;
        r
    }
}
