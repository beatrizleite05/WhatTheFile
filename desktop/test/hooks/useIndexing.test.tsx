import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import type { ReactNode } from 'react';
import { useIndexing, IndexingProvider } from '../../src/hooks/useIndexing';
import {
  startIndexing as apiStartIndexing,
  cancelIndexing as apiCancelIndexing,
  getIndexingProgress as apiGetIndexingProgress,
  getActivityLog as apiGetActivityLog,
  type ProgressSnapshot,
} from '../../src/api/indexing';

vi.mock('../../src/api/indexing', () => ({
  startIndexing: vi.fn().mockResolvedValue(undefined),
  cancelIndexing: vi.fn().mockResolvedValue(undefined),
  getIndexingProgress: vi.fn().mockResolvedValue(null),
  getActivityLog: vi.fn().mockResolvedValue([]),
}));

const mockStart = vi.mocked(apiStartIndexing);
const mockCancel = vi.mocked(apiCancelIndexing);
const mockGetProgress = vi.mocked(apiGetIndexingProgress);
const mockGetActivity = vi.mocked(apiGetActivityLog);

beforeEach(() => {
  vi.clearAllMocks();
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

const wrapper = ({ children }: { children: ReactNode }) => (
  <IndexingProvider>{children}</IndexingProvider>
);

function snap(overrides: Partial<ProgressSnapshot> = {}): ProgressSnapshot {
  return {
    jobId: 42,
    rootId: 1,
    seq: 1,
    phase: 'discovering',
    filesTotal: 0,
    filesDone: 0,
    filesAdded: 0,
    filesUpdated: 0,
    filesMoved: 0,
    filesDeleted: 0,
    errorCount: 0,
    currentFile: null,
    extractionTotal: 0,
    extractionDone: 0,
    isComplete: false,
    ...overrides,
  };
}

describe('useIndexing (polling provider)', () => {
  it('starts with empty jobs and null activeJob', () => {
    const { result } = renderHook(() => useIndexing(), { wrapper });
    expect(result.current.jobs).toEqual([]);
    expect(result.current.activeJob).toBeNull();
  });

  it('shows pending placeholder immediately on startIndexing', async () => {
    mockGetProgress.mockResolvedValue(null);
    const { result } = renderHook(() => useIndexing(), { wrapper });
    await act(async () => { await result.current.startIndexing(1); });
    expect(result.current.activeJob).not.toBeNull();
    expect(result.current.activeJob?.rootId).toBe(1);
    expect(result.current.activeJob?.filesTotal).toBe(0);
  });

  it('upserts the real job from a poll snapshot', async () => {
    const { result } = renderHook(() => useIndexing(), { wrapper });
    mockGetProgress.mockResolvedValue(snap({ jobId: 99, filesTotal: 10, filesDone: 3, phase: 'fingerprinting' }));
    await act(async () => { await result.current.startIndexing(1); });
    // First pollOnce runs synchronously inside startPolling; await its microtask.
    await act(async () => { await Promise.resolve(); });
    expect(result.current.activeJob?.jobId).toBe(99);
    expect(result.current.activeJob?.filesTotal).toBe(10);
    expect(result.current.activeJob?.filesDone).toBe(3);
    expect(result.current.activeJob?.phase).toBe('fingerprinting');
  });

  it('drops stale snapshots with seq <= last seen', async () => {
    const { result } = renderHook(() => useIndexing(), { wrapper });
    mockGetProgress.mockResolvedValueOnce(snap({ seq: 5, filesDone: 5 }));
    await act(async () => { await result.current.startIndexing(1); });
    await act(async () => { await Promise.resolve(); });
    expect(result.current.activeJob?.filesDone).toBe(5);

    mockGetProgress.mockResolvedValueOnce(snap({ seq: 3, filesDone: 99 })); // stale
    await act(async () => { vi.advanceTimersByTime(250); });
    await act(async () => { await Promise.resolve(); });
    expect(result.current.activeJob?.filesDone).toBe(5);
  });

  it('marks job complete and stops polling when snapshot.isComplete=true', async () => {
    const { result } = renderHook(() => useIndexing(), { wrapper });
    mockGetProgress.mockResolvedValueOnce(snap({ seq: 1, filesDone: 5, filesTotal: 5, isComplete: true }));
    mockGetActivity.mockResolvedValue([
      { jobId: 42, rootId: 1, filesTotal: 5, filesAdded: 5, filesUpdated: 0, filesMoved: 0, filesDeleted: 0, errorCount: 0, completedAt: 1700000000 },
    ]);

    await act(async () => { await result.current.startIndexing(1); });
    await act(async () => { await Promise.resolve(); await Promise.resolve(); });

    expect(result.current.activeJob).toBeNull();
    expect(result.current.jobs[0].isComplete).toBe(true);
    expect(result.current.jobs[0].phase).toBe('completed');
    expect(result.current.jobs[0].filesAdded).toBe(5);
    expect(result.current.jobs[0].completedAt).toBe(1700000000);

    const callsBefore = mockGetProgress.mock.calls.length;
    await act(async () => { vi.advanceTimersByTime(1000); });
    expect(mockGetProgress.mock.calls.length).toBe(callsBefore);
  });

  it('cancel marks all active jobs complete and calls the cancel command', async () => {
    const { result } = renderHook(() => useIndexing(), { wrapper });
    await act(async () => { await result.current.startIndexing(1); });
    expect(result.current.activeJob).not.toBeNull();

    await act(async () => { await result.current.cancelIndexing(); });
    expect(result.current.activeJob).toBeNull();
    expect(mockCancel).toHaveBeenCalledTimes(1);
  });

  it('startIndexing throws and clears placeholder if backend invoke fails', async () => {
    mockStart.mockRejectedValueOnce(new Error('boom'));
    const { result } = renderHook(() => useIndexing(), { wrapper });
    await expect(
      act(async () => { await result.current.startIndexing(1); })
    ).rejects.toThrow('boom');
    expect(result.current.activeJob).toBeNull();
  });

  it('progressPercent computes from done/total, 0 when total is 0', async () => {
    const { result } = renderHook(() => useIndexing(), { wrapper });
    mockGetProgress.mockResolvedValueOnce(snap({ seq: 1, filesDone: 3, filesTotal: 10 }));
    await act(async () => { await result.current.startIndexing(1); });
    await act(async () => { await Promise.resolve(); });
    expect(result.current.activeJob?.progressPercent).toBe(30);

    mockGetProgress.mockResolvedValueOnce(snap({ seq: 2, filesDone: 0, filesTotal: 0 }));
    await act(async () => { vi.advanceTimersByTime(250); });
    await act(async () => { await Promise.resolve(); });
    expect(result.current.activeJob?.progressPercent).toBe(0);
  });
});
