import { AnimatePresence, motion } from 'framer-motion';
import { ExternalLink, FileText } from 'lucide-react';
import { formatBytes, formatRelativeTime } from '../utils';
import { slideRight } from '../styles/animations';
import type { FileResult } from '../core/types';

interface PreviewPaneProps {
  result: FileResult | null;
  onOpen: (result: FileResult) => void;
}

export function PreviewPane({ result, onOpen }: PreviewPaneProps) {
  if (!result) {
    return (
      <div
        data-testid="preview-placeholder"
        style={{
          flex: 2,
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
          color: 'var(--text-tertiary)',
          fontSize: 'var(--font-size-sm)',
          gap: 8,
          borderLeft: '1px solid var(--divider)',
        }}
      >
        <FileText size={24} style={{ opacity: 0.3 }} />
        <span>Select a file to preview</span>
      </div>
    );
  }

  return (
    <AnimatePresence mode="wait">
      <motion.div
        key={result.fileId}
        variants={slideRight}
        initial="initial"
        animate="animate"
        exit="exit"
        style={{
          flex: 2,
          display: 'flex',
          flexDirection: 'column',
          gap: 16,
          padding: '20px 20px',
          borderLeft: '1px solid var(--divider)',
          overflow: 'hidden',
        }}
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
          <h2 style={{
            fontSize: 'var(--font-size-md)',
            fontWeight: 700,
            color: 'var(--text-primary)',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
            margin: 0,
          }}>
            {result.filename}
          </h2>
          <span style={{
            fontSize: 'var(--font-size-xs)',
            color: 'var(--text-tertiary)',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
            fontFamily: 'var(--font-mono)',
          }}>
            {result.path}
          </span>
        </div>

        {result.snippet && (
          <p style={{
            fontSize: 'var(--font-size-sm)',
            color: 'var(--text-secondary)',
            lineHeight: 1.6,
            margin: 0,
            flex: 1,
            overflow: 'hidden',
          }}>
            {result.snippet}
          </p>
        )}

        <div style={{
          display: 'grid',
          gridTemplateColumns: '1fr 1fr',
          gap: 8,
          fontSize: 'var(--font-size-xs)',
          color: 'var(--text-tertiary)',
          fontFamily: 'var(--font-mono)',
        }}>
          <span>{result.mediaType.toUpperCase()}</span>
          <span>{formatBytes(result.sizeBytes)}</span>
          <span>Indexed {formatRelativeTime(result.indexedAt)}</span>
          <span>{Math.round(result.confidence * 100)}% confidence</span>
        </div>

        <button
          onClick={() => onOpen(result)}
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            gap: 6,
            padding: '8px 16px',
            background: 'var(--accent)',
            border: 'none',
            borderRadius: 'var(--radius-pill)',
            color: '#fff',
            fontSize: 'var(--font-size-sm)',
            fontWeight: 600,
            fontFamily: 'var(--font-system)',
            cursor: 'pointer',
          }}
        >
          <ExternalLink size={13} />
          Open File
        </button>
      </motion.div>
    </AnimatePresence>
  );
}
