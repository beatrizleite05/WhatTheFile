import { invoke } from '@tauri-apps/api/core';

export async function getRuntimeStatus(): Promise<unknown> {
  return invoke('get_runtime_status');
}

export async function openFile(path: string): Promise<void> {
  return invoke('open_file', { path });
}
