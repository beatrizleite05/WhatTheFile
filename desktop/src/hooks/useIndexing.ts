import { useState, useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';

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
}

export interface UseIndexingReturn {
  jobs: IndexingJob[];
  activeJob: IndexingJob | null;
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

export function useIndexing(): UseIndexingReturn {
  const [, forceRender] = useState(0);
  const jobsRef = useRef<Map<number, IndexingJob>>(new Map());
  const unlistenRefs = useRef<Array<() => void>>([]);

  useEffect(() => {
    let mounted = true;

    const setupListeners = async () => {
      const unlistenProgress = await listen<ProgressPayload>('indexing://progress', (event) => {
        if (!mounted) return;
        const p = event.payload;
        const existing = jobsRef.current.get(p.jobId);
        const updated: IndexingJob = {
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
          errorCount: p.errorCount,
          progressPercent: computePercent(p.filesDone, p.filesTotal),
          isComplete: false,
        };
        jobsRef.current.set(p.jobId, updated);
        forceRender((n) => n + 1);
      });

      const unlistenCompleted = await listen<CompletedPayload>('indexing://completed', (event) => {
        if (!mounted) return;
        const p = event.payload;
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

  const jobs = Array.from(jobsRef.current.values());
  const activeJob = jobs.find((j) => !j.isComplete) ?? null;

  return { jobs, activeJob };
}
