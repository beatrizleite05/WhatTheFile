import { invoke } from '@tauri-apps/api/core';
import type { FileResult } from '../core/types';

export interface SearchQuery {
  queryText: string;
  mediaTypes: string[];
  rootScope: string[];
  dateFrom?: string;
  dateTo?: string;
  minConfidence: number;
  mode: 'hybrid' | 'keyword' | 'semantic';
  limit: number;
  offset: number;
  cursor?: string;
}

export interface SearchResponse {
  results: FileResult[];
  total: number;
  limit: number;
  offset: number;
  nextCursor: string | null;
}

export async function search(query: SearchQuery): Promise<SearchResponse> {
  return invoke<SearchResponse>('search', { query });
}
