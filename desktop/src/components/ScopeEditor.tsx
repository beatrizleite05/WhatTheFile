import { FolderOpen, Trash2, RefreshCw, Plus, LoaderCircle } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { formatRelativeTime } from '../utils';
import type { useSettings } from '../hooks/useSettings';

type SettingsHook = ReturnType<typeof useSettings>;

interface ScopeEditorProps {
  settings: SettingsHook;
}

export function ScopeEditor({ settings }: ScopeEditorProps) {
  const { roots, loading, error, addRoot, removeRoot, reindex, indexingRootIds } = settings;

  async function handleAddFolder() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === 'string' && picked.trim().length > 0) {
      await addRoot(picked);
    }
  }

  if (loading) {
    return (
      <div style={{ color: 'var(--text-tertiary)', fontSize: 'var(--font-size-sm)', padding: 16 }}>
        Loading…
      </div>
    );
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <h2 style={{ fontSize: 'var(--font-size-md)', fontWeight: 700, color: 'var(--text-primary)', margin: 0 }}>
          Indexed Folders
        </h2>
        <button
          aria-label="Add folder"
          onClick={handleAddFolder}
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: 6,
            padding: '6px 14px',
            background: 'var(--selection-bg)',
            border: '1px solid var(--selection-border)',
            borderRadius: 'var(--radius-pill)',
            color: 'var(--accent)',
            fontFamily: 'var(--font-system)',
            fontSize: 'var(--font-size-sm)',
            cursor: 'pointer',
          }}
        >
          <Plus size={12} />
          Add Folder
        </button>
      </div>

      {roots.length === 0 && (
        <p style={{ color: 'var(--text-tertiary)', fontSize: 'var(--font-size-sm)', margin: 0 }}>
          No folders indexed yet.
        </p>
      )}

      {error && (
        <p style={{ color: 'var(--status-error-text)', fontSize: 'var(--font-size-xs)', margin: 0 }}>
          {error}
        </p>
      )}

      {roots.map((root) => (
        <div
          key={root.id}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 10,
            padding: '10px 14px',
            background: 'rgba(255,255,255,0.04)',
            border: '1px solid var(--surface-border)',
            borderRadius: 'var(--radius-element)',
          }}
        >
          <FolderOpen size={14} style={{ color: 'var(--text-secondary)', flexShrink: 0 }} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ color: 'var(--text-primary)', fontSize: 'var(--font-size-sm)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              {root.label}
            </div>
            <div style={{ color: 'var(--text-tertiary)', fontSize: 'var(--font-size-xs)', fontFamily: 'var(--font-mono)' }}>
              {root.lastIndexedAt ? `Indexed ${formatRelativeTime(root.lastIndexedAt)}` : 'Not yet indexed'}
            </div>
          </div>
          <button
            aria-label={`Reindex ${root.label}`}
            onClick={() => reindex(root.id)}
            disabled={indexingRootIds.has(root.id)}
            style={{ background: 'none', border: 'none', cursor: indexingRootIds.has(root.id) ? 'default' : 'pointer', color: 'var(--text-tertiary)', padding: 4, display: 'flex', opacity: indexingRootIds.has(root.id) ? 0.4 : 1 }}
          >
            {indexingRootIds.has(root.id)
              ? <LoaderCircle size={13} style={{ animation: 'spin 1s linear infinite' }} />
              : <RefreshCw size={13} />}
          </button>
          <button
            aria-label={`Remove ${root.label}`}
            onClick={() => removeRoot(root.id)}
            style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--status-error-text)', padding: 4, display: 'flex', opacity: 0.7 }}
          >
            <Trash2 size={13} />
          </button>
        </div>
      ))}
    </div>
  );
}
