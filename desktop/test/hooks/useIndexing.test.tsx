import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { listen } from '@tauri-apps/api/event';
import { useIndexing } from '../../src/hooks/useIndexing';
import { startIndexing as apiStartIndexing, cancelIndexing as apiCancelIndexing } from '../../src/api/indexing';

vi.mock('../../src/api/indexing', () => ({
  startIndexing: vi.fn().mockResolvedValue(1),
  cancelIndexing: vi.fn().mockResolvedValue(undefined),
  getActivityLog: vi.fn().mockResolvedValue([]),
}));

const mockListen = vi.mocked(listen);
const mockApiStartIndexing = vi.mocked(apiStartIndexing);
const mockApiCancelIndexing = vi.mocked(apiCancelIndexing);

type EventHandler = (e: { payload: unknown }) => void;

beforeEach(() => {
  vi.clearAllMocks();
  vi.useFakeTimers();
  mockListen.mockResolvedValue(() => {});
});

afterEach(() => {
  vi.useRealTimers();
});

function captureHandlers(): Record<string, EventHandler> {
  const handlers: Record<string, EventHandler> = {};
  mockListen.mockImplementation((event, handler) => {
    handlers[event as string] = handler as EventHandler;
    return Promise.resolve(() => {});
  });
  return handlers;
}

