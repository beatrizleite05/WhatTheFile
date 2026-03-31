import { describe, expect, it } from 'vitest';
import { mrr, ndcgAtK, recallAtK } from '../src/core/metrics';

describe('recallAtK', () => {
  it('returns 1.0 when all relevant results appear in top-K', () => {
    expect(recallAtK(['a', 'b'], ['a', 'b', 'c'], 3)).toBeCloseTo(1.0);
  });

  it('returns partial recall when only some relevant results are in top-K', () => {
    expect(recallAtK(['a', 'b'], ['a', 'c', 'd', 'b'], 2)).toBeCloseTo(0.5);
  });

  it('returns 0 when no relevant results are in top-K', () => {
    expect(recallAtK(['a', 'b'], ['c', 'd'], 2)).toBe(0);
  });

  it('returns 0 when relevant list is empty', () => {
    expect(recallAtK([], ['a', 'b'], 2)).toBe(0);
  });

  it('returns 0 when k is 0', () => {
    expect(recallAtK(['a'], ['a', 'b'], 0)).toBe(0);
  });
});

describe('mrr', () => {
  it('returns 1.0 when first result is relevant', () => {
    expect(mrr(['a'], ['a', 'b', 'c'])).toBeCloseTo(1.0);
  });

  it('returns 0.5 when second result is relevant', () => {
    expect(mrr(['b'], ['a', 'b', 'c'])).toBeCloseTo(0.5);
  });

  it('returns 0 when no relevant result is found', () => {
    expect(mrr(['z'], ['a', 'b', 'c'])).toBe(0);
  });

  it('returns 0 when relevant list is empty', () => {
    expect(mrr([], ['a', 'b'])).toBe(0);
  });
});

describe('ndcgAtK', () => {
  it('returns 1.0 for a perfect ranking', () => {
    const graded = { a: 3, b: 2, c: 1 };
    expect(ndcgAtK(graded, ['a', 'b', 'c'], 3)).toBeCloseTo(1.0);
  });

  it('returns less than 1.0 for an imperfect ranking', () => {
    const graded = { a: 3, b: 2, c: 1 };
    expect(ndcgAtK(graded, ['c', 'b', 'a'], 3)).toBeLessThan(1.0);
  });

  it('returns 0 when graded relevance has no positive gains', () => {
    expect(ndcgAtK({ a: 0, b: 0 }, ['a', 'b'], 2)).toBe(0);
  });

  it('returns 0 when k is 0', () => {
    expect(ndcgAtK({ a: 1 }, ['a'], 0)).toBe(0);
  });

  it('ignores results beyond k', () => {
    const graded = { a: 3, b: 2 };
    const atOne = ndcgAtK(graded, ['a', 'b'], 1);
    const atTwo = ndcgAtK(graded, ['a', 'b'], 2);
    expect(atOne).toBeLessThanOrEqual(atTwo);
  });
});
