import { useState } from 'react';
import { motion } from 'framer-motion';
import { WifiOff, X } from 'lucide-react';
import { fadeIn } from '../styles/animations';

interface OllamaBannerProps {
  reachable: boolean;
  modelsLoaded: string[];
  loading: boolean;
}

export function OllamaBanner({ reachable, loading }: OllamaBannerProps) {
  const [dismissed, setDismissed] = useState(false);

  if (loading || reachable || dismissed) return null;

  return (
    <motion.div
      role="alert"
      variants={fadeIn}
      initial="initial"
      animate="animate"
      exit="exit"
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 8,
        padding: '8px 12px',
        background: 'var(--status-warning-bg)',
        border: '1px solid var(--status-warning-border)',
        borderRadius: 'var(--radius-element)',
        color: 'var(--status-warning-text)',
        fontSize: 'var(--font-size-sm)',
      }}
    >
      <WifiOff size={14} />
      <span style={{ flex: 1 }}>
        Ollama is unreachable — limited mode. Search requires Ollama running locally.
      </span>
      <button
        aria-label="Dismiss"
        onClick={() => setDismissed(true)}
        style={{
          background: 'none',
          border: 'none',
          cursor: 'pointer',
          color: 'inherit',
          padding: 2,
          display: 'flex',
          alignItems: 'center',
        }}
      >
        <X size={13} />
      </button>
    </motion.div>
  );
}
