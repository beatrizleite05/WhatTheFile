import { describe, expect, it } from 'vitest';
import { parseNaturalLanguageQuery } from '../src/core/queryParser';

describe('parseNaturalLanguageQuery', () => {
  it('extracts media type, root scope, and date range from structured query', () => {
    const { parsed } = parseNaturalLanguageQuery('receipts in pdf from 2024 in Dropbox');
    expect(parsed.queryText.length).toBeGreaterThan(0);
    expect(parsed.mediaTypes).toContain('pdf');
    expect(parsed.rootScope).toContain('dropbox');
    expect(parsed.dateFrom).toBe('2024-01-01');
    expect(parsed.dateTo).toBe('2024-12-31');
  });

  it('extracts confidence threshold when present', () => {
    const { parsed } = parseNaturalLanguageQuery('anime girl min confidence 0.8');
    expect(parsed.minConfidence).toBeCloseTo(0.8, 5);
  });

  it('resolves "today" to current date range', () => {
    const { parsed } = parseNaturalLanguageQuery('documents modified today');
    expect(parsed.dateFrom).toBeTruthy();
    expect(parsed.dateTo).toBe(parsed.dateFrom);
  });

  it('empty input returns safe defaults with empty queryText', () => {
    const { parsed } = parseNaturalLanguageQuery('');
    expect(parsed.mediaTypes).toEqual([]);
    expect(parsed.rootScope).toEqual([]);
    expect(parsed.dateFrom).toBe('');
    expect(parsed.queryText).toBe('');
  });

  it('extracts multiple media types from one query', () => {
    const { parsed } = parseNaturalLanguageQuery('show me pdf and docx files');
    expect(parsed.mediaTypes).toContain('pdf');
    expect(parsed.mediaTypes).toContain('docx');
  });

  it('"in" followed by a media type keyword does not create rootScope', () => {
    const { parsed } = parseNaturalLanguageQuery('search in pdf');
    expect(parsed.rootScope).toEqual([]);
    expect(parsed.mediaTypes).toContain('pdf');
  });

  it('extracts date range from standalone year without "from"', () => {
    const { parsed } = parseNaturalLanguageQuery('documents 2023');
    expect(parsed.dateFrom).toBe('2023-01-01');
    expect(parsed.dateTo).toBe('2023-12-31');
  });

  it('extracts minConfidence with decimal value', () => {
    const { parsed } = parseNaturalLanguageQuery('photos min confidence 0.75');
    expect(parsed.minConfidence).toBeCloseTo(0.75, 5);
  });

  it('falls back to full input as queryText when all tokens are consumed as structured params', () => {
    const { parsed } = parseNaturalLanguageQuery('pdf 2024');
    expect(parsed.queryText).toBe('pdf 2024');
  });

  it('preserves unrecognised words as queryText', () => {
    const { parsed } = parseNaturalLanguageQuery('birthday party photos');
    expect(parsed.queryText).toBe('birthday party photos');
    expect(parsed.mediaTypes).toEqual([]);
  });

  it('passes mode through to the returned ParsedQuery', () => {
    expect(parseNaturalLanguageQuery('pdf files', 'semantic').parsed.mode).toBe('semantic');
    expect(parseNaturalLanguageQuery('pdf files', 'keyword').parsed.mode).toBe('keyword');
    expect(parseNaturalLanguageQuery('pdf files').parsed.mode).toBe('hybrid');
  });

  it('recognises jpeg and jpg as distinct media types', () => {
    const { parsed: jpeg } = parseNaturalLanguageQuery('jpeg files');
    expect(jpeg.mediaTypes).toContain('jpeg');

    const { parsed: jpg } = parseNaturalLanguageQuery('jpg files');
    expect(jpg.mediaTypes).toContain('jpg');
  });

  describe('needsLlmFallback', () => {
    it('is false for short queries (≤5 words)', () => {
      expect(parseNaturalLanguageQuery('pdf 2024').needsLlmFallback).toBe(false);
    });

    it('is false when >5 words but ≤3 tokens unresolved', () => {
      expect(parseNaturalLanguageQuery('receipts in pdf from 2024 in Dropbox').needsLlmFallback).toBe(false);
    });

    it('is true when >5 words and >3 tokens unresolved', () => {
      expect(parseNaturalLanguageQuery('find the document about the project planning meeting notes').needsLlmFallback).toBe(true);
    });

    it('is always false in keyword mode regardless of length', () => {
      expect(parseNaturalLanguageQuery(
        'find the document about the project planning meeting notes',
        'keyword',
      ).needsLlmFallback).toBe(false);
    });

    it('is false at the exact boundary of 3 unresolved tokens (threshold is >3)', () => {
      // "from 2024" → consumed, "pdf" → consumed, leaves 3 unresolved
      expect(parseNaturalLanguageQuery('elephant giraffe banana from 2024 pdf').needsLlmFallback).toBe(false);
    });
  });
});
