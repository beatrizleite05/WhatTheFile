import { describe, expect, it } from 'vitest';
import { planIndexAction } from '../src/core/indexPlanner';

describe('planIndexAction', () => {
  it('indexes when file is new', () => {
    expect(
      planIndexAction(null, { path: '/a.txt', hash: 'h1', mtimeMs: 1, sizeBytes: 10 })
    ).toBe('index');
  });

  it('deletes when file no longer exists', () => {
    expect(
      planIndexAction({ path: '/a.txt', hash: 'h1', mtimeMs: 1, sizeBytes: 10 }, null)
    ).toBe('delete');
  });

  it('does nothing when fingerprint unchanged', () => {
    expect(
      planIndexAction(
        { path: '/a.txt', hash: 'h1', mtimeMs: 1, sizeBytes: 10 },
        { path: '/a.txt', hash: 'h1', mtimeMs: 2, sizeBytes: 10 }
      )
    ).toBe('noop');
  });

  it('reindexes when fingerprint changed', () => {
    expect(
      planIndexAction(
        { path: '/a.txt', hash: 'h1', mtimeMs: 1, sizeBytes: 10 },
        { path: '/a.txt', hash: 'h2', mtimeMs: 2, sizeBytes: 10 }
      )
    ).toBe('reindex');
  });
});
