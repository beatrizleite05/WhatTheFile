/**
 * Policy contract spec.
 *
 * These tests define the INTENDED behaviour of the indexing policy gate that
 * will be enforced by `indexer.rs / run_discovery` once configurable policy
 * lands (DB schema + Rust implementation still pending).
 *
 * The TS `shouldIndexFile` in `policy.ts` is the reference implementation of
 * that contract. When Rust gains configurable policy, its behaviour must match
 * every case below.
 *
 * Current Rust hardcodes: allowlist via detect_media_type, hidden-file skip,
 * MAX_FILE_SIZE_BYTES = 100 MB, no glob support. Each of those is captured
 * here so the delta is visible.
 */
import { describe, expect, it } from 'vitest';
import { normalizeExt, shouldIndexFile } from '../src/core/policy';
import type { ScanPolicy } from '../src/core/types';

const MVP_EXTENSIONS = ['pdf', 'docx', 'xlsx', 'csv', 'txt', 'md', 'png', 'jpg', 'jpeg'];

const basePolicy = (): ScanPolicy => ({
  includeRoots: ['/Users/me/Documents'],
  allowedExtensions: MVP_EXTENSIONS,
  excludeGlobs: [],
  maxFileSizeBytes: 100 * 1024 * 1024,
  includeHidden: false,
});

describe('normalizeExt', () => {
  it('strips dot and lowercases', () => {
    expect(normalizeExt('/tmp/Report.PDF')).toBe('pdf');
    expect(normalizeExt('/tmp/notes.MD')).toBe('md');
  });

  it('returns empty string for files with no extension', () => {
    expect(normalizeExt('/tmp/Makefile')).toBe('');
  });
});

describe('root scope', () => {
  it('accepts a file under an included root', () => {
    expect(shouldIndexFile('/Users/me/Documents/report.pdf', 1024, basePolicy())).toBe(true);
  });

  it('rejects a file outside all included roots', () => {
    expect(shouldIndexFile('/Users/me/Downloads/report.pdf', 1024, basePolicy())).toBe(false);
  });

  it('does not match a root that is a strict prefix of another root name', () => {
    const policy = { ...basePolicy(), includeRoots: ['/Users/me/Doc'] };
    expect(shouldIndexFile('/Users/me/Documents/report.pdf', 1024, policy)).toBe(false);
  });
});

describe('hidden files', () => {
  it('rejects hidden files when includeHidden is false', () => {
    expect(shouldIndexFile('/Users/me/Documents/.hidden.txt', 100, basePolicy())).toBe(false);
  });

  it('accepts hidden files when includeHidden is true', () => {
    const policy = { ...basePolicy(), includeHidden: true };
    expect(shouldIndexFile('/Users/me/Documents/.hidden.txt', 100, policy)).toBe(true);
  });
});

describe('allowlist (extension gate)', () => {
  it('accepts all MVP v1 extensions', () => {
    for (const ext of MVP_EXTENSIONS) {
      expect(
        shouldIndexFile(`/Users/me/Documents/a.${ext}`, 1024, basePolicy()),
        `expected ${ext} to be allowed`
      ).toBe(true);
    }
  });

  it('rejects an extension not in the allowlist', () => {
    expect(shouldIndexFile('/Users/me/Documents/slides.pptx', 1024, basePolicy())).toBe(false);
    expect(shouldIndexFile('/Users/me/Documents/id_rsa.pem', 1024, basePolicy())).toBe(false);
    expect(shouldIndexFile('/Users/me/Documents/binary', 1024, basePolicy())).toBe(false);
  });
});

describe('size limit', () => {
  it('accepts a file at exactly the size limit', () => {
    const policy = { ...basePolicy(), maxFileSizeBytes: 1024 };
    expect(shouldIndexFile('/Users/me/Documents/a.txt', 1024, policy)).toBe(true);
  });

  it('rejects a file exceeding the size limit', () => {
    const policy = { ...basePolicy(), maxFileSizeBytes: 1024 };
    expect(shouldIndexFile('/Users/me/Documents/a.txt', 1025, policy)).toBe(false);
  });
});

describe('exclude globs', () => {
  it('rejects a file matching an exclude glob', () => {
    const policy = { ...basePolicy(), excludeGlobs: ['**/.git/**', '**/node_modules/**'] };
    expect(shouldIndexFile('/Users/me/Documents/.git/config', 100, policy)).toBe(false);
    expect(shouldIndexFile('/Users/me/Documents/node_modules/pkg/index.js', 100, policy)).toBe(false);
  });

  it('accepts a file that does not match any exclude glob', () => {
    const policy = { ...basePolicy(), excludeGlobs: ['**/node_modules/**'] };
    expect(shouldIndexFile('/Users/me/Documents/src/main.ts', 100, policy)).toBe(false); // not in allowlist
    expect(shouldIndexFile('/Users/me/Documents/src/readme.md', 100, policy)).toBe(true);
  });
});
