import { useState } from 'react';
import { motion } from 'framer-motion';
import { FolderOpen, Shield, AlertTriangle, Plus, X } from 'lucide-react';
import { documentDir, desktopDir, downloadDir, pictureDir } from '@tauri-apps/api/path';
import { open } from '@tauri-apps/plugin-dialog';
import { fadeSlide } from '../styles/animations';
import type { useSettings } from '../hooks/useSettings';

type SettingsHook = ReturnType<typeof useSettings>;

interface OnboardingFlowProps {
  onComplete: (skipped: boolean) => void;
  settings: SettingsHook;
}

const PRESETS: { label: string; resolver: () => Promise<string> }[] = [
  { label: 'Documents', resolver: documentDir },
  { label: 'Desktop',   resolver: desktopDir  },
  { label: 'Downloads', resolver: downloadDir },
  { label: 'Pictures',  resolver: pictureDir  },
];

export function OnboardingFlow({ onComplete, settings }: OnboardingFlowProps) {
  const [step, setStep] = useState<'folders' | 'privacy'>('folders');
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [customFolders, setCustomFolders] = useState<string[]>([]);

  function togglePreset(label: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(label)) next.delete(label);
      else next.add(label);
      return next;
    });
  }

  async function handleAddCustomFolder() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked !== 'string' || picked.trim().length === 0) return;

    setCustomFolders((prev) => (prev.includes(picked) ? prev : [...prev, picked]));
  }

  function removeCustomFolder(path: string) {
    setCustomFolders((prev) => prev.filter((p) => p !== path));
  }

  async function handleGetStarted() {
    for (const root of settings.roots) {
      await settings.removeRoot(root.id).catch(() => {});
    }

    for (const preset of PRESETS) {
      if (!selected.has(preset.label)) continue;
      try {
        const path = await preset.resolver();
        await settings.addRoot(path);
      } catch {
        // If the OS directory doesn't exist, skip it silently.
      }
    }

    for (const path of customFolders) {
      try {
        await settings.addRoot(path);
      } catch {
        // Skip invalid or inaccessible custom paths.
      }
    }

    onComplete(false);
  }

  const btnBase: React.CSSProperties = {
    padding: '8px 20px',
    borderRadius: 'var(--radius-pill)',
    border: 'none',
    fontFamily: 'var(--font-system)',
    fontSize: 'var(--font-size-sm)',
    fontWeight: 600,
    cursor: 'pointer',
  };

  return (
    <div style={{
      width: '100%',
      height: '100%',
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      justifyContent: 'center',
      padding: 32,
      gap: 24,
    }}>
      {step === 'folders' && (
        <motion.div
          data-testid="folder-picker"
          variants={fadeSlide}
          initial="initial"
          animate="animate"
          style={{ display: 'flex', flexDirection: 'column', gap: 20, width: '100%', maxWidth: 400 }}
        >
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            <h1 style={{ fontSize: 'var(--font-size-lg)', fontWeight: 700, color: 'var(--text-primary)', margin: 0 }}>
              Choose folders to index
            </h1>
            <p style={{ color: 'var(--text-secondary)', fontSize: 'var(--font-size-sm)', margin: 0 }}>
              WhatTheFile will index these locations to enable semantic search.
            </p>
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            {PRESETS.map(({ label }) => (
              <label
                key={label}
                aria-label={label}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 10,
                  padding: '10px 14px',
                  borderRadius: 'var(--radius-element)',
                  background: selected.has(label) ? 'var(--selection-bg)' : 'rgba(255,255,255,0.04)',
                  border: `1px solid ${selected.has(label) ? 'var(--selection-border)' : 'var(--surface-border)'}`,
                  cursor: 'pointer',
                  userSelect: 'none',
                }}
              >
                <input
                  type="checkbox"
                  checked={selected.has(label)}
                  onChange={() => togglePreset(label)}
                  aria-label={label}
                  style={{ width: 14, height: 14, accentColor: 'var(--accent)' }}
                />
                <FolderOpen size={15} style={{ color: 'var(--text-secondary)' }} />
                <span style={{ color: 'var(--text-primary)', fontSize: 'var(--font-size-sm)' }}>
                  {label}
                </span>
              </label>
            ))}
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            <button
              type="button"
              onClick={() => {
                handleAddCustomFolder().catch(() => {});
              }}
              style={{
                ...btnBase,
                border: '1px solid var(--surface-border)',
                background: 'rgba(255,255,255,0.05)',
                color: 'var(--text-primary)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                gap: 6,
              }}
            >
              <Plus size={13} />
              Add Custom Folder
            </button>

            {customFolders.map((path) => (
              <div
                key={path}
                data-testid="custom-folder-item"
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                  padding: '8px 12px',
                  borderRadius: 'var(--radius-element)',
                  background: 'rgba(255,255,255,0.04)',
                  border: '1px solid var(--surface-border)',
                }}
              >
                <span
                  style={{
                    color: 'var(--text-secondary)',
                    fontSize: 'var(--font-size-xs)',
                    fontFamily: 'var(--font-mono)',
                    flex: 1,
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    whiteSpace: 'nowrap',
                  }}
                >
                  {path}
                </span>
                <button
                  type="button"
                  aria-label={`Remove ${path}`}
                  onClick={() => removeCustomFolder(path)}
                  style={{ background: 'none', border: 'none', color: 'var(--text-tertiary)', cursor: 'pointer', display: 'flex' }}
                >
                  <X size={13} />
                </button>
              </div>
            ))}
          </div>

          <div style={{ display: 'flex', gap: 8 }}>
            <button
              onClick={() => setStep('privacy')}
              style={{ ...btnBase, background: 'var(--accent)', color: '#fff', flex: 1 }}
            >
              Next
            </button>
            <button
              onClick={() => onComplete(true)}
              style={{ ...btnBase, background: 'rgba(255,255,255,0.06)', color: 'var(--text-secondary)', border: '1px solid var(--surface-border)' }}
            >
              Skip for now
            </button>
          </div>
        </motion.div>
      )}

      {step === 'privacy' && (
        <motion.div
          data-testid="privacy-notice"
          variants={fadeSlide}
          initial="initial"
          animate="animate"
          style={{ display: 'flex', flexDirection: 'column', gap: 20, width: '100%', maxWidth: 400 }}
        >
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            <h1 style={{ fontSize: 'var(--font-size-lg)', fontWeight: 700, color: 'var(--text-primary)', margin: 0 }}>
              100% local & private
            </h1>
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            {[
              { icon: <Shield size={14} />, text: 'All indexing runs on your machine via Ollama — nothing leaves your device.' },
              { icon: <AlertTriangle size={14} />, text: 'WhatTheFile never reads, copies, or modifies your source files.' },
              { icon: <FolderOpen size={14} />, text: 'Indexed data is stored under your user data directory, not inside your folders.' },
            ].map(({ icon, text }, i) => (
              <div key={i} style={{ display: 'flex', gap: 10, alignItems: 'flex-start', color: 'var(--text-secondary)', fontSize: 'var(--font-size-sm)' }}>
                <span style={{ color: 'var(--accent)', flexShrink: 0, marginTop: 2 }}>{icon}</span>
                {text}
              </div>
            ))}
          </div>

          <button
            onClick={handleGetStarted}
            style={{ ...btnBase, background: 'var(--accent)', color: '#fff' }}
          >
            Get Started
          </button>
        </motion.div>
      )}
    </div>
  );
}
