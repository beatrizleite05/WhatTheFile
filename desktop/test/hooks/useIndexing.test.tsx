import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { listen } from '@tauri-apps/api/event';
import { useIndexing } from '../../src/hooks/useIndexing';

const mockListen = vi.mocked(listen);

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
});
