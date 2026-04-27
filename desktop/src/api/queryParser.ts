import { invoke } from '@tauri-apps/api/core';
import type { ParsedQuery } from '../core/types';

interface ParseQueryPayload {
  queryText: string;
  mediaTypes: string[];
  rootScope: string[];
  dateFrom: string;
  dateTo: string;
  minConfidence: number;
}

export async function parseQueryLlm(input: string, mode: ParsedQuery['mode']): Promise<ParsedQuery> {
  const payload = await invoke<ParseQueryPayload>('parse_query', { input });
  return {
    queryText: payload.queryText,
    mediaTypes: payload.mediaTypes,
    rootScope: payload.rootScope,
    dateFrom: payload.dateFrom,
    dateTo: payload.dateTo,
    minConfidence: payload.minConfidence,
    mode,
  };
}
