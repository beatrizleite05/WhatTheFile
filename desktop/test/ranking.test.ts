import { describe, expect, it } from 'vitest';
import { rankFiles } from '../src/core/ranking';
import type { FileResult } from '../src/core/types';

function makeResult(overrides: Partial<FileResult> & { path: string; score: number }): FileResult {
  return {
    fileId: 1,
    rootId: 1,
    filename: overrides.path.split('/').at(-1) ?? '',
    mediaType: 'pdf',
    sizeBytes: 1024,
    indexedAt: 1000,
    confidence: 1.0,
    snippet: '',
    ...overrides
  };
}

describe('rankFiles', () => {
  it('returns top-K from pre-ranked results', () => {
    const results = [
      makeResult({ path: '/a.pdf', score: 0.9 }),
      makeResult({ path: '/b.docx', score: 0.7 }),
      makeResult({ path: '/c.txt', score: 0.5 })
    ];
    const ranked = rankFiles(results, 2);
    expect(ranked).toHaveLength(2);
    expect(ranked[0].path).toBe('/a.pdf');
    expect(ranked[1].path).toBe('/b.docx');
  });

  it('tiebreaks deterministically by path', () => {
    const results = [
      makeResult({ path: '/z.pdf', score: 0.8 }),
      makeResult({ path: '/a.pdf', score: 0.8 })
    ];
    const ranked = rankFiles(results, 10);
    expect(ranked[0].path).toBe('/a.pdf');
    expect(ranked[1].path).toBe('/z.pdf');
  });

  it('lists duplicate files as separate entries', () => {
    // Same content at two paths — both must appear per spec (no deduplication).
    const results = [
      makeResult({ fileId: 1, path: '/docs/invoice.pdf', score: 0.9 }),
      makeResult({ fileId: 2, path: '/backup/invoice.pdf', score: 0.9 })
    ];
    expect(rankFiles(results, 10)).toHaveLength(2);
  });

  it('does not mutate the input array', () => {
    const results = [
      makeResult({ path: '/b.pdf', score: 0.5 }),
      makeResult({ path: '/a.pdf', score: 0.9 })
    ];
    const originalFirst = results[0].path;
    rankFiles(results, 10);
    expect(results[0].path).toBe(originalFirst);
  });

  it('returns empty list for empty input', () => {
    expect(rankFiles([], 10)).toEqual([]);
  });

  it('returns empty list when topK is zero or negative', () => {
    const results = [makeResult({ path: '/a.pdf', score: 0.9 })];
    expect(rankFiles(results, 0)).toEqual([]);
    expect(rankFiles(results, -1)).toEqual([]);
  });
});
