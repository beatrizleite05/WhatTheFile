import { describe, expect, it } from 'vitest';
import { parseNaturalLanguageQuery } from '../src/core/queryParser';

describe('parseNaturalLanguageQuery', () => {
  it('extracts media type, root scope, and date range from structured query', () => {
    const parsed = parseNaturalLanguageQuery('receipts in pdf from 2024 in Dropbox');
    expect(parsed.queryText.length).toBeGreaterThan(0);
    expect(parsed.mediaTypes).toContain('pdf');
    expect(parsed.rootScope).toContain('dropbox');
    expect(parsed.dateFrom).toBe('2024-01-01');
    expect(parsed.dateTo).toBe('2024-12-31');
  });

  it('extracts confidence threshold when present', () => {
    const parsed = parseNaturalLanguageQuery('anime girl min confidence 0.8');
    expect(parsed.minConfidence).toBeCloseTo(0.8, 5);
  });

  it('resolves "today" to current date range', () => {
    const parsed = parseNaturalLanguageQuery('documents modified today');
    expect(parsed.dateFrom).toBeTruthy();
    expect(parsed.dateTo).toBe(parsed.dateFrom);
  });

  it('does not flag short queries for LLM fallback', () => {
    // ≤5 words — deterministic pass is sufficient regardless of unresolved tokens
    const parsed = parseNaturalLanguageQuery('pdf 2024');
    expect(parsed.parserConfidence).toBeGreaterThanOrEqual(0.5);
  });

  it('does not flag well-parsed longer queries for LLM fallback', () => {
    // >5 words but ≤3 unresolved tokens after parsing
    const parsed = parseNaturalLanguageQuery('receipts in pdf from 2024 in Dropbox');
    expect(parsed.parserConfidence).toBeGreaterThanOrEqual(0.5);
  });

  it('flags long ambiguous queries for LLM fallback', () => {
    // >5 words AND >3 tokens unrecognized — deterministic pass cannot handle it alone
    const parsed = parseNaturalLanguageQuery('find the document about the project planning meeting notes');
    expect(parsed.parserConfidence).toBeLessThan(0.5);
  });

  it('never returns parserConfidence outside 0–1', () => {
    const inputs = ['', 'a', 'pdf', 'receipts pdf 2024 documents dropbox min confidence 0.9'];
    for (const input of inputs) {
      const { parserConfidence } = parseNaturalLanguageQuery(input);
      expect(parserConfidence).toBeGreaterThanOrEqual(0);
      expect(parserConfidence).toBeLessThanOrEqual(1);
    }
  });

  it('empty input returns safe defaults with empty queryText', () => {
    const parsed = parseNaturalLanguageQuery('');
    expect(parsed.mediaTypes).toEqual([]);
    expect(parsed.rootScope).toEqual([]);
    expect(parsed.dateFrom).toBe('');
    expect(parsed.queryText).toBe('');
  });

  it('extracts multiple media types from one query', () => {
    const parsed = parseNaturalLanguageQuery('show me pdf and docx files');
    expect(parsed.mediaTypes).toContain('pdf');
    expect(parsed.mediaTypes).toContain('docx');
  });

  it('"in" followed by a media type keyword does not create rootScope', () => {
    const parsed = parseNaturalLanguageQuery('search in pdf');
    expect(parsed.rootScope).toEqual([]);
    expect(parsed.mediaTypes).toContain('pdf');
  });

  it('extracts date range from standalone year without "from"', () => {
    const parsed = parseNaturalLanguageQuery('documents 2023');
    expect(parsed.dateFrom).toBe('2023-01-01');
    expect(parsed.dateTo).toBe('2023-12-31');
  });

  it('extracts minConfidence with decimal value', () => {
    const parsed = parseNaturalLanguageQuery('photos min confidence 0.75');
    expect(parsed.minConfidence).toBeCloseTo(0.75, 5);
  });

  it('falls back to full input as queryText when all tokens are consumed as structured params', () => {
    const parsed = parseNaturalLanguageQuery('pdf 2024');
    expect(parsed.queryText).toBe('pdf 2024');
  });

  it('preserves unrecognised words as queryText', () => {
    const parsed = parseNaturalLanguageQuery('birthday party photos');
    expect(parsed.queryText).toBe('birthday party photos');
    expect(parsed.mediaTypes).toEqual([]);
  });

  it('flags exact boundary case (6 words, 4 unresolved) for LLM fallback', () => {
    // Exactly at the threshold: wordCount=6 (>5) and all 6 tokens unresolved (>3)
    const parsed = parseNaturalLanguageQuery('elephant giraffe banana xylophone koala mango');
    expect(parsed.parserConfidence).toBeLessThan(0.5);
  });

  it('does NOT flag a 6-word query that is mostly parsed as needing LLM', () => {
    // 6 words but only 1 unresolved token (≤3) — deterministic pass handled it
    const parsed = parseNaturalLanguageQuery('receipts in pdf from 2024 Dropbox');
    expect(parsed.parserConfidence).toBeGreaterThanOrEqual(0.5);
  });

  it('never triggers LLM fallback in keyword mode, even for long ambiguous queries', () => {
    // keyword mode must always produce high parserConfidence regardless of length/tokens
    const parsed = parseNaturalLanguageQuery(
      'find the document about the project planning meeting notes',
      'keyword',
    );
    expect(parsed.parserConfidence).toBeGreaterThanOrEqual(0.5);
    expect(parsed.mode).toBe('keyword');
  });

  it('does NOT trigger LLM fallback when exactly 3 tokens are unresolved (boundary)', () => {
    // >5 words but only 3 unresolved tokens — must NOT trigger LLM (threshold is >3)
    const parsed = parseNaturalLanguageQuery('elephant giraffe banana from 2024 pdf');
    // "from 2024" → consumed (dateFrom), "pdf" → consumed (mediaType), leaves 3 unresolved
    expect(parsed.parserConfidence).toBeGreaterThanOrEqual(0.5);
  });

  it('passes mode through to the returned SearchRequest', () => {
    expect(parseNaturalLanguageQuery('pdf files', 'semantic').mode).toBe('semantic');
    expect(parseNaturalLanguageQuery('pdf files', 'keyword').mode).toBe('keyword');
    expect(parseNaturalLanguageQuery('pdf files').mode).toBe('hybrid');
  });

  it('recognises jpeg and jpg as distinct media types', () => {
    const jpeg = parseNaturalLanguageQuery('jpeg files');
    expect(jpeg.mediaTypes).toContain('jpeg');

    const jpg = parseNaturalLanguageQuery('jpg files');
    expect(jpg.mediaTypes).toContain('jpg');
  });
});
