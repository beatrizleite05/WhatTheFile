import { motion } from 'framer-motion';
import { LoaderCircle } from 'lucide-react';
import type { IndexingJob } from '../hooks/useIndexing';

interface IndexingHeroProps {
  activeJob: IndexingJob | null;
}

const PHASE_LABELS: Record<IndexingJob['phase'], string> = {
  discovering: 'Discovering files',
  fingerprinting: 'Fingerprinting',
  extracting: 'Extracting text',
  completed: 'Complete',
};

const PHASE_HINTS: Record<IndexingJob['phase'], string> = {
  discovering: 'Scanning your folders to find files to index.',
  fingerprinting: 'Checking which files have changed since the last index.',
  extracting: 'Reading file contents so they become searchable.',
  completed: 'Your index is up to date.',
};

export function IndexingHero({ activeJob }: IndexingHeroProps) {
  if (!activeJob) return null;

  const { phase, filesDone, filesTotal, progressPercent } = activeJob;

  return (
    <div
      data-testid="indexing-hero"
      style={{
        flex: 1,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        gap: 14,
        padding: 24,
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, color: 'var(--accent)' }}>
        <LoaderCircle size={18} style={{ animation: 'spin 1s linear infinite' }} />
        <span style={{ fontSize: 'var(--font-size-md)', fontWeight: 600, color: 'var(--text-primary)' }}>
          {PHASE_LABELS[phase]}
        </span>
      </div>
      <span style={{ color: 'var(--text-secondary)', fontSize: 'var(--font-size-sm)', textAlign: 'center', maxWidth: 360 }}>
        {PHASE_HINTS[phase]}
      </span>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6, width: 'min(360px, 80%)' }}>
        <div
          role="progressbar"
          aria-valuenow={progressPercent}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-label="Indexing progress"
          style={{
            height: 6,
            background: 'rgba(255,255,255,0.08)',
            borderRadius: 999,
            overflow: 'hidden',
          }}
        >
          <motion.div
            layout
            style={{
              height: '100%',
              background: 'var(--accent)',
              borderRadius: 999,
              width: `${progressPercent}%`,
            }}
            transition={{ type: 'spring', stiffness: 200, damping: 30 }}
          />
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', fontFamily: 'var(--font-mono)', fontSize: 'var(--font-size-xs)', color: 'var(--text-tertiary)' }}>
          <span>{filesDone} / {filesTotal} files</span>
          <span>{progressPercent}%</span>
        </div>
      </div>
      <span style={{ color: 'var(--text-tertiary)', fontSize: 'var(--font-size-xs)', textAlign: 'center' }}>
        You can start searching at any time — results will appear as files are indexed.
      </span>
    </div>
  );
}
