import type { FileResult } from './types';

export function rankFiles(results: FileResult[], topK: number): FileResult[] {
  if (topK <= 0) return [];
  return [...results]
    .sort((a, b) => {
      if (b.score !== a.score) return b.score - a.score;
      return a.path.localeCompare(b.path);
    })
    .slice(0, topK);
}
