import type { ParsedQuery } from './types';
import MEDIA_TYPES from './mediaTypes.json';

export interface ParseResult {
  parsed: ParsedQuery;
  // True when the deterministic pass left >3 tokens unresolved on a non-keyword
  // query longer than 5 words — signals useSearch to fire an LLM fallback parse.
  needsLlmFallback: boolean;
}

export function parseNaturalLanguageQuery(
  input: string,
  mode: ParsedQuery['mode'] = 'hybrid',
): ParseResult {
  const tokens = input.trim().split(/\s+/).filter(t => t.length > 0);
  const consumed = new Set<number>();

  const mediaTypes: string[] = [];
  const rootScope: string[] = [];
  let dateFrom = '';
  let dateTo = '';
  let minConfidence = 0;

  // Extract media types
  for (let i = 0; i < tokens.length; i++) {
    if (MEDIA_TYPES.includes(tokens[i].toLowerCase())) {
      mediaTypes.push(tokens[i].toLowerCase());
      consumed.add(i);
    }
  }

  // Extract "from YYYY"
  for (let i = 0; i < tokens.length - 1 && !dateFrom; i++) {
    if (!consumed.has(i) && tokens[i].toLowerCase() === 'from' && /^\d{4}$/.test(tokens[i + 1])) {
      dateFrom = `${tokens[i + 1]}-01-01`;
      dateTo = `${tokens[i + 1]}-12-31`;
      consumed.add(i);
      consumed.add(i + 1);
    }
  }

  // Extract standalone YYYY
  for (let i = 0; i < tokens.length && !dateFrom; i++) {
    if (!consumed.has(i) && /^\d{4}$/.test(tokens[i])) {
      dateFrom = `${tokens[i]}-01-01`;
      dateTo = `${tokens[i]}-12-31`;
      consumed.add(i);
    }
  }

  // Extract "today"
  for (let i = 0; i < tokens.length && !dateFrom; i++) {
    if (!consumed.has(i) && tokens[i].toLowerCase() === 'today') {
      const today = new Date().toISOString().split('T')[0];
      dateFrom = today;
      dateTo = today;
      consumed.add(i);
    }
  }

  // Extract "in [Scope]" — scope must not be a media type keyword
  for (let i = 0; i < tokens.length - 1; i++) {
    if (!consumed.has(i) && tokens[i].toLowerCase() === 'in' && !consumed.has(i + 1)) {
      const scope = tokens[i + 1].toLowerCase();
      if (!MEDIA_TYPES.includes(scope)) {
        rootScope.push(scope);
        consumed.add(i);
        consumed.add(i + 1);
      }
    }
  }

  // Extract "min confidence N"
  for (let i = 0; i < tokens.length - 2; i++) {
    if (
      !consumed.has(i) &&
      tokens[i].toLowerCase() === 'min' &&
      tokens[i + 1]?.toLowerCase() === 'confidence' &&
      !consumed.has(i + 1) &&
      !consumed.has(i + 2)
    ) {
      const val = parseFloat(tokens[i + 2]);
      if (!isNaN(val)) {
        minConfidence = val;
        consumed.add(i);
        consumed.add(i + 1);
        consumed.add(i + 2);
      }
    }
  }

  const unresolvedTokens = tokens.filter((_, i) => !consumed.has(i));
  // When all tokens are consumed as structured params (e.g. "pdf 2024"),
  // fall back to the full input as query text so FTS/vector still have something to search.
  const queryText = unresolvedTokens.join(' ') || input.trim();
  const wordCount = tokens.length;
  const unresolvedCount = unresolvedTokens.length;
  const needsLlmFallback = mode !== 'keyword' && wordCount > 5 && unresolvedCount > 3;

  return {
    parsed: { queryText, mediaTypes, rootScope, dateFrom, dateTo, minConfidence, mode },
    needsLlmFallback,
  };
}
