import { describe, expect, it } from 'vitest';
import { chunkText } from '../src/core/chunker';

describe('chunkText', () => {
  it('creates overlapping chunks using token boundaries', () => {
    const text = Array.from({ length: 20 }, (_, i) => `t${i + 1}`).join(' ');
    const chunks = chunkText(text, { chunkSizeTokens: 8, overlapTokens: 2 });

    expect(chunks.length).toBe(3);
    expect(chunks[0].startToken).toBe(0);
    expect(chunks[0].endToken).toBe(8);
    expect(chunks[1].startToken).toBe(6);
    expect(chunks[1].endToken).toBe(14);
    expect(chunks[2].startToken).toBe(12);
    expect(chunks[2].endToken).toBe(20);
  });

  it('returns empty array for empty input', () => {
    expect(chunkText('', { chunkSizeTokens: 100, overlapTokens: 20 })).toEqual([]);
  });

  it('throws when overlap is greater than or equal chunk size', () => {
    expect(() => chunkText('a b c', { chunkSizeTokens: 5, overlapTokens: 5 })).toThrow();
  });
});
