import { invoke } from '@tauri-apps/api/core';

export interface RootPayload {
  id: number;
  path: string;
  label: string;
  active: boolean;
  createdAt: number;
  lastIndexedAt: number | null;
}

export async function listRoots(): Promise<RootPayload[]> {
  return invoke<RootPayload[]>('list_roots');
}

export async function removeRoot(id: number): Promise<void> {
  return invoke('remove_root', { id });
}

export async function deleteIndex(): Promise<void> {
  return invoke('delete_index');
}
