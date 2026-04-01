import { describe, expect, it } from 'vitest';
import { normalizeExt, shouldIndexFile } from '../src/core/policy';
import type { ScanPolicy } from '../src/core/types';

const policy: ScanPolicy = {
  includeRoots: ['/Users/me/Documents'],
  excludeGlobs: ['**/.git/**', '**/node_modules/**', '**/.env*'],
  excludedExtensions: ['key', 'pem'],
  maxFileSizeBytes: 10 * 1024 * 1024,
  includeHidden: false
};

describe('normalizeExt', () => {
  it('normalizes extension to lowercase without dot', () => {
    expect(normalizeExt('/tmp/Report.PDF')).toBe('pdf');
    expect(normalizeExt('/tmp/notes')).toBe('');
  });
});

describe('shouldIndexFile', () => {
  it('accepts safe regular office files under size limit', () => {
    expect(shouldIndexFile('/Users/me/Documents/roadmap.docx', 4096, policy)).toBe(true);
    expect(shouldIndexFile('/Users/me/Documents/finance.xlsx', 4096, policy)).toBe(true);
  });

  it('rejects hidden files when includeHidden is false', () => {
    expect(shouldIndexFile('/Users/me/Documents/.secrets.txt', 100, policy)).toBe(false);
  });

  it('rejects excluded extension and oversize files', () => {
    expect(shouldIndexFile('/Users/me/Documents/id_rsa.pem', 100, policy)).toBe(false);
    expect(shouldIndexFile('/Users/me/Documents/huge.pdf', 50 * 1024 * 1024, policy)).toBe(false);
  });

  it('rejects paths outside included roots', () => {
    expect(shouldIndexFile('/Users/me/Downloads/a.txt', 200, policy)).toBe(false);
  });

  it('does not match a root that is a strict prefix of another root name', () => {
    const narrowPolicy: ScanPolicy = {
      ...policy,
      includeRoots: ['/Users/me/Doc'],
    };
    expect(shouldIndexFile('/Users/me/Documents/roadmap.docx', 4096, narrowPolicy)).toBe(false);
  });
});
