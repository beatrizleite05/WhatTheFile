import { invoke } from '@tauri-apps/api/core';

export interface RuntimeStatus {
  ollamaReachable: boolean;
  modelsLoaded: string[];
}

export async function getRuntimeStatus(): Promise<RuntimeStatus> {
  return invoke<RuntimeStatus>('get_runtime_status');
}

export async function openFile(path: string): Promise<void> {
  return invoke('open_file', { path });
}

