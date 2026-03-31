import { describe, expect, it } from 'vitest';
import { buildFingerprint } from '../src/core/fingerprint';

describe('buildFingerprint', () => {
  it('is deterministic for same inputs', () => {
    const a = buildFingerprint('hello', 1000, 10);
    const b = buildFingerprint('hello', 1000, 10);
    expect(a).toBe(b);
  });

  it('changes when content or metadata changes', () => {
    const base = buildFingerprint('hello', 1000, 10);
    expect(buildFingerprint('hello2', 1000, 10)).not.toBe(base);
    expect(buildFingerprint('hello', 1001, 10)).not.toBe(base);
    expect(buildFingerprint('hello', 1000, 11)).not.toBe(base);
  });
});
