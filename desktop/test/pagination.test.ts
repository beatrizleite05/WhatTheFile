import { describe, expect, it } from 'vitest';
import { normalizePagination } from '../src/core/pagination';

describe('normalizePagination', () => {
  it('caps large limits to keep query UX responsive', () => {
    const plan = normalizePagination({ limit: 5000, offset: 0, maxLimit: 200 });
    expect(plan.limit).toBe(200);
  });

  it('normalizes invalid offsets/limits to safe values', () => {
    const plan = normalizePagination({ limit: -5, offset: -10, maxLimit: 200 });
    expect(plan.limit).toBeGreaterThan(0);
    expect(plan.offset).toBe(0);
  });
});
