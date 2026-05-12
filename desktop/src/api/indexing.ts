import { invoke } from '@tauri-apps/api/core';
import type { RootPayload } from './settings';

export async function addRoot(path: string): Promise<RootPayload> {
  return invoke<RootPayload>('add_root', { path });
}

export async function startIndexing(rootId: number): Promise<number> {
  return invoke<number>('start_indexing', { rootId });
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
