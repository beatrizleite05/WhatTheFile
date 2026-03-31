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
});
