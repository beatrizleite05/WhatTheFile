export const RUST_SEARCH_MAX_LIMIT = 200;

interface PaginationInput {
  limit: number;
  offset: number;
  /** Must be ≤ RUST_SEARCH_MAX_LIMIT (200). Rust will clamp to 200 regardless. */
  maxLimit: number;
}

export interface PaginationResult {
  limit: number;
  offset: number;
}

export function normalizePagination({ limit, offset, maxLimit }: PaginationInput): PaginationResult {
  const effectiveMax = Math.min(maxLimit, RUST_SEARCH_MAX_LIMIT);
  return {
    limit: Math.min(Math.max(limit, 1), effectiveMax),
    offset: Math.max(offset, 0),
  };
}
