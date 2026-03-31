import { describe, expect, it } from 'vitest';
import { coalesceEvents } from '../src/core/eventCoalescer';

describe('coalesceEvents', () => {
  it('keeps only latest meaningful event per file', () => {
    const result = coalesceEvents([
      { path: '/a.txt', type: 'created', ts: 1 },
      { path: '/a.txt', type: 'modified', ts: 2 },
      { path: '/b.txt', type: 'modified', ts: 3 },
      { path: '/b.txt', type: 'deleted', ts: 4 }
    ]);

    expect(result).toEqual([
      { path: '/a.txt', type: 'modified', ts: 2 },
      { path: '/b.txt', type: 'deleted', ts: 4 }
    ]);
  });

  it('normalizes create followed by delete into delete only', () => {
    const result = coalesceEvents([
      { path: '/c.txt', type: 'created', ts: 1 },
      { path: '/c.txt', type: 'deleted', ts: 2 }
    ]);
    expect(result).toEqual([{ path: '/c.txt', type: 'deleted', ts: 2 }]);
  });
});
