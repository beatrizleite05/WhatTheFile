import { parseQuery } from '../core/queryParser';
import { parseQueryLlm } from '../api/queryParser';
import type { ParsedQuery } from '../core/types';

export function parseQueryDefault(input: string, mode: ParsedQuery['mode']): Promise<ParsedQuery> {
  return parseQuery(input, mode, { llmFallback: parseQueryLlm });
}
