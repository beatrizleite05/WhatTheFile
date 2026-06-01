import type { ParsedQuery } from './types';
import MEDIA_TYPES from './mediaTypes.json';

export interface ParseQueryDeps {
  llmFallback: (input: string, mode: ParsedQuery['mode']) => Promise<ParsedQuery>;
}

export async function parseQuery(
  input: string,
  mode: ParsedQuery['mode'],
  deps: ParseQueryDeps,
): Promise<ParsedQuery> {
  const { parsed, useLlmFallback } = parseDeterministic(input, mode);
  if (!useLlmFallback) return parsed;

  try {
    return await deps.llmFallback(input, mode);
  } catch (err) {
    console.warn('[queryParser] LLM fallback failed; using deterministic result', err);
    return parsed;
  }
}

interface DeterministicResult {
  parsed: ParsedQuery;
  useLlmFallback: boolean;
}

function parseDeterministic(
  input: string,
  mode: ParsedQuery['mode'],
): DeterministicResult {
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
  const queryText = unresolvedTokens.join(' ') || input.trim();
  const useLlmFallback = mode !== 'keyword' && tokens.length > 5 && unresolvedTokens.length > 3;

  return {
    parsed: { queryText, mediaTypes, rootScope, dateFrom, dateTo, minConfidence, mode },
    useLlmFallback,
  };
}

