import { useState, useEffect, useRef, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { startIndexing as apiStartIndexing, cancelIndexing as apiCancelIndexing } from '../api/indexing';

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
  cancelIndexing: () => Promise<void>;
  cancelPending: boolean;
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
  const listenersReadyRef = useRef<Promise<void> | null>(null);
  const [cancelPending, setCancelPending] = useState(false);

  useEffect(() => {
    let mounted = true;

    const setupListeners = async () => {
      const unlistenProgress = await listen<ProgressPayload>('indexing://progress', (event) => {
        if (!mounted) return;
        const p = event.payload;
        console.log('[useIndexing] indexing://progress', p);
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
        const { jobId, rootId } = event.payload;
        console.log('[useIndexing] indexing://cancelled', event.payload);
        jobsRef.current.delete(PENDING_JOB_ID);
        const existing = jobsRef.current.get(jobId);
        if (existing) {
          jobsRef.current.set(jobId, { ...existing, isComplete: true, completedAt: Math.floor(Date.now() / 1000) });
        } else {
          jobsRef.current.set(jobId, makeCompletedJob(jobId, rootId));
        }
        setCancelPending(false);
        forceRender((n) => n + 1);
      });

      const unlistenCompleted = await listen<CompletedPayload>('indexing://completed', (event) => {
        if (!mounted) return;
        const p = event.payload;
        console.log('[useIndexing] indexing://completed', p);
        jobsRef.current.delete(PENDING_JOB_ID);
        const existing = jobsRef.current.get(p.jobId);
        const base = existing ?? makeCompletedJob(p.jobId, p.rootId);
        jobsRef.current.set(p.jobId, {
          ...base,
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
      });

      if (mounted) {
        unlistenRefs.current = [unlistenProgress, unlistenCompleted, unlistenCancelled];
      } else {
        unlistenProgress();
        unlistenCompleted();
        unlistenCancelled();
      }
    };

    listenersReadyRef.current = setupListeners();

    return () => {
      mounted = false;
      unlistenRefs.current.forEach((fn) => fn());
    };
  }, []);

  const startIndexing = useCallback(async (rootId: number) => {
    if (listenersReadyRef.current) {
      await listenersReadyRef.current;
    }

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
      jobsRef.current.delete(PENDING_JOB_ID);
      for (const [id, job] of jobsRef.current) {
        if (job.rootId === rootId && !job.isComplete) {
          jobsRef.current.set(id, { ...job, isComplete: true, completedAt: Math.floor(Date.now() / 1000) });
        }
      }
      setCancelPending(false);
      forceRender((n) => n + 1);
    }
  }, []);

  const cancelIndexing = useCallback(async () => {
    console.log('[useIndexing] cancelIndexing called');
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
      console.error('[useIndexing] cancelIndexing failed', e);
    }
  }, []);

  const jobs = Array.from(jobsRef.current.values());
  const activeJob = jobs.find((j) => !j.isComplete) ?? null;

  return { jobs, activeJob, startIndexing, cancelIndexing, cancelPending };
}

function makeCompletedJob(jobId: number, rootId: number): IndexingJob {
  return {
    jobId,
    rootId,
    phase: 'completed',
    filesTotal: 0,
    filesDone: 0,
    filesAdded: 0,
    filesUpdated: 0,
    filesMoved: 0,
    filesDeleted: 0,
    errorCount: 0,
    progressPercent: 0,
    isComplete: true,
    completedAt: Math.floor(Date.now() / 1000),
  };
}
