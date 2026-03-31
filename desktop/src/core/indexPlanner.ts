import type { FileState } from './types';

export type IndexAction = 'index' | 'delete' | 'noop' | 'reindex';

export function planIndexAction(
  existing: FileState | null,
  current: FileState | null
): IndexAction {
  if (existing === null) return 'index';
  if (current === null) return 'delete';
  if (existing.hash === current.hash) return 'noop';
  return 'reindex';
}
