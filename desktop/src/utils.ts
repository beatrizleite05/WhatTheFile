export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const value = bytes / Math.pow(1024, i);
  return `${value % 1 === 0 ? value : value.toFixed(1)} ${units[i]}`;
}

export function formatRelativeTime(unixSeconds: number): string {
  const now = Date.now() / 1000;
  const diff = now - unixSeconds;

  if (diff < 60) return 'just now';
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 86400 * 30) return `${Math.floor(diff / 86400)}d ago`;
  const d = new Date(unixSeconds * 1000);
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
}

export function mediaTypeLabel(mediaType: string): string {
  const labels: Record<string, string> = {
    pdf: 'PDF',
    docx: 'Word',
    xlsx: 'Excel',
    csv: 'CSV',
    txt: 'Text',
    md: 'Markdown',
    png: 'PNG',
    jpg: 'JPEG',
    jpeg: 'JPEG',
  };
  return labels[mediaType.toLowerCase()] ?? mediaType.toUpperCase();
}

type PillType = 'mediaType' | 'date' | 'scope' | 'confidence';

export function pillColorVar(tokenType: PillType): { bg: string; text: string } {
  const map: Record<PillType, { bg: string; text: string }> = {
    mediaType: { bg: 'var(--pill-doc-bg)', text: 'var(--pill-doc-text)' },
    date: { bg: 'var(--pill-date-bg)', text: 'var(--pill-date-text)' },
    scope: { bg: 'var(--pill-scope-bg)', text: 'var(--pill-scope-text)' },
    confidence: { bg: 'var(--pill-confidence-bg)', text: 'var(--pill-confidence-text)' },
  };
  return map[tokenType];
}

export function mediaTypePillColors(mediaType: string): { bg: string; text: string } {
  const t = mediaType.toLowerCase();
  if (t === 'pdf') return { bg: 'var(--pill-pdf-bg)', text: 'var(--pill-pdf-text)' };
  if (t === 'png' || t === 'jpg' || t === 'jpeg') return { bg: 'var(--pill-image-bg)', text: 'var(--pill-image-text)' };
  if (t === 'xlsx' || t === 'csv') return { bg: 'var(--pill-sheet-bg)', text: 'var(--pill-sheet-text)' };
  if (t === 'txt' || t === 'md') return { bg: 'var(--pill-text-bg)', text: 'var(--pill-text-text)' };
  return { bg: 'var(--pill-doc-bg)', text: 'var(--pill-doc-text)' };
}

export function errorMessage(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return 'An unexpected error occurred';
}

export function basename(path: string): string {
  return path.replace(/\\/g, '/').split('/').pop() ?? path;
}

export function dirname(path: string): string {
  const normalized = path.replace(/\\/g, '/');
  const idx = normalized.lastIndexOf('/');
  return idx >= 0 ? normalized.slice(0, idx) : '';
}
