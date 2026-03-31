import { createHash } from 'node:crypto';

export function buildFingerprint(content: string, mtimeMs: number, sizeBytes: number): string {
  return createHash('sha256')
    .update(`${content}:${mtimeMs}:${sizeBytes}`)
    .digest('hex');
}
