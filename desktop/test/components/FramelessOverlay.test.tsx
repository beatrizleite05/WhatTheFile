import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { FramelessOverlay } from '../../src/components/FramelessOverlay';
import type { UseSearchReturn } from '../../src/hooks/useSearch';
import type { UseOllamaStatusReturn } from '../../src/hooks/useOllamaStatus';
import type { IndexingJob } from '../../src/hooks/useIndexing';

interface IndexingState {
  jobs: IndexingJob[];
  activeJob: IndexingJob | null;
}

const makeSearch = (overrides: Partial<UseSearchReturn> = {}): UseSearchReturn => ({
  query: '', setQuery: vi.fn(), mode: 'hybrid', setMode: vi.fn(),
  results: [], total: 0, hasMore: false, loading: false, error: null,
  parsedRequest: null, loadMore: vi.fn(), clearQuery: vi.fn(),
  ...overrides,
});

const makeIndexing = (overrides: Partial<IndexingState> = {}): IndexingState => ({
  jobs: [], activeJob: null, ...overrides,
});

const makeOllama = (overrides: Partial<UseOllamaStatusReturn> = {}): UseOllamaStatusReturn => ({
  reachable: true, modelsLoaded: [], loading: false, checkNow: vi.fn(), ...overrides,
});

const mockGetCurrentWindow = vi.mocked(getCurrentWindow);

describe('FramelessOverlay', () => {
  it('renders search bar', () => {
    render(<FramelessOverlay search={makeSearch()} indexing={makeIndexing()} ollamaStatus={makeOllama()} />);
    expect(screen.getByRole('searchbox')).toBeInTheDocument();
  });

  it('shows OllamaBanner when ollama is not reachable', () => {
    render(<FramelessOverlay search={makeSearch()} indexing={makeIndexing()} ollamaStatus={makeOllama({ reachable: false })} />);
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });

  it('does not show OllamaBanner when reachable', () => {
    render(<FramelessOverlay search={makeSearch()} indexing={makeIndexing()} ollamaStatus={makeOllama({ reachable: true })} />);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('shows IndexingProgress when activeJob exists', () => {
    const activeJob: IndexingJob = {
      jobId: 1, rootId: 1, phase: 'extracting',
      filesTotal: 100, filesDone: 50, filesAdded: 20, filesUpdated: 0,
      filesMoved: 0, filesDeleted: 0, errorCount: 0, progressPercent: 50, isComplete: false,
    };
    render(<FramelessOverlay search={makeSearch()} indexing={makeIndexing({ activeJob })} ollamaStatus={makeOllama()} />);
    expect(screen.getByRole('progressbar')).toBeInTheDocument();
  });

  it('hides window when Escape pressed with empty query', async () => {
    const hide = vi.fn().mockResolvedValue(undefined);
    mockGetCurrentWindow.mockReturnValue({ hide } as unknown as ReturnType<typeof getCurrentWindow>);
    const user = userEvent.setup({ advanceTimers: () => {} });
    render(<FramelessOverlay search={makeSearch({ query: '' })} indexing={makeIndexing()} ollamaStatus={makeOllama()} />);
    await user.keyboard('{Escape}');
    expect(hide).toHaveBeenCalled();
  });

  it('clears query on Escape when query is non-empty', async () => {
    const clearQuery = vi.fn();
    const user = userEvent.setup({ advanceTimers: () => {} });
    render(<FramelessOverlay search={makeSearch({ query: 'invoices', clearQuery })} indexing={makeIndexing()} ollamaStatus={makeOllama()} />);
    await user.keyboard('{Escape}');
    expect(clearQuery).toHaveBeenCalled();
  });
});
