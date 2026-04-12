import { ScopeEditor } from './ScopeEditor';
import { ActivityLog } from './ActivityLog';
import { DeleteIndexButton } from './DeleteIndexButton';
import { useSettings } from '../hooks/useSettings';
import { useIndexing } from '../hooks/useIndexing';

export function SettingsView() {
  const settings = useSettings();
  const indexing = useIndexing();

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
      <h1 style={{ fontSize: 'var(--font-size-lg)', fontWeight: 700, margin: 0 }}>Settings</h1>

      <ScopeEditor settings={settings} />

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
        <DeleteIndexButton />
      </div>
    </div>
  );
}
