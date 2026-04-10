import type { SearchRequest } from './types';

const MEDIA_TYPES = ['pdf', 'docx', 'xlsx', 'csv', 'txt', 'md', 'png', 'jpg', 'jpeg'];

export function parseNaturalLanguageQuery(
  input: string,
  mode: SearchRequest['mode'] = 'hybrid',
): SearchRequest {
  const tokens = input.trim().split(/\s+/).filter(t => t.length > 0);
  const wordCount = tokens.length;
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
  const unresolvedCount = unresolvedTokens.length;

  // LLM fallback triggers only when ALL conditions hold per spec:
  //   - mode is not "keyword" (keyword mode never uses LLM)
  //   - query is long (>5 words)
  //   - the deterministic pass left >3 tokens unresolved
  let parserConfidence: number;
  if (mode !== 'keyword' && wordCount > 5 && unresolvedCount > 3) {
    parserConfidence = 0.2; // needs LLM
  } else if (wordCount <= 5) {
    parserConfidence = 0.9; // short query — deterministic pass is sufficient
  } else {
    parserConfidence = 0.8; // long but mostly parsed, or keyword mode
  }

  return { queryText, mediaTypes, rootScope, dateFrom, dateTo, minConfidence, mode, parserConfidence };
}
