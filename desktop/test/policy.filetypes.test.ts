import { describe, expect, it } from 'vitest';
import { shouldIndexFile } from '../src/core/policy';
import type { ScanPolicy } from '../src/core/types';

const policy: ScanPolicy = {
  includeRoots: ['/Users/me/Documents'],
  excludeGlobs: [],
  excludedExtensions: ['pem', 'key', 'pptx'],
  maxFileSizeBytes: 20 * 1024 * 1024,
  includeHidden: false
};

describe('file type scope alignment (mvp)', () => {
  it('allows locked v1 types', () => {
    const allowed = ['pdf', 'docx', 'xlsx', 'csv', 'txt', 'md', 'png', 'jpg', 'jpeg'];
    for (const ext of allowed) {
      expect(shouldIndexFile(`/Users/me/Documents/a.${ext}`, 1024, policy)).toBe(true);
    }
  });

  it('rejects phase-2 pptx in mvp policy', () => {
    expect(shouldIndexFile('/Users/me/Documents/slides.pptx', 1024, policy)).toBe(false);
  });
});
