import { createContext, useContext, useState, useEffect, useRef, useCallback, ReactNode } from 'react';
import { Channel } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { startIndexing as apiStartIndexing, cancelIndexing as apiCancelIndexing } from '../api/indexing';

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

// ── progress event shape from Rust ────────────────────────────────────────────

interface ProgressEvent {
  jobId: number;
  rootId: number;
  seq: number;
  phase: 'discovering' | 'fingerprinting' | 'extracting';
  filesTotal: number;
  filesDone: number;
  filesAdded: number;
  filesUpdated: number;
  filesMoved: number;
  filesDeleted: number;
  errorCount: number;
  currentFile: string | null;
  extractionTotal: number;
  extractionDone: number;
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

// ── helpers ───────────────────────────────────────────────────────────────────

function computePercent(done: number, total: number): number {
  if (total === 0) return 0;
  return Math.round((done / total) * 100);
}

const PENDING_JOB_ID = -1;

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
    currentFile: null,
    extractionTotal: 0,
    extractionDone: 0,
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
  // Tracks last seen seq per jobId — events with seq <= lastSeq[jobId] are dropped.
  const lastSeqRef = useRef<Map<number, number>>(new Map());
  const unlistenRefs = useRef<Array<() => void>>([]);
  const [cancelPending, setCancelPending] = useState(false);

  const handleProgress = useCallback((p: ProgressEvent) => {
    const lastSeq = lastSeqRef.current.get(p.jobId) ?? 0;
    if (p.seq <= lastSeq) return; // drop out-of-order frame
    lastSeqRef.current.set(p.jobId, p.seq);

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
      currentFile: p.currentFile ?? null,
      extractionTotal: p.extractionTotal ?? 0,
      extractionDone: p.extractionDone ?? 0,
    });
    forceRender((n) => n + 1);
  }, []);

  useEffect(() => {
    let mounted = true;

    const setup = async () => {
      const unlistenCancelled = await listen<{ jobId: number; rootId: number }>('indexing://cancelled', (event) => {
        if (!mounted) return;
        const { jobId, rootId } = event.payload;
        jobsRef.current.delete(PENDING_JOB_ID);
        const existing = jobsRef.current.get(jobId);
        jobsRef.current.set(jobId, {
          ...(existing ?? makeCompletedJob(jobId, rootId)),
          isComplete: true,
          completedAt: Math.floor(Date.now() / 1000),
        });
        setCancelPending(false);
        forceRender((n) => n + 1);
      });

      const unlistenCompleted = await listen<CompletedPayload>('indexing://completed', (event) => {
        if (!mounted) return;
        const p = event.payload;
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
        unlistenRefs.current = [unlistenCancelled, unlistenCompleted];
      } else {
        unlistenCancelled();
        unlistenCompleted();
      }
    };

    setup();

    return () => {
      mounted = false;
      unlistenRefs.current.forEach((fn) => fn());
    };
  }, []);

  const startIndexing = useCallback(async (rootId: number) => {
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
    forceRender((n) => n + 1);

    try {
      await apiStartIndexing(rootId, handleProgress);
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
  }, [handleProgress]);

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
