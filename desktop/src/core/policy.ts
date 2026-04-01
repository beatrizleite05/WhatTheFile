import { extname, basename } from 'node:path';
import type { ScanPolicy } from './types';

export function normalizeExt(filePath: string): string {
  const ext = extname(filePath);
  return ext.startsWith('.') ? ext.slice(1).toLowerCase() : ext.toLowerCase();
}

function globToRegex(glob: string): RegExp {
  const escaped = glob
    .replace(/[.+^${}()|[\]\\]/g, '\\$&')
    .replace(/\*\*/g, '\u0000GLOBSTAR\u0000')
    .replace(/\*/g, '[^/]*')
    .replace(/\u0000GLOBSTAR\u0000/g, '.*');
  return new RegExp(`^${escaped}$`);
}

export function shouldIndexFile(
  filePath: string,
  sizeBytes: number,
  policy: ScanPolicy
): boolean {
  const normalized = filePath.replace(/\\/g, '/');

  // Must be under an included root (path-prefix, not substring)
  const inRoot = policy.includeRoots.some(root => {
    const normalizedRoot = root.replace(/\\/g, '/').replace(/\/$/, '');
    return normalized.startsWith(normalizedRoot + '/');
  });
  if (!inRoot) return false;

  // Reject hidden files (filename starts with '.')
  if (!policy.includeHidden && basename(filePath).startsWith('.')) return false;

  // Reject excluded extensions
  if (policy.excludedExtensions.includes(normalizeExt(filePath))) return false;

  // Reject oversized files
  if (sizeBytes > policy.maxFileSizeBytes) return false;

  // Reject files matching any exclude glob
  if (policy.excludeGlobs.some(glob => globToRegex(glob).test(normalized))) return false;

  return true;
}
