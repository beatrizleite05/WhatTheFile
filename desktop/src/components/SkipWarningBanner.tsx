import { AlertTriangle } from 'lucide-react';
import { openSettings } from '../api/runtime';

interface SkipWarningBannerProps {
  visible: boolean;
}

export function SkipWarningBanner({ visible }: SkipWarningBannerProps) {
  if (!visible) return null;

  function handleOpenSettings() {
    openSettings().catch((error) => {
      console.error('Failed to open settings window', error);
    });
  }

  return (
    <div
      role="status"
      data-testid="skip-warning-banner"
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
      <AlertTriangle size={14} />
      <span style={{ flex: 1 }}>
        Search quality is limited until you add at least one folder scope.
      </span>
      <button
        onClick={handleOpenSettings}
        style={{
          border: '1px solid var(--status-warning-border)',
          background: 'transparent',
          color: 'inherit',
          borderRadius: 'var(--radius-pill)',
          padding: '3px 10px',
          fontSize: 'var(--font-size-xs)',
          cursor: 'pointer',
        }}
      >
        Configure Now
      </button>
    </div>
  );
}
