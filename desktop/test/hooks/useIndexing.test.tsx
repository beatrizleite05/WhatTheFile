import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import type { ReactNode } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useIndexing, IndexingProvider } from '../../src/hooks/useIndexing';
import { startIndexing as apiStartIndexing, cancelIndexing as apiCancelIndexing } from '../../src/api/indexing';
import type { ProgressEvent } from '../../src/api/indexing';

vi.mock('../../src/api/indexing', () => ({
  startIndexing: vi.fn().mockResolvedValue(1),
  cancelIndexing: vi.fn().mockResolvedValue(undefined),
  getActivityLog: vi.fn().mockResolvedValue([]),
}));

const mockListen = vi.mocked(listen);
const mockApiStartIndexing = vi.mocked(apiStartIndexing);
const mockApiCancelIndexing = vi.mocked(apiCancelIndexing);

type TerminalHandler = (e: { payload: unknown }) => void;

beforeEach(() => {
  vi.clearAllMocks();
  vi.useFakeTimers();
  mockListen.mockResolvedValue(() => {});
});

afterEach(() => {
  vi.useRealTimers();
});

// Wrapper that mounts the provider so useIndexing() reads from context.
const wrapper = ({ children }: { children: ReactNode }) => (
  <IndexingProvider>{children}</IndexingProvider>
);

// Render the hook inside a provider and wait for the setup useEffect to settle.
async function renderAndSettle() {
  const hook = renderHook(() => useIndexing(), { wrapper });
  await act(async () => {}); // let the setup useEffect run
  return hook;
}

