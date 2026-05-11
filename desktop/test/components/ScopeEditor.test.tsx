import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { open } from '@tauri-apps/plugin-dialog';
import { ScopeEditor } from '../../src/components/ScopeEditor';

const mockOpen = vi.mocked(open);

const settings = {
  roots: [
    {
      id: 1,
      path: '/Users/test/Documents',
      label: 'Documents',
      active: true,
      createdAt: 100,
      lastIndexedAt: null,
    },
  ],
  loading: false,
  error: null,
  indexingRootIds: new Set<number>(),
  addRoot: vi.fn().mockResolvedValue(undefined),
  removeRoot: vi.fn().mockResolvedValue(undefined),
  reindex: vi.fn(),
  deleteIndex: vi.fn().mockResolvedValue(undefined),
};

describe('ScopeEditor', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockOpen.mockResolvedValue(null);
  });

  it('shows loading state', () => {
    render(<ScopeEditor settings={{ ...settings, loading: true }} />);
    expect(screen.getByText(/Loading/i)).toBeInTheDocument();
  });

  it('shows empty state when no roots exist', () => {
    render(<ScopeEditor settings={{ ...settings, roots: [] }} />);
    expect(screen.getByText(/No folders indexed yet/i)).toBeInTheDocument();
  });

  it('adds root from dialog picker', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const addRoot = vi.fn().mockResolvedValue(undefined);
    mockOpen.mockResolvedValue('/Users/test/Pictures');

    render(<ScopeEditor settings={{ ...settings, addRoot }} />);
    await user.click(screen.getByRole('button', { name: /Add folder/i }));

    expect(addRoot).toHaveBeenCalledWith('/Users/test/Pictures');
  });

  it('calls reindex for a root', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const reindex = vi.fn();
    render(<ScopeEditor settings={{ ...settings, reindex }} />);

    await user.click(screen.getByRole('button', { name: /Reindex Documents/i }));
    expect(reindex).toHaveBeenCalledWith(1);
  });

  it('calls remove for a root', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const removeRoot = vi.fn().mockResolvedValue(undefined);
    render(<ScopeEditor settings={{ ...settings, removeRoot }} />);

    await user.click(screen.getByRole('button', { name: /Remove Documents/i }));
    expect(removeRoot).toHaveBeenCalledWith(1);
  });
});
