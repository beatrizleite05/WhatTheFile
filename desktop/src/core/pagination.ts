interface PaginationInput {
  limit: number;
  offset: number;
  maxLimit: number;
}

export interface PaginationResult {
  limit: number;
  offset: number;
}

export function normalizePagination({ limit, offset, maxLimit }: PaginationInput): PaginationResult {
  return {
    limit: Math.min(Math.max(limit, 1), maxLimit),
    offset: Math.max(offset, 0),
  };
}