describe('useIndexing (provider)', () => {
  it('starts with empty jobs and null activeJob', async () => {
    const { result } = await renderAndSettle();
    expect(result.current.jobs).toEqual([]);
    expect(result.current.activeJob).toBeNull();
  });

  it('marks job completed when indexing://completed fires', async () => {
    const handlers: Record<string, TerminalHandler> = {};
    mockListen.mockImplementation((event, handler) => {
      handlers[event as string] = handler as TerminalHandler;
      return Promise.resolve(() => {});
    });

    const { result } = await renderAndSettle();

    act(() => {
      handlers['indexing://completed']?.({
        payload: { jobId: 1, rootId: 10, filesTotal: 50, filesAdded: 10, filesUpdated: 0, filesMoved: 0, filesDeleted: 0, errorCount: 0 },
      });
    });

    expect(result.current.jobs).toHaveLength(1);
    expect(result.current.jobs[0].jobId).toBe(1);
    expect(result.current.jobs[0].isComplete).toBe(true);
    expect(result.current.jobs[0].phase).toBe('completed');
    expect(result.current.activeJob).toBeNull();
  });

  it('marks job complete on indexing://completed even without prior progress event', async () => {
    const handlers: Record<string, TerminalHandler> = {};
    mockListen.mockImplementation((event, handler) => {
      handlers[event as string] = handler as TerminalHandler;
      return Promise.resolve(() => {});
    });

    const { result } = await renderAndSettle();

    act(() => {
      handlers['indexing://completed']?.({
        payload: { jobId: 42, rootId: 9, filesTotal: 5, filesAdded: 5, filesUpdated: 0, filesMoved: 0, filesDeleted: 0, errorCount: 0 },
      });
    });

    expect(result.current.jobs).toHaveLength(1);
    expect(result.current.jobs[0].jobId).toBe(42);
    expect(result.current.jobs[0].isComplete).toBe(true);
    expect(result.current.activeJob).toBeNull();
  });

  it('marks job cancelled when indexing://cancelled fires', async () => {
    const handlers: Record<string, TerminalHandler> = {};
    mockListen.mockImplementation((event, handler) => {
      handlers[event as string] = handler as TerminalHandler;
      return Promise.resolve(() => {});
    });

    const { result } = await renderAndSettle();

    act(() => {
      handlers['indexing://cancelled']?.({
        payload: { jobId: 3, rootId: 2 },
      });
    });

    expect(result.current.jobs).toHaveLength(1);
    expect(result.current.jobs[0].isComplete).toBe(true);
  });

  it('progress events arrive via channel callback and update state', async () => {
    let capturedOnProgress: ((p: ProgressEvent) => void) | null = null;
    let resolveStart!: (v: number) => void;
    mockApiStartIndexing.mockImplementation(async (_rootId, onProgress) => {
      capturedOnProgress = onProgress!;
      return new Promise<number>((r) => { resolveStart = r; });
    });

    const { result } = await renderAndSettle();

    // Start indexing (does not resolve yet so we can push events).
    let startDone = false;
    act(() => {
      result.current.startIndexing(10).then(() => { startDone = true; });
    });
    await act(async () => { await Promise.resolve(); });

    // Push first event.
    act(() => {
      capturedOnProgress?.({
        jobId: 1, rootId: 10, seq: 1, phase: 'discovering',
        filesTotal: 100, filesDone: 20, filesAdded: 5, filesUpdated: 0,
        filesMoved: 0, filesDeleted: 0, errorCount: 0,
        currentFile: null, extractionTotal: 0, extractionDone: 0,
      });
    });

    const job = result.current.jobs.find((j) => j.jobId === 1);
    expect(job).toBeDefined();
    expect(job?.phase).toBe('discovering');
    expect(job?.filesDone).toBe(20);

    // Push second event — should update, not add a duplicate.
    act(() => {
      capturedOnProgress?.({
        jobId: 1, rootId: 10, seq: 2, phase: 'extracting',
        filesTotal: 100, filesDone: 60, filesAdded: 5, filesUpdated: 10,
        filesMoved: 0, filesDeleted: 0, errorCount: 0,
        currentFile: 'notes.txt', extractionTotal: 10, extractionDone: 3,
      });
    });

    const activeJobs = result.current.jobs.filter((j) => j.jobId === 1);
    expect(activeJobs).toHaveLength(1);
    expect(activeJobs[0].phase).toBe('extracting');
    expect(activeJobs[0].currentFile).toBe('notes.txt');

    // Resolve start so cleanup happens.
    await act(async () => { resolveStart(1); });
    expect(startDone).toBe(true);
  });

  it('drops progress events with stale seq', async () => {
    let capturedOnProgress: ((p: ProgressEvent) => void) | null = null;
    mockApiStartIndexing.mockImplementation(async (_rootId, onProgress) => {
      capturedOnProgress = onProgress!;
      return new Promise<number>(() => {}); // hangs
    });

    const { result } = await renderAndSettle();

    act(() => {
      result.current.startIndexing(5);
    });
    await act(async () => { await Promise.resolve(); });

    // Higher seq arrives first.
    act(() => {
      capturedOnProgress?.({
        jobId: 1, rootId: 5, seq: 2, phase: 'extracting',
        filesTotal: 10, filesDone: 5, filesAdded: 0, filesUpdated: 0,
        filesMoved: 0, filesDeleted: 0, errorCount: 0,
        currentFile: null, extractionTotal: 0, extractionDone: 0,
      });
    });
    // Stale seq=1 arrives late — must be dropped.
    act(() => {
      capturedOnProgress?.({
        jobId: 1, rootId: 5, seq: 1, phase: 'discovering',
        filesTotal: 10, filesDone: 1, filesAdded: 0, filesUpdated: 0,
        filesMoved: 0, filesDeleted: 0, errorCount: 0,
        currentFile: null, extractionTotal: 0, extractionDone: 0,
      });
    });

    const job = result.current.jobs.find((j) => j.jobId === 1);
    expect(job?.phase).toBe('extracting'); // seq=1 was dropped; seq=2 state kept
    expect(job?.filesDone).toBe(5);
  });

  it('cancelIndexing optimistically dismisses the hero before the cancelled event arrives', async () => {
    let capturedOnProgress: ((p: ProgressEvent) => void) | null = null;
    mockApiStartIndexing.mockImplementation(async (_rootId, onProgress) => {
      capturedOnProgress = onProgress!;
      return new Promise<number>(() => {}); // hangs
    });

    const { result } = await renderAndSettle();

    act(() => { result.current.startIndexing(3); });
    await act(async () => { await Promise.resolve(); });

    act(() => {
      capturedOnProgress?.({
        jobId: 5, rootId: 3, seq: 1, phase: 'extracting',
        filesTotal: 100, filesDone: 5, filesAdded: 5, filesUpdated: 0,
        filesMoved: 0, filesDeleted: 0, errorCount: 0,
        currentFile: null, extractionTotal: 0, extractionDone: 0,
      });
    });
    expect(result.current.activeJob).not.toBeNull();

    await act(async () => { await result.current.cancelIndexing(); });

    expect(result.current.activeJob).toBeNull();
    expect(mockApiCancelIndexing).toHaveBeenCalled();
    expect(result.current.jobs.find((j) => j.jobId === 5)?.isComplete).toBe(true);
  });

  it('computes progressPercent from filesDone / filesTotal', async () => {
    let capturedOnProgress: ((p: ProgressEvent) => void) | null = null;
    mockApiStartIndexing.mockImplementation(async (_rootId, onProgress) => {
      capturedOnProgress = onProgress!;
      return new Promise<number>(() => {});
    });

    const { result } = await renderAndSettle();

    act(() => { result.current.startIndexing(10); });
    await act(async () => { await Promise.resolve(); });

    act(() => {
      capturedOnProgress?.({
        jobId: 1, rootId: 10, seq: 1, phase: 'extracting',
        filesTotal: 200, filesDone: 50, filesAdded: 0, filesUpdated: 0,
        filesMoved: 0, filesDeleted: 0, errorCount: 0,
        currentFile: null, extractionTotal: 0, extractionDone: 0,
      });
    });

    expect(result.current.jobs.find((j) => j.jobId === 1)?.progressPercent).toBe(25);
  });

  it('progressPercent is 0 when filesTotal is 0', async () => {
    let capturedOnProgress: ((p: ProgressEvent) => void) | null = null;
    mockApiStartIndexing.mockImplementation(async (_rootId, onProgress) => {
      capturedOnProgress = onProgress!;
      return new Promise<number>(() => {});
    });

    const { result } = await renderAndSettle();

    act(() => { result.current.startIndexing(10); });
    await act(async () => { await Promise.resolve(); });

    act(() => {
      capturedOnProgress?.({
        jobId: 1, rootId: 10, seq: 1, phase: 'discovering',
        filesTotal: 0, filesDone: 0, filesAdded: 0, filesUpdated: 0,
        filesMoved: 0, filesDeleted: 0, errorCount: 0,
        currentFile: null, extractionTotal: 0, extractionDone: 0,
      });
    });

    expect(result.current.jobs.find((j) => j.jobId === 1)?.progressPercent).toBe(0);
  });

  it('calls unlisten on unmount', async () => {
    const unlisten = vi.fn();
    mockListen.mockResolvedValue(unlisten);

    const { unmount } = await renderAndSettle();
    unmount();

    expect(unlisten).toHaveBeenCalled();
  });
});
