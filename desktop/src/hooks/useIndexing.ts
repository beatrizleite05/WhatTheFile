import { useState, useEffect, useRef, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getActivityLog, startIndexing as apiStartIndexing } from '../api/indexing';

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
}

export interface UseIndexingReturn {
  jobs: IndexingJob[];
  activeJob: IndexingJob | null;
  startIndexing: (rootId: number) => Promise<void>;
}

interface ProgressPayload {
  jobId: number;
  rootId: number;
  phase: 'discovering' | 'fingerprinting' | 'extracting';
  filesTotal: number;
  filesDone: number;
  filesAdded: number;
  filesUpdated: number;
  filesMoved: number;
  filesDeleted: number;
  errorCount: number;
}

interface CompletedPayload {
  jobId: number;
  rootId: number;
  filesTotal: number;
  filesAdded: number;
  filesUpdated: number;
  filesMoved: number;
  filesDeleted: number;
  errorCount: number;
}

function computePercent(done: number, total: number): number {
  if (total === 0) return 0;
  return Math.round((done / total) * 100);
}

// Synthetic job ID for optimistic "pending" state before Rust emits the first event.
const PENDING_JOB_ID = -1;

export function useIndexing(): UseIndexingReturn {
  const [, forceRender] = useState(0);
  const jobsRef = useRef<Map<number, IndexingJob>>(new Map());
  const unlistenRefs = useRef<Array<() => void>>([]);

  useEffect(() => {
    let mounted = true;

    getActivityLog(50).then((records) => {
      if (!mounted) return;
      for (const r of records) {
        if (!jobsRef.current.has(r.jobId)) {
          jobsRef.current.set(r.jobId, {
            jobId: r.jobId,
            rootId: r.rootId,
            phase: 'completed',
            filesTotal: r.filesTotal,
            filesDone: r.filesTotal,
            filesAdded: r.filesAdded,
            filesUpdated: r.filesUpdated,
            filesMoved: r.filesMoved,
            filesDeleted: r.filesDeleted,
            errorCount: r.errorCount,
            progressPercent: 100,
            isComplete: true,
            completedAt: r.completedAt,
          });
        }
      }
      forceRender((n) => n + 1);
    }).catch(() => {});

    const setupListeners = async () => {
      const unlistenProgress = await listen<ProgressPayload>('indexing://progress', (event) => {
        if (!mounted) return;
        const p = event.payload;
        // Remove the synthetic pending job for this root now that a real job has started.
        jobsRef.current.delete(PENDING_JOB_ID);
        const existing = jobsRef.current.get(p.jobId);
        jobsRef.current.set(p.jobId, {
          ...(existing ?? {}),
          jobId: p.jobId,
          rootId: p.rootId,
          phase: p.phase,
          filesTotal: p.filesTotal,
          filesDone: p.filesDone,
          filesAdded: p.filesAdded,
          filesUpdated: p.filesUpdated,
          filesMoved: p.filesMoved,
          filesDeleted: p.filesDeleted,
          errorCount: p.errorCount ?? existing?.errorCount ?? 0,
          progressPercent: computePercent(p.filesDone, p.filesTotal),
          isComplete: false,
          completedAt: existing?.completedAt ?? null,
        });
        forceRender((n) => n + 1);
      });

      const unlistenCompleted = await listen<CompletedPayload>('indexing://completed', (event) => {
        if (!mounted) return;
        const p = event.payload;
        jobsRef.current.delete(PENDING_JOB_ID);
        const existing = jobsRef.current.get(p.jobId);
        if (existing) {
          jobsRef.current.set(p.jobId, {
            ...existing,
            phase: 'completed',
            filesTotal: p.filesTotal,
            filesAdded: p.filesAdded,
            filesUpdated: p.filesUpdated,
            filesMoved: p.filesMoved,
            filesDeleted: p.filesDeleted,
            errorCount: p.errorCount,
            progressPercent: 100,
            isComplete: true,
            completedAt: Math.floor(Date.now() / 1000),
          });
          forceRender((n) => n + 1);
        }
      });

      if (mounted) {
        unlistenRefs.current = [unlistenProgress, unlistenCompleted];
      } else {
        unlistenProgress();
        unlistenCompleted();
      }
    };

    setupListeners();

    return () => {
      mounted = false;
      unlistenRefs.current.forEach((fn) => fn());
    };
  }, []);

  const startIndexing = useCallback(async (rootId: number) => {
    // Immediately inject a synthetic pending job so the UI shows feedback
    // before the first indexing://progress event arrives from Rust.
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
    });
    forceRender((n) => n + 1);

    try {
      await apiStartIndexing(rootId);
    } finally {
      // Clean up the pending sentinel if Rust never replaced it (e.g. error).
      jobsRef.current.delete(PENDING_JOB_ID);
      forceRender((n) => n + 1);
    }
  }, []);

  const jobs = Array.from(jobsRef.current.values());
  const activeJob = jobs.find((j) => !j.isComplete) ?? null;

  return { jobs, activeJob, startIndexing };
}
