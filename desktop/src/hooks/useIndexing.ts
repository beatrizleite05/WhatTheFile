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

    // Activity log is intentionally not loaded eagerly here to avoid
    // invoking Tauri commands during many unit tests which can cause
    // mocked `invoke` call ordering to become flaky. The activity log
    // is fetched on-demand by the UI when needed.

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

      const unlistenCancelled = await listen<{ jobId: number; rootId: number }>('indexing://cancelled', (event) => {
        if (!mounted) return;
        const { jobId } = event.payload;
        jobsRef.current.delete(PENDING_JOB_ID);
        const existing = jobsRef.current.get(jobId);
        if (existing) {
          jobsRef.current.set(jobId, { ...existing, isComplete: true, completedAt: Math.floor(Date.now() / 1000) });
        }
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
        unlistenRefs.current = [unlistenProgress, unlistenCompleted, unlistenCancelled];
      } else {
        unlistenProgress();
        unlistenCompleted();
        unlistenCancelled();
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
      // Remove the sentinel and mark any still-active job for this root as
      // complete so the hero dismisses on cancellation or error.
      jobsRef.current.delete(PENDING_JOB_ID);
      for (const [id, job] of jobsRef.current) {
        if (job.rootId === rootId && !job.isComplete) {
          jobsRef.current.set(id, { ...job, isComplete: true, completedAt: Math.floor(Date.now() / 1000) });
        }
      }
      forceRender((n) => n + 1);
    }
  }, []);

  const jobs = Array.from(jobsRef.current.values());
  const activeJob = jobs.find((j) => !j.isComplete) ?? null;

  return { jobs, activeJob, startIndexing };
}
