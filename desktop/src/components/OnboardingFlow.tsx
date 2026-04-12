import { useState } from 'react';
import { motion } from 'framer-motion';
import { FolderOpen, Shield, AlertTriangle } from 'lucide-react';
import { fadeSlide } from '../styles/animations';
import type { useSettings } from '../hooks/useSettings';

type SettingsHook = ReturnType<typeof useSettings>;

interface OnboardingFlowProps {
  onComplete: () => void;
  settings: SettingsHook;
}

const PRESETS = [
  { label: 'Documents', pathSuffix: 'Documents' },
  { label: 'Desktop', pathSuffix: 'Desktop' },
  { label: 'Downloads', pathSuffix: 'Downloads' },
  { label: 'Pictures', pathSuffix: 'Pictures' },
];

function getHomePath(): string {
  // In the browser/Tauri context, home dir is approximated
  return typeof window !== 'undefined' && navigator.platform.includes('Win')
    ? `C:\\Users\\${import.meta.env.VITE_USER ?? 'user'}`
    : `/Users/${import.meta.env.VITE_USER ?? 'user'}`;
}

export function OnboardingFlow({ onComplete, settings }: OnboardingFlowProps) {
  const [step, setStep] = useState<'folders' | 'privacy'>('folders');
  const [selected, setSelected] = useState<Set<string>>(new Set());

  function togglePreset(suffix: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(suffix)) next.delete(suffix);
      else next.add(suffix);
      return next;
    });
  }

  async function handleGetStarted() {
    const home = getHomePath();
    for (const suffix of selected) {
      const sep = home.includes('\\') ? '\\' : '/';
      await settings.addRoot(`${home}${sep}${suffix}`);
    }
    onComplete();
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
            {PRESETS.map(({ label, pathSuffix }) => (
              <label
                key={pathSuffix}
                aria-label={label}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 10,
                  padding: '10px 14px',
                  borderRadius: 'var(--radius-element)',
                  background: selected.has(pathSuffix) ? 'var(--selection-bg)' : 'rgba(255,255,255,0.04)',
                  border: `1px solid ${selected.has(pathSuffix) ? 'var(--selection-border)' : 'var(--surface-border)'}`,
                  cursor: 'pointer',
                  userSelect: 'none',
                }}
              >
                <input
                  type="checkbox"
                  checked={selected.has(pathSuffix)}
                  onChange={() => togglePreset(pathSuffix)}
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

          <div style={{ display: 'flex', gap: 8 }}>
            <button
              onClick={() => setStep('privacy')}
              style={{ ...btnBase, background: 'var(--accent)', color: '#fff', flex: 1 }}
            >
              Next
            </button>
            <button
              onClick={onComplete}
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
