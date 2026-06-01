import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import MEDIA_TYPES from '../src/core/mediaTypes.json';

describe('mediaTypes parity', () => {
  it('TS-imported MEDIA_TYPES equals the JSON source file', () => {
    const jsonPath = join(__dirname, '../src/core/mediaTypes.json');
    const fromDisk = JSON.parse(readFileSync(jsonPath, 'utf-8'));
    expect(MEDIA_TYPES).toEqual(fromDisk);
  });

  it('all entries are lowercase non-empty strings', () => {
    for (const t of MEDIA_TYPES) {
      expect(typeof t).toBe('string');
      expect(t).toBe(t.toLowerCase());
      expect(t.length).toBeGreaterThan(0);
    }
  });
});
