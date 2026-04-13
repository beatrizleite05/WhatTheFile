import { invoke } from '@tauri-apps/api/core';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

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

export async function openSettings(): Promise<void> {
  const win = await WebviewWindow.getByLabel('settings');
  if (win) {
    await win.show();
    await win.setFocus();
  }
}
