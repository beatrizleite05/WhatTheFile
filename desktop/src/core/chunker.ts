import type { Chunk } from './types';

export interface ChunkOptions {
  chunkSizeTokens: number;
  overlapTokens: number;
}

export function chunkText(text: string, options: ChunkOptions): Chunk[] {
  const { chunkSizeTokens, overlapTokens } = options;

  if (overlapTokens >= chunkSizeTokens) {
    throw new Error(
      `overlapTokens (${overlapTokens}) must be less than chunkSizeTokens (${chunkSizeTokens})`
    );
  }

  const tokens = text.split(/\s+/).filter(t => t.length > 0);
  if (tokens.length === 0) return [];

  const chunks: Chunk[] = [];
  const step = chunkSizeTokens - overlapTokens;

  for (let start = 0; start < tokens.length; start += step) {
    const end = Math.min(start + chunkSizeTokens, tokens.length);
    chunks.push({
      startToken: start,
      endToken: end,
      text: tokens.slice(start, end).join(' '),
    });
    if (end === tokens.length) break;
  }

  return chunks;
}
