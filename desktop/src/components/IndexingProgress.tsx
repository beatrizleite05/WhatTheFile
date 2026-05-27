import { motion } from 'framer-motion';
import type { IndexingJob } from '../hooks/useIndexing';

interface IndexingProgressProps {
  activeJob: IndexingJob | null;
  rootLabel?: string;
}

const PHASE_LABELS: Record<IndexingJob['phase'], string> = {
  discovering: 'Discovering',
  fingerprinting: 'Fingerprinting',
  extracting: 'Extracting',
  completed: 'Complete',
};

export function IndexingProgress({ activeJob, rootLabel }: IndexingProgressProps) {
  if (!activeJob) return null;

  const { phase, filesDone, filesTotal, progressPercent, extractionDone, extractionTotal } = activeJob;
  const isExtracting = phase === 'extracting';
  const countDone = isExtracting && extractionTotal > 0 ? extractionDone : filesDone;
  const countTotal = isExtracting && extractionTotal > 0 ? extractionTotal : filesTotal;

  return (
    <div style={{
      padding: '6px 16px 10px',
      display: 'flex',
      flexDirection: 'column',
      gap: 4,
      borderTop: '1px solid var(--divider)',
    }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span style={{ fontSize: 'var(--font-size-xs)', color: 'var(--text-secondary)' }}>
          {PHASE_LABELS[phase]}{rootLabel ? ` — ${rootLabel}` : ''}
        </span>
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--font-size-xs)', color: 'var(--text-tertiary)' }}>
          {countDone} / {countTotal}
        </span>
      </div>
      <div
        role="progressbar"
        aria-valuenow={progressPercent}
        aria-valuemin={0}
        aria-valuemax={100}
        style={{
          height: 3,
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
    </div>
  );
}
