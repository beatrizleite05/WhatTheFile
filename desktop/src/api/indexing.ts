import { invoke } from '@tauri-apps/api/core';

export async function startIndexing(rootPath: string): Promise<void> {
  return invoke('start_indexing', { rootPath });
}

export async function addRoot(path: string): Promise<void> {
  return invoke('add_root', { path });
}
