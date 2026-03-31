import { invoke } from '@tauri-apps/api/core';

export async function search(query: string): Promise<unknown> {
  return invoke('search', { query });
}
