import { ArrowLeft } from 'lucide-react';
import { ScopeEditor } from './ScopeEditor';
import { ActivityLog } from './ActivityLog';
import { DeleteIndexButton } from './DeleteIndexButton';
import { useSettings } from '../hooks/useSettings';
import { useIndexing } from '../hooks/useIndexing';

interface SettingsViewProps {
  onBack: () => void;
  onResetOnboarding: () => void;
}

export function SettingsView({ onBack, onResetOnboarding }: SettingsViewProps) {
  const indexing = useIndexing();
  const settings = useSettings(indexing.startIndexing);

  return (
    <div
      style={{
        width: '100%',
        height: '100%',
        background: 'var(--surface-bg-solid)',
        color: 'var(--text-primary)',
        fontFamily: 'var(--font-system)',
        overflowY: 'auto',
        padding: 24,
        display: 'flex',
        flexDirection: 'column',
        gap: 32,
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <button
          onClick={onBack}
          aria-label="Back"
          style={{
            background: 'none',
            border: 'none',
            cursor: 'pointer',
            color: 'var(--text-secondary)',
            display: 'flex',
            alignItems: 'center',
            padding: 4,
          }}
        >
          <ArrowLeft size={16} />
        </button>
        <h1 style={{ fontSize: 'var(--font-size-lg)', fontWeight: 700, margin: 0 }}>Settings</h1>
      </div>

      <ScopeEditor settings={settings} jobs={indexing.jobs} />

      <hr style={{ border: 'none', borderTop: '1px solid var(--divider)' }} />

      <ActivityLog jobs={indexing.jobs} />

      <hr style={{ border: 'none', borderTop: '1px solid var(--divider)' }} />

      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        <h2 style={{ fontSize: 'var(--font-size-md)', fontWeight: 700, color: 'var(--text-primary)', margin: 0 }}>
          Data
        </h2>
        <p style={{ color: 'var(--text-secondary)', fontSize: 'var(--font-size-sm)', margin: 0 }}>
          Permanently remove all indexed data. Your source files will not be affected.
        </p>
        <DeleteIndexButton onDeleted={settings.deleteIndex} />
      </div>

      <hr style={{ border: 'none', borderTop: '1px solid var(--divider)' }} />

      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        <h2 style={{ fontSize: 'var(--font-size-md)', fontWeight: 700, color: 'var(--text-primary)', margin: 0 }}>
          Setup
        </h2>
        <p style={{ color: 'var(--text-secondary)', fontSize: 'var(--font-size-sm)', margin: 0 }}>
          Re-run the onboarding flow to reconfigure your indexed folders from scratch.
        </p>
        <button
          type="button"
          onClick={onResetOnboarding}
          style={{
            alignSelf: 'flex-start',
            padding: '8px 16px',
            borderRadius: 'var(--radius-pill)',
            border: '1px solid var(--surface-border)',
            background: 'rgba(255,255,255,0.05)',
            color: 'var(--text-primary)',
            fontFamily: 'var(--font-system)',
            fontSize: 'var(--font-size-sm)',
            fontWeight: 600,
            cursor: 'pointer',
          }}
        >
          Reset setup
        </button>
      </div>
    </div>
  );
}