describe('useIndexing', () => {
  it('starts with empty jobs and null activeJob', () => {
    const { result } = renderHook(() => useIndexing());
    expect(result.current.jobs).toEqual([]);
    expect(result.current.activeJob).toBeNull();
  });

  it('creates a job when indexing://progress fires', async () => {
    const handlers = captureHandlers();
    const { result } = renderHook(() => useIndexing());
    await act(async () => {});

    act(() => {
      handlers['indexing://progress']?.({
        payload: { jobId: 1, rootId: 10, phase: 'discovering', filesTotal: 100, filesDone: 20, filesAdded: 5, filesUpdated: 3, filesMoved: 0, filesDeleted: 0, errorCount: 0 },
      });
    });

    expect(result.current.jobs).toHaveLength(1);
    expect(result.current.jobs[0].jobId).toBe(1);
    expect(result.current.jobs[0].phase).toBe('discovering');
    expect(result.current.jobs[0].filesDone).toBe(20);
  });

  it('updates existing job on duplicate progress (no duplicate entries)', async () => {
    const handlers = captureHandlers();
    const { result } = renderHook(() => useIndexing());
    await act(async () => {});

    act(() => {
      handlers['indexing://progress']?.({ payload: { jobId: 1, rootId: 10, phase: 'discovering', filesTotal: 100, filesDone: 20, filesAdded: 5, filesUpdated: 0, filesMoved: 0, filesDeleted: 0, errorCount: 0 } });
    });
    act(() => {
      handlers['indexing://progress']?.({ payload: { jobId: 1, rootId: 10, phase: 'extracting', filesTotal: 100, filesDone: 60, filesAdded: 5, filesUpdated: 10, filesMoved: 0, filesDeleted: 0, errorCount: 0 } });
    });

    expect(result.current.jobs).toHaveLength(1);
    expect(result.current.jobs[0].filesDone).toBe(60);
    expect(result.current.jobs[0].phase).toBe('extracting');
  });

  it('marks job completed when indexing://completed fires', async () => {
    const handlers = captureHandlers();
    const { result } = renderHook(() => useIndexing());
    await act(async () => {});

    act(() => {
      handlers['indexing://progress']?.({ payload: { jobId: 1, rootId: 10, phase: 'extracting', filesTotal: 50, filesDone: 50, filesAdded: 10, filesUpdated: 0, filesMoved: 0, filesDeleted: 0, errorCount: 0 } });
    });
    act(() => {
      handlers['indexing://completed']?.({ payload: { jobId: 1, rootId: 10, filesTotal: 50, filesAdded: 10, filesUpdated: 0, filesMoved: 0, filesDeleted: 0, errorCount: 0 } });
    });

    expect(result.current.jobs[0].isComplete).toBe(true);
    expect(result.current.jobs[0].phase).toBe('completed');
    expect(result.current.activeJob).toBeNull();
  });

  it('computes progressPercent as filesDone / filesTotal * 100', async () => {
    const handlers = captureHandlers();
    const { result } = renderHook(() => useIndexing());
    await act(async () => {});

    act(() => {
      handlers['indexing://progress']?.({ payload: { jobId: 1, rootId: 10, phase: 'extracting', filesTotal: 200, filesDone: 50, filesAdded: 0, filesUpdated: 0, filesMoved: 0, filesDeleted: 0, errorCount: 0 } });
    });

    expect(result.current.jobs[0].progressPercent).toBe(25);
  });

  it('progressPercent is 0 when filesTotal is 0', async () => {
    const handlers = captureHandlers();
    const { result } = renderHook(() => useIndexing());
    await act(async () => {});

    act(() => {
      handlers['indexing://progress']?.({ payload: { jobId: 1, rootId: 10, phase: 'discovering', filesTotal: 0, filesDone: 0, filesAdded: 0, filesUpdated: 0, filesMoved: 0, filesDeleted: 0, errorCount: 0 } });
    });

    expect(result.current.jobs[0].progressPercent).toBe(0);
  });

  it('activeJob is the most recent non-completed job', async () => {
    const handlers = captureHandlers();
    const { result } = renderHook(() => useIndexing());
    await act(async () => {});

    act(() => {
      handlers['indexing://progress']?.({ payload: { jobId: 2, rootId: 20, phase: 'discovering', filesTotal: 10, filesDone: 0, filesAdded: 0, filesUpdated: 0, filesMoved: 0, filesDeleted: 0, errorCount: 0 } });
    });

    expect(result.current.activeJob?.jobId).toBe(2);
  });

  it('calls unlisten on unmount', async () => {
    const unlisten = vi.fn();
    mockListen.mockResolvedValue(unlisten);

    const { unmount } = renderHook(() => useIndexing());
    await act(async () => {});
    unmount();

    expect(unlisten).toHaveBeenCalled();
  });

  it('exposes currentFile from progress payload', async () => {
    const handlers = captureHandlers();
    const { result } = renderHook(() => useIndexing());
    await act(async () => {});

    act(() => {
      handlers['indexing://progress']?.({
        payload: { jobId: 1, rootId: 10, phase: 'extracting', filesTotal: 5, filesDone: 5, filesAdded: 5, filesUpdated: 0, filesMoved: 0, filesDeleted: 0, errorCount: 0, currentFile: 'reports/q1.pdf', extractionTotal: 5, extractionDone: 2 },
      });
    });

    expect(result.current.jobs[0].currentFile).toBe('reports/q1.pdf');
  });

  it('uses extraction sub-counts for progressPercent during extracting phase', async () => {
    const handlers = captureHandlers();
    const { result } = renderHook(() => useIndexing());
    await act(async () => {});

    act(() => {
      handlers['indexing://progress']?.({
        payload: { jobId: 1, rootId: 10, phase: 'extracting', filesTotal: 100, filesDone: 100, filesAdded: 100, filesUpdated: 0, filesMoved: 0, filesDeleted: 0, errorCount: 0, currentFile: null, extractionTotal: 4, extractionDone: 1 },
      });
    });

    expect(result.current.jobs[0].progressPercent).toBe(25);
  });

  it('falls back to filesDone/filesTotal when phase is not extracting', async () => {
    const handlers = captureHandlers();
    const { result } = renderHook(() => useIndexing());
    await act(async () => {});

    act(() => {
      handlers['indexing://progress']?.({
        payload: { jobId: 1, rootId: 10, phase: 'fingerprinting', filesTotal: 8, filesDone: 2, filesAdded: 0, filesUpdated: 0, filesMoved: 0, filesDeleted: 0, errorCount: 0, currentFile: null, extractionTotal: 0, extractionDone: 0 },
      });
    });

    expect(result.current.jobs[0].progressPercent).toBe(25);
  });

  it('preserves currentFile across throttled progress events without it', async () => {
    const handlers = captureHandlers();
    const { result } = renderHook(() => useIndexing());
    await act(async () => {});

    act(() => {
      handlers['indexing://progress']?.({
        payload: { jobId: 1, rootId: 10, phase: 'extracting', filesTotal: 3, filesDone: 3, filesAdded: 3, filesUpdated: 0, filesMoved: 0, filesDeleted: 0, errorCount: 0, currentFile: 'notes.md', extractionTotal: 3, extractionDone: 1 },
      });
    });
    act(() => {
      handlers['indexing://progress']?.({
        payload: { jobId: 1, rootId: 10, phase: 'extracting', filesTotal: 3, filesDone: 3, filesAdded: 3, filesUpdated: 0, filesMoved: 0, filesDeleted: 0, errorCount: 0, currentFile: null, extractionTotal: 3, extractionDone: 2 },
      });
    });

    expect(result.current.jobs[0].currentFile).toBe('notes.md');
  });

  it('startIndexing awaits listener registration before invoking the IPC', async () => {
    let resolveListen: (() => void) | null = null;
    mockListen.mockImplementation(() => {
      return new Promise<() => void>((resolve) => {
        resolveListen = () => resolve(() => {});
      });
    });

    const { result } = renderHook(() => useIndexing());
    await act(async () => {});

    const startPromise = result.current.startIndexing(7);
    expect(mockApiStartIndexing).not.toHaveBeenCalled();

    await act(async () => {
      while (resolveListen) {
        const r = resolveListen;
        resolveListen = null;
        r();
        await Promise.resolve();
      }
    });
    await act(async () => { await startPromise; });

    expect(mockApiStartIndexing).toHaveBeenCalledWith(7);
  });

  it('marks job complete on indexing://completed even without prior progress event', async () => {
    const handlers = captureHandlers();
    const { result } = renderHook(() => useIndexing());
    await act(async () => {});

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

  it('cancelIndexing optimistically dismisses the hero before the cancelled event arrives', async () => {
    const handlers = captureHandlers();
    const { result } = renderHook(() => useIndexing());
    await act(async () => {});

    act(() => {
      handlers['indexing://progress']?.({
        payload: { jobId: 5, rootId: 3, phase: 'extracting', filesTotal: 100, filesDone: 5, filesAdded: 5, filesUpdated: 0, filesMoved: 0, filesDeleted: 0, errorCount: 0, currentFile: 'foo.pdf', extractionTotal: 10, extractionDone: 1 },
      });
    });
    expect(result.current.activeJob).not.toBeNull();

    await act(async () => { await result.current.cancelIndexing(); });

    expect(result.current.activeJob).toBeNull();
    expect(mockApiCancelIndexing).toHaveBeenCalled();
    expect(result.current.jobs[0].isComplete).toBe(true);
  });
});
