import { describe, it, expect, vi, beforeAll } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { FramelessOverlay } from '../../src/components/FramelessOverlay';
import type { UseSearchReturn } from '../../src/hooks/useSearch';
import type { UseOllamaStatusReturn } from '../../src/hooks/useOllamaStatus';
import type { IndexingJob } from '../../src/hooks/useIndexing';
import type { FileResult } from '../../src/core/types';

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

const noop = vi.fn();

const makeResult = (id: number): FileResult => ({
  fileId: id,
  rootId: 1,
  path: `/docs/file${id}.txt`,
  filename: `file${id}.txt`,
  mediaType: 'txt',
  sizeBytes: 1024,
  indexedAt: 1000,
  confidence: 1,
  snippet: `snippet ${id}`,
  score: 0.9,
});

const mockGetCurrentWindow = vi.mocked(getCurrentWindow);

beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, 'getBoundingClientRect', {
    configurable: true,
    value: () => ({ width: 600, height: 480, top: 0, left: 0, bottom: 480, right: 600 }),
  });
  Object.defineProperty(HTMLElement.prototype, 'offsetHeight', { configurable: true, get: () => 480 });
  Object.defineProperty(HTMLElement.prototype, 'offsetWidth', { configurable: true, get: () => 600 });
});

describe('FramelessOverlay', () => {
  it('renders search bar', () => {
    render(<FramelessOverlay search={makeSearch()} indexing={makeIndexing()} ollamaStatus={makeOllama()} onOpenSettings={vi.fn()} />);
    expect(screen.getByRole('searchbox')).toBeInTheDocument();
  });

  it('shows OllamaBanner when ollama is not reachable', () => {
    render(<FramelessOverlay search={makeSearch()} indexing={makeIndexing()} ollamaStatus={makeOllama({ reachable: false })} onOpenSettings={noop} />);
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });

  it('shows skip warning banner when requested', () => {
    render(
      <FramelessOverlay
        search={makeSearch()}
        indexing={makeIndexing()}
        ollamaStatus={makeOllama()}
        showSkipWarning={true}
        onOpenSettings={noop}
      />
    );
    expect(screen.getByTestId('skip-warning-banner')).toBeInTheDocument();
  });

  it('hides skip warning banner by default', () => {
    render(<FramelessOverlay search={makeSearch()} indexing={makeIndexing()} ollamaStatus={makeOllama()} onOpenSettings={vi.fn()} />);
    expect(screen.queryByTestId('skip-warning-banner')).not.toBeInTheDocument();
  });

  it('does not show OllamaBanner when reachable', () => {
    render(<FramelessOverlay search={makeSearch()} indexing={makeIndexing()} ollamaStatus={makeOllama({ reachable: true })} onOpenSettings={noop} />);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('shows IndexingProgress when activeJob exists', () => {
    const activeJob: IndexingJob = {
      jobId: 1, rootId: 1, phase: 'extracting',
      filesTotal: 100, filesDone: 50, filesAdded: 20, filesUpdated: 0,
      filesMoved: 0, filesDeleted: 0, errorCount: 0, progressPercent: 50, isComplete: false,
    };
    render(<FramelessOverlay search={makeSearch()} indexing={makeIndexing({ activeJob })} ollamaStatus={makeOllama()} onOpenSettings={noop} />);
    expect(screen.getByRole('progressbar')).toBeInTheDocument();
  });

  it('hides window when Escape pressed with empty query', async () => {
    const hide = vi.fn().mockResolvedValue(undefined);
    mockGetCurrentWindow.mockReturnValue({ hide } as unknown as ReturnType<typeof getCurrentWindow>);
    const user = userEvent.setup({ advanceTimers: () => {} });
    render(<FramelessOverlay search={makeSearch({ query: '' })} indexing={makeIndexing()} ollamaStatus={makeOllama()} onOpenSettings={noop} />);
    await user.keyboard('{Escape}');
    expect(hide).toHaveBeenCalled();
  });

  it('clears query on Escape when query is non-empty', async () => {
    const clearQuery = vi.fn();
    const user = userEvent.setup({ advanceTimers: () => {} });
    render(<FramelessOverlay search={makeSearch({ query: 'invoices', clearQuery })} indexing={makeIndexing()} ollamaStatus={makeOllama()} onOpenSettings={noop} />);
    await user.keyboard('{Escape}');
    expect(clearQuery).toHaveBeenCalled();
  });

  it('uses full-width results pane when no preview is selected', () => {
    render(<FramelessOverlay search={makeSearch()} indexing={makeIndexing()} ollamaStatus={makeOllama()} onOpenSettings={vi.fn()} />);
    const resultsPane = screen.getByTestId('overlay-results-pane');
    expect(resultsPane).toHaveStyle({ flex: '1' });
    expect(screen.queryByTestId('overlay-preview-pane')).not.toBeInTheDocument();
  });

  it('uses 60/40 split when a preview is selected', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const search = makeSearch({ results: [makeResult(1)] });
    render(<FramelessOverlay search={search} indexing={makeIndexing()} ollamaStatus={makeOllama()} onOpenSettings={noop} />);

    await user.click(screen.getByTestId('result-tile'));

    expect(screen.getByTestId('overlay-results-pane')).toHaveStyle({ flex: '3' });
    expect(screen.getByTestId('overlay-preview-pane')).toHaveStyle({ flex: '2' });
  });

  it('opens and closes full preview modal with Space and Escape', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const search = makeSearch({ results: [makeResult(1)] });
    render(<FramelessOverlay search={search} indexing={makeIndexing()} ollamaStatus={makeOllama()} onOpenSettings={noop} />);

    await user.click(screen.getByTestId('result-tile'));
    expect(screen.queryByTestId('preview-modal')).not.toBeInTheDocument();

    await user.keyboard(' ');
    expect(screen.getByTestId('preview-modal')).toBeInTheDocument();

    await user.keyboard('{Escape}');
    expect(screen.queryByTestId('preview-modal')).not.toBeInTheDocument();
  });
});
