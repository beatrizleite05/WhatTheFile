import { useState } from 'react';
import { Trash2, AlertTriangle } from 'lucide-react';
import { errorMessage } from '../utils';

interface DeleteIndexButtonProps {
  onDeleted: () => Promise<void>;
}

export function DeleteIndexButton({ onDeleted }: DeleteIndexButtonProps) {
  const [confirming, setConfirming] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleConfirm() {
    setDeleting(true);
    setError(null);
    try {
      await onDeleted();
      setConfirming(false);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setDeleting(false);
    }
  }

  if (confirming) {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        <div style={{
          display: 'flex',
          alignItems: 'center',
          gap: 8,
          padding: '10px 14px',
          background: 'var(--status-error-bg)',
          border: '1px solid rgba(239,68,68,0.25)',
          borderRadius: 'var(--radius-element)',
          color: 'var(--status-error-text)',
          fontSize: 'var(--font-size-sm)',
        }}>
          <AlertTriangle size={14} />
          This will delete all indexed data. Source files are not affected.
        </div>
        <div style={{ display: 'flex', gap: 8 }}>
          <button
            aria-label="Confirm delete index"
            disabled={deleting}
            onClick={handleConfirm}
            style={{
              padding: '6px 16px',
              background: 'rgba(239,68,68,0.8)',
              border: 'none',
              borderRadius: 'var(--radius-pill)',
              color: '#fff',
              fontFamily: 'var(--font-system)',
              fontSize: 'var(--font-size-sm)',
              cursor: deleting ? 'not-allowed' : 'pointer',
              opacity: deleting ? 0.6 : 1,
            }}
          >
            {deleting ? 'Deleting…' : 'Confirm'}
          </button>
          <button
            aria-label="Cancel"
            onClick={() => setConfirming(false)}
            style={{
              padding: '6px 16px',
              background: 'rgba(255,255,255,0.07)',
              border: '1px solid var(--surface-border)',
              borderRadius: 'var(--radius-pill)',
              color: 'var(--text-secondary)',
              fontFamily: 'var(--font-system)',
              fontSize: 'var(--font-size-sm)',
              cursor: 'pointer',
            }}
          >
            Cancel
          </button>
        </div>
        {error && (
          <span style={{ color: 'var(--status-error-text)', fontSize: 'var(--font-size-xs)' }}>
            {error}
          </span>
        )}
      </div>
    );
  }

  return (
    <button
      aria-label="Delete all indexed data"
      onClick={() => setConfirming(true)}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 6,
        padding: '7px 16px',
        background: 'rgba(239,68,68,0.12)',
        border: '1px solid rgba(239,68,68,0.25)',
        borderRadius: 'var(--radius-pill)',
        color: 'var(--status-error-text)',
        fontFamily: 'var(--font-system)',
        fontSize: 'var(--font-size-sm)',
        cursor: 'pointer',
      }}
    >
      <Trash2 size={13} />
      Delete Index
    </button>
  );
}
