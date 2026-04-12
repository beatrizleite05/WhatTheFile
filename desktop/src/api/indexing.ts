import { invoke } from '@tauri-apps/api/core';
import type { RootPayload } from './settings';

export async function addRoot(path: string): Promise<RootPayload> {
  return invoke<RootPayload>('add_root', { path });
}

export async function startIndexing(rootId: number): Promise<number> {
  return invoke<number>('start_indexing', { rootId });
}
