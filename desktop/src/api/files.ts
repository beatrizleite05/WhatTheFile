import { invoke } from '@tauri-apps/api/core';
import type { FileResult } from '../core/types';

export async function getRecentFiles(limit = 4): Promise<FileResult[]> {
  return invoke<FileResult[]>('get_recent_files', { limit });
}
