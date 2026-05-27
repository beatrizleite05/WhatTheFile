import { invoke } from '@tauri-apps/api/core';
import type { RootPayload } from './settings';

export interface ProgressSnapshot {
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
  isComplete: boolean;
}

export async function addRoot(path: string): Promise<RootPayload> {
  return invoke<RootPayload>('add_root', { path });
}

export async function startIndexing(rootId: number): Promise<void> {
  return invoke<void>('start_indexing', { rootId });
}

export async function getIndexingProgress(): Promise<ProgressSnapshot | null> {
  return invoke<ProgressSnapshot | null>('get_indexing_progress');
}

export async function cancelIndexing(): Promise<void> {
  return invoke<void>('cancel_indexing');
}

export interface CompletedJobRecord {
  jobId: number;
  rootId: number;
  filesTotal: number;
  filesAdded: number;
  filesUpdated: number;
  filesMoved: number;
  filesDeleted: number;
  errorCount: number;
  completedAt: number;
}

export async function getActivityLog(limit = 50): Promise<CompletedJobRecord[]> {
  return invoke<CompletedJobRecord[]>('get_activity_log', { limit });
}
