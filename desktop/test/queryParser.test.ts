import { describe, expect, it, vi } from 'vitest';
import { parseQuery } from '../src/core/queryParser';
import type { ParsedQuery } from '../src/core/types';

function noopLlm() {
  return vi.fn<(input: string, mode: ParsedQuery['mode']) => Promise<ParsedQuery>>(async () => {
    throw new Error('LLM should not have been called');
  });
}

describe('parseQuery — deterministic extraction', () => {
  it('extracts media type, root scope, and date range from structured query', async () => {
    const parsed = await parseQuery('receipts in pdf from 2024 in Dropbox', 'hybrid', { llmFallback: noopLlm() });
    expect(parsed.queryText.length).toBeGreaterThan(0);
    expect(parsed.mediaTypes).toContain('pdf');
    expect(parsed.rootScope).toContain('dropbox');
    expect(parsed.dateFrom).toBe('2024-01-01');
    expect(parsed.dateTo).toBe('2024-12-31');
  });

  it('extracts confidence threshold when present', async () => {
    const parsed = await parseQuery('anime girl min confidence 0.8', 'hybrid', { llmFallback: noopLlm() });
    expect(parsed.minConfidence).toBeCloseTo(0.8, 5);
  });

  it('resolves "today" to current date range', async () => {
    const parsed = await parseQuery('documents modified today', 'hybrid', { llmFallback: noopLlm() });
    expect(parsed.dateFrom).toBeTruthy();
    expect(parsed.dateTo).toBe(parsed.dateFrom);
  });

  it('empty input returns safe defaults with empty queryText', async () => {
    const parsed = await parseQuery('', 'hybrid', { llmFallback: noopLlm() });
    expect(parsed.mediaTypes).toEqual([]);
    expect(parsed.rootScope).toEqual([]);
    expect(parsed.dateFrom).toBe('');
    expect(parsed.queryText).toBe('');
  });

  it('extracts multiple media types from one query', async () => {
    const parsed = await parseQuery('show me pdf and docx files', 'hybrid', { llmFallback: noopLlm() });
    expect(parsed.mediaTypes).toContain('pdf');
    expect(parsed.mediaTypes).toContain('docx');
  });

  it('"in" followed by a media type keyword does not create rootScope', async () => {
    const parsed = await parseQuery('search in pdf', 'hybrid', { llmFallback: noopLlm() });
    expect(parsed.rootScope).toEqual([]);
    expect(parsed.mediaTypes).toContain('pdf');
  });

  it('extracts date range from standalone year without "from"', async () => {
    const parsed = await parseQuery('documents 2023', 'hybrid', { llmFallback: noopLlm() });
    expect(parsed.dateFrom).toBe('2023-01-01');
    expect(parsed.dateTo).toBe('2023-12-31');
  });

  it('extracts minConfidence with decimal value', async () => {
    const parsed = await parseQuery('photos min confidence 0.75', 'hybrid', { llmFallback: noopLlm() });
    expect(parsed.minConfidence).toBeCloseTo(0.75, 5);
  });

  it('falls back to full input as queryText when all tokens are consumed as structured params', async () => {
    const parsed = await parseQuery('pdf 2024', 'hybrid', { llmFallback: noopLlm() });
    expect(parsed.queryText).toBe('pdf 2024');
  });

  it('preserves unrecognised words as queryText', async () => {
    const parsed = await parseQuery('birthday party photos', 'hybrid', { llmFallback: noopLlm() });
    expect(parsed.queryText).toBe('birthday party photos');
    expect(parsed.mediaTypes).toEqual([]);
  });

  it('passes mode through to the returned ParsedQuery', async () => {
    const semantic = await parseQuery('pdf files', 'semantic', { llmFallback: noopLlm() });
    expect(semantic.mode).toBe('semantic');
    const keyword = await parseQuery('pdf files', 'keyword', { llmFallback: noopLlm() });
    expect(keyword.mode).toBe('keyword');
    const hybrid = await parseQuery('pdf files', 'hybrid', { llmFallback: noopLlm() });
    expect(hybrid.mode).toBe('hybrid');
  });

  it('recognises jpeg and jpg as distinct media types', async () => {
    const jpeg = await parseQuery('jpeg files', 'hybrid', { llmFallback: noopLlm() });
    expect(jpeg.mediaTypes).toContain('jpeg');
    const jpg = await parseQuery('jpg files', 'hybrid', { llmFallback: noopLlm() });
    expect(jpg.mediaTypes).toContain('jpg');
  });
});

describe('parseQuery — LLM-fallback gate', () => {
  const stubParsed: ParsedQuery = {
    queryText: 'llm-refined',
    mediaTypes: [],
    rootScope: [],
    dateFrom: '',
    dateTo: '',
    minConfidence: 0,
    mode: 'hybrid',
  };

  it('does not call llmFallback for short queries (≤5 words)', async () => {
    const llmFallback = vi.fn(async () => stubParsed);
    await parseQuery('pdf 2024', 'hybrid', { llmFallback });
    expect(llmFallback).not.toHaveBeenCalled();
  });

  it('does not call llmFallback when >5 words but ≤3 tokens unresolved', async () => {
    const llmFallback = vi.fn(async () => stubParsed);
    await parseQuery('receipts in pdf from 2024 in Dropbox', 'hybrid', { llmFallback });
    expect(llmFallback).not.toHaveBeenCalled();
  });

  it('calls llmFallback when >5 words and >3 tokens unresolved', async () => {
    const llmFallback = vi.fn(async () => stubParsed);
    await parseQuery('find the document about the project planning meeting notes', 'hybrid', { llmFallback });
    expect(llmFallback).toHaveBeenCalledOnce();
  });

  it('never calls llmFallback in keyword mode regardless of length', async () => {
    const llmFallback = vi.fn(async () => stubParsed);
    await parseQuery('find the document about the project planning meeting notes', 'keyword', { llmFallback });
    expect(llmFallback).not.toHaveBeenCalled();
  });

  it('does not call llmFallback at the exact boundary of 3 unresolved tokens (threshold is >3)', async () => {
    const llmFallback = vi.fn(async () => stubParsed);
    await parseQuery('elephant giraffe banana from 2024 pdf', 'hybrid', { llmFallback });
    expect(llmFallback).not.toHaveBeenCalled();
  });
});

describe('parseQuery — orchestration', () => {
  const refined: ParsedQuery = {
    queryText: 'meeting notes',
    mediaTypes: ['pdf'],
    rootScope: ['work'],
    dateFrom: '2024-01-01',
    dateTo: '2024-12-31',
    minConfidence: 0,
    mode: 'hybrid',
  };

  it('returns the LLM-refined ParsedQuery when fallback succeeds', async () => {
    const llmFallback = vi.fn(async () => refined);
    const parsed = await parseQuery('find the document about the project planning meeting notes', 'hybrid', { llmFallback });
    expect(parsed).toEqual(refined);
  });

  it('returns the deterministic ParsedQuery and warns when llmFallback throws', async () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const llmFallback = vi.fn(async () => { throw new Error('ollama down'); });
    const parsed = await parseQuery('find the document about the project planning meeting notes', 'hybrid', { llmFallback });
    expect(parsed.queryText.length).toBeGreaterThan(0);
    expect(parsed.mode).toBe('hybrid');
    expect(warnSpy).toHaveBeenCalledOnce();
    warnSpy.mockRestore();
  });
});
