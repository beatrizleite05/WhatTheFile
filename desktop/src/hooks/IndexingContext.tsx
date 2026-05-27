import { createContext, useContext, useState, useEffect, useRef, useCallback, ReactNode } from 'react';
import {
  startIndexing as apiStartIndexing,
  cancelIndexing as apiCancelIndexing,
  getIndexingProgress as apiGetIndexingProgress,
  getActivityLog as apiGetActivityLog,
  type ProgressSnapshot,
} from '../api/indexing';

// ── types ─────────────────────────────────────────────────────────────────────

export interface IndexingJob {
  jobId: number;
  rootId: number;
  phase: 'discovering' | 'fingerprinting' | 'extracting' | 'completed';
  filesTotal: number;
  filesDone: number;
  filesAdded: number;
  filesUpdated: number;
  filesMoved: number;
  filesDeleted: number;
  errorCount: number;
  progressPercent: number;
  isComplete: boolean;
  completedAt: number | null;
  currentFile: string | null;
  extractionTotal: number;
  extractionDone: number;
}

export interface IndexingContextValue {
  jobs: IndexingJob[];
  activeJob: IndexingJob | null;
  startIndexing: (rootId: number) => Promise<void>;
  cancelIndexing: () => Promise<void>;
  cancelPending: boolean;
}

// ── helpers ───────────────────────────────────────────────────────────────────

function computePercent(done: number, total: number): number {
  if (total === 0) return 0;
  return Math.round((done / total) * 100);
}

const PENDING_JOB_ID = -1;
const POLL_INTERVAL_MS = 200;

function snapshotToJob(s: ProgressSnapshot, existing?: IndexingJob): IndexingJob {
  return {
    jobId: s.jobId,
    rootId: s.rootId,
    phase: s.isComplete ? 'completed' : s.phase,
    filesTotal: s.filesTotal,
    filesDone: s.filesDone,
    filesAdded: s.filesAdded,
    filesUpdated: s.filesUpdated,
    filesMoved: s.filesMoved,
    filesDeleted: s.filesDeleted,
    errorCount: s.errorCount,
    progressPercent: s.isComplete ? 100 : computePercent(s.filesDone, s.filesTotal),
    isComplete: s.isComplete,
    completedAt: s.isComplete ? Math.floor(Date.now() / 1000) : existing?.completedAt ?? null,
    currentFile: s.currentFile,
    extractionTotal: s.extractionTotal,
    extractionDone: s.extractionDone,
  };
}

// ── context ───────────────────────────────────────────────────────────────────

export const IndexingContext = createContext<IndexingContextValue>({
  jobs: [],
  activeJob: null,
  startIndexing: async () => {},
  cancelIndexing: async () => {},
  cancelPending: false,
});

// ── provider ──────────────────────────────────────────────────────────────────

interface IndexingProviderProps {
  children: ReactNode;
}

export function IndexingProvider({ children }: IndexingProviderProps) {
  const [, forceRender] = useState(0);
  const jobsRef = useRef<Map<number, IndexingJob>>(new Map());
  const lastSeqRef = useRef<number>(0);
  const pollHandleRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const [cancelPending, setCancelPending] = useState(false);

  const stopPolling = useCallback(() => {
    if (pollHandleRef.current !== null) {
      clearInterval(pollHandleRef.current);
      pollHandleRef.current = null;
    }
  }, []);

  // We poll while a job is active. Each tick:
  //   1. invoke get_indexing_progress
  //   2. if snapshot is null, nothing's running — stop polling
  //   3. if seq <= last seen, skip (no new data)
  //   4. upsert the job, drop PENDING placeholder
  //   5. if snapshot.isComplete, hydrate final counts via get_activity_log and stop
  const pollOnce = useCallback(async () => {
    let snap: ProgressSnapshot | null;
    try {
      snap = await apiGetIndexingProgress();
    } catch (e) {
      console.error('[IndexingProvider] get_indexing_progress failed', e);
      return;
    }
    if (!snap) {
      stopPolling();
      return;
    }
    if (snap.seq <= lastSeqRef.current) return;
    lastSeqRef.current = snap.seq;

    jobsRef.current.delete(PENDING_JOB_ID);
    const key = snap.jobId === 0 ? PENDING_JOB_ID : snap.jobId;
    const existing = jobsRef.current.get(key);
    jobsRef.current.set(key, snapshotToJob(snap, existing));
    forceRender((n) => n + 1);

    if (snap.isComplete) {
      stopPolling();
      setCancelPending(false);
      // Authoritative final counts live in the activity log row written by the indexer.
      try {
        const activity = await apiGetActivityLog(1);
        const latest = activity[0];
        if (latest && latest.jobId === snap.jobId) {
          const existing2 = jobsRef.current.get(snap.jobId);
          if (existing2) {
            jobsRef.current.set(snap.jobId, {
              ...existing2,
              filesTotal: latest.filesTotal,
              filesAdded: latest.filesAdded,
              filesUpdated: latest.filesUpdated,
              filesMoved: latest.filesMoved,
              filesDeleted: latest.filesDeleted,
              errorCount: latest.errorCount,
              completedAt: latest.completedAt,
            });
            forceRender((n) => n + 1);
          }
        }
      } catch (e) {
        console.error('[IndexingProvider] hydrate final counts failed', e);
      }
    }
  }, [stopPolling]);

  const startPolling = useCallback(() => {
    if (pollHandleRef.current !== null) return;
    pollHandleRef.current = setInterval(() => { void pollOnce(); }, POLL_INTERVAL_MS);
    void pollOnce();
  }, [pollOnce]);

  useEffect(() => stopPolling, [stopPolling]);

  const startIndexing = useCallback(async (rootId: number) => {
    // Insert a PENDING placeholder so the hero appears instantly.
    jobsRef.current.set(PENDING_JOB_ID, {
      jobId: PENDING_JOB_ID,
      rootId,
      phase: 'discovering',
      filesTotal: 0,
      filesDone: 0,
      filesAdded: 0,
      filesUpdated: 0,
      filesMoved: 0,
      filesDeleted: 0,
      errorCount: 0,
      progressPercent: 0,
      isComplete: false,
      completedAt: null,
      currentFile: null,
      extractionTotal: 0,
      extractionDone: 0,
    });
    lastSeqRef.current = 0;
    forceRender((n) => n + 1);

    try {
      await apiStartIndexing(rootId);
      startPolling();
    } catch (e) {
      jobsRef.current.delete(PENDING_JOB_ID);
      forceRender((n) => n + 1);
      throw e;
    }
  }, [startPolling]);

  const cancelIndexing = useCallback(async () => {
    setCancelPending(true);
    const now = Math.floor(Date.now() / 1000);
    for (const [id, job] of jobsRef.current) {
      if (!job.isComplete) {
        jobsRef.current.set(id, { ...job, isComplete: true, completedAt: now });
      }
    }
    forceRender((n) => n + 1);
    try {
      await apiCancelIndexing();
    } catch (e) {
      console.error('[IndexingProvider] cancelIndexing failed', e);
    }
  }, []);

  const jobs = Array.from(jobsRef.current.values());
  const activeJob = jobs.find((j) => !j.isComplete) ?? null;

  return (
    <IndexingContext.Provider value={{ jobs, activeJob, startIndexing, cancelIndexing, cancelPending }}>
      {children}
    </IndexingContext.Provider>
  );
}

export function useIndexing(): IndexingContextValue {
  return useContext(IndexingContext);
}
