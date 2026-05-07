import { motion } from 'framer-motion';
import { FileText, FileSpreadsheet, Image, File } from 'lucide-react';
import { formatBytes, formatRelativeTime, dirname } from '../utils';
import type { FileResult } from '../core/types';

function parseHighlighted(snippet: string): React.ReactNode[] {
  const parts = snippet.split(/<mark>|<\/mark>/);
  return parts.map((part, i) =>
    i % 2 === 1 ? <mark key={i}>{part}</mark> : part
  );
}

interface ResultTileProps {
  result: FileResult;
  isSelected: boolean;
  onSelect: (result: FileResult) => void;
  onOpen: (result: FileResult) => void;
  style?: React.CSSProperties;
}

function MediaIcon({ mediaType }: { mediaType: string }) {
  const t = mediaType.toLowerCase();
  const size = 15;
  const style = { flexShrink: 0 };
  if (t === 'pdf') return <FileText size={size} style={style} />;
  if (t === 'xlsx' || t === 'csv') return <FileSpreadsheet size={size} style={style} />;
  if (t === 'png' || t === 'jpg' || t === 'jpeg' || t === 'webp') return <Image size={size} style={style} />;
  return <File size={size} style={style} />;
}

export function ResultTile({ result, isSelected, onSelect, onOpen, style }: ResultTileProps) {
  const dir = dirname(result.path);

  return (
    <motion.div
      data-testid="result-tile"
      data-selected={isSelected ? 'true' : 'false'}
      tabIndex={0}
      whileHover={{ backgroundColor: 'rgba(255,255,255,0.04)' }}
      onClick={() => {
        onSelect(result);
        onOpen(result);
      }}
      onKeyDown={(e) => { if (e.key === 'Enter') onOpen(result); }}
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 3,
        padding: '10px 16px',
        cursor: 'default',
        outline: 'none',
        backgroundColor: isSelected ? 'var(--selection-bg)' : 'transparent',
        borderLeft: isSelected ? '2px solid var(--accent)' : '2px solid transparent',
        ...style,
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <span style={{ color: isSelected ? 'var(--accent)' : 'var(--text-secondary)' }}>
          <MediaIcon mediaType={result.mediaType} />
        </span>
        <span style={{ fontWeight: 600, fontSize: 'var(--font-size-md)', color: 'var(--text-primary)', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {result.filename}
        </span>
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--font-size-xs)', color: 'var(--text-tertiary)', flexShrink: 0 }}>
          {formatBytes(result.sizeBytes)}
        </span>
      </div>

      {result.snippet && (
        <p style={{
          fontSize: 'var(--font-size-sm)',
          color: 'var(--text-secondary)',
          overflow: 'hidden',
          display: '-webkit-box',
          WebkitLineClamp: 2,
          WebkitBoxOrient: 'vertical',
          lineHeight: 1.4,
          margin: 0,
        }}>
          {parseHighlighted(result.snippet)}
        </p>
      )}

      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <span style={{ fontSize: 'var(--font-size-xs)', color: 'var(--text-tertiary)', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {dir}
        </span>
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--font-size-xs)', color: 'var(--text-tertiary)', flexShrink: 0 }}>
          {formatRelativeTime(result.indexedAt)}
        </span>
      </div>
    </motion.div>
  );
}
