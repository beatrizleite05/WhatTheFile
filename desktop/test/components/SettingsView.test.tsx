import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { SettingsView } from '../../src/components/SettingsView';

const useSettingsMock = vi.fn();
const useIndexingMock = vi.fn();

vi.mock('../../src/hooks/useSettings', () => ({
  useSettings: () => useSettingsMock(),
}));

vi.mock('../../src/hooks/useIndexing', () => ({
  useIndexing: () => useIndexingMock(),
}));

describe('SettingsView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSettingsMock.mockReturnValue({
      roots: [],
      loading: false,
      error: null,
      addRoot: vi.fn(),
      removeRoot: vi.fn(),
      reindex: vi.fn(),
    });
    useIndexingMock.mockReturnValue({
      jobs: [
        {
          jobId: 1,
          rootId: 2,
          phase: 'completed',
          filesTotal: 10,
          filesDone: 10,
          filesAdded: 5,
          filesUpdated: 0,
          filesMoved: 0,
          filesDeleted: 1,
          errorCount: 0,
          progressPercent: 100,
          isComplete: true,
        },
      ],
      activeJob: null,
    });
  });

  it('renders main settings sections', () => {
    render(<SettingsView onBack={vi.fn()} onResetOnboarding={vi.fn()} />);
    expect(screen.getByRole('heading', { name: /Settings/i })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /Indexed Folders/i })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /Activity Log/i })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /Data/i })).toBeInTheDocument();
  });

  it('renders activity entries from indexing jobs', () => {
    render(<SettingsView onBack={vi.fn()} onResetOnboarding={vi.fn()} />);
    expect(screen.getByText(/Root #2/i)).toBeInTheDocument();
    expect(screen.getByText(/\+5 added/i)).toBeInTheDocument();
  });

  it('renders delete index action', () => {
    render(<SettingsView onBack={vi.fn()} onResetOnboarding={vi.fn()} />);
    expect(screen.getByRole('button', { name: /Delete all indexed data/i })).toBeInTheDocument();
  });
});
