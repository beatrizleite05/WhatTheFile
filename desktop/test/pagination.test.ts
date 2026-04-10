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

  it('clamps limit=0 to 1', () => {
    const plan = normalizePagination({ limit: 0, offset: 0, maxLimit: 200 });
    expect(plan.limit).toBe(1);
  });

  it('passes through limit=maxLimit unchanged', () => {
    const plan = normalizePagination({ limit: 200, offset: 0, maxLimit: 200 });
    expect(plan.limit).toBe(200);
  });

  it('passes through limit=1 (minimum boundary)', () => {
    const plan = normalizePagination({ limit: 1, offset: 5, maxLimit: 200 });
    expect(plan.limit).toBe(1);
    expect(plan.offset).toBe(5);
  });

  it('returns correct shape with limit and offset fields', () => {
    const plan = normalizePagination({ limit: 10, offset: 20, maxLimit: 100 });
    expect(plan.limit).toBe(10);
    expect(plan.offset).toBe(20);
    expect(Object.keys(plan).sort()).toEqual(['limit', 'offset']);
  });
});
