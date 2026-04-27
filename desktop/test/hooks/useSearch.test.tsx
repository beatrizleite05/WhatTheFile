import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useSearch } from '../../src/hooks/useSearch';

vi.mock('../../src/api/queryParser', () => ({
  parseQueryLlm: vi.fn(),
}));
import { parseQueryLlm } from '../../src/api/queryParser';
const mockParseQueryLlm = vi.mocked(parseQueryLlm);

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

const makeResult = (id: number) => ({
  fileId: id, rootId: 1, path: `/docs/file${id}.txt`, filename: `file${id}.txt`,
  mediaType: 'txt', sizeBytes: 1024, indexedAt: 1000, confidence: 1, snippet: 'some text', score: 0.9,
});

const makeResponse = (results = [makeResult(1)], total = 1) => ({
  results, total, limit: 50, offset: 0, nextCursor: null,
});

/** Helper: set query, flush effects, advance debounce, flush promises */
async function triggerSearch(setQuery: (q: string) => void, query: string) {
  act(() => { setQuery(query); });
  await act(async () => { vi.advanceTimersByTime(300); });
  await act(async () => {});
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.useFakeTimers();
  mockListen.mockResolvedValue(() => {});
});

afterEach(() => {
  vi.useRealTimers();
});

describe('useSearch', () => {
  it('starts with empty state', () => {
    const { result } = renderHook(() => useSearch());
    expect(result.current.query).toBe('');
    expect(result.current.results).toEqual([]);
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
    expect(result.current.parsedRequest).toBeNull();
  });

  it('does not invoke for queries shorter than 2 chars', async () => {
    const { result } = renderHook(() => useSearch());
    await triggerSearch(result.current.setQuery, 'a');
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it('debounces: invoke fires after 300ms, not before', async () => {
    mockInvoke.mockResolvedValue(makeResponse());
    const { result } = renderHook(() => useSearch());

    act(() => { result.current.setQuery('invoices pdf'); });
    expect(mockInvoke).not.toHaveBeenCalled();

    await act(async () => { vi.advanceTimersByTime(300); });
    expect(mockInvoke).toHaveBeenCalledWith('search', expect.any(Object));
  });

  it('populates results after invoke resolves', async () => {
    mockInvoke.mockResolvedValue(makeResponse([makeResult(1), makeResult(2)], 2));
    const { result } = renderHook(() => useSearch());

    await triggerSearch(result.current.setQuery, 'invoices');

    expect(result.current.results).toHaveLength(2);
    expect(result.current.total).toBe(2);
  });

  it('parsedRequest is populated from queryParser', async () => {
    mockInvoke.mockResolvedValue(makeResponse());
    const { result } = renderHook(() => useSearch());

    await triggerSearch(result.current.setQuery, 'invoices pdf');

    expect(result.current.parsedRequest).not.toBeNull();
    expect(result.current.parsedRequest?.mediaTypes).toContain('pdf');
  });

  it('discards stale responses via generation counter', async () => {
    let resolveFirst!: (v: unknown) => void;
    let resolveSecond!: (v: unknown) => void;
    mockInvoke
      .mockReturnValueOnce(new Promise((r) => { resolveFirst = r; }))
      .mockReturnValueOnce(new Promise((r) => { resolveSecond = r; }));

    const { result } = renderHook(() => useSearch());

    act(() => { result.current.setQuery('contracts'); });
    await act(async () => { vi.advanceTimersByTime(300); });

    act(() => { result.current.setQuery('contracts pdf'); });
    await act(async () => { vi.advanceTimersByTime(300); });

    // Resolve second first (latest)
    await act(async () => { resolveSecond(makeResponse([makeResult(99)], 1)); });
    // Resolve first (stale)
    await act(async () => { resolveFirst(makeResponse([makeResult(1)], 1)); });

    // Only the second (non-stale) result should land
    expect(result.current.results[0]?.fileId).toBe(99);
    expect(result.current.results).toHaveLength(1);
  });

  it('loadMore appends results and increments offset', async () => {
    const page1 = makeResponse([makeResult(1), makeResult(2)], 4);
    const page2 = { ...makeResponse([makeResult(3), makeResult(4)], 4), offset: 50 };
    mockInvoke.mockResolvedValueOnce(page1).mockResolvedValueOnce(page2);

    const { result } = renderHook(() => useSearch());
    await triggerSearch(result.current.setQuery, 'reports');

    await act(async () => { result.current.loadMore(); });
    await act(async () => {});

    expect(result.current.results).toHaveLength(4);
    expect(result.current.results[2].fileId).toBe(3);
  });

  it('hasMore is false when all results are loaded', async () => {
    mockInvoke.mockResolvedValue(makeResponse([makeResult(1)], 1));
    const { result } = renderHook(() => useSearch());
    await triggerSearch(result.current.setQuery, 'notes');
    expect(result.current.hasMore).toBe(false);
  });

  it('hasMore is true when more results exist', async () => {
    mockInvoke.mockResolvedValue(makeResponse([makeResult(1)], 50));
    const { result } = renderHook(() => useSearch());
    await triggerSearch(result.current.setQuery, 'notes');
    expect(result.current.hasMore).toBe(true);
  });

  it('clearQuery resets all state', async () => {
    mockInvoke.mockResolvedValue(makeResponse());
    const { result } = renderHook(() => useSearch());
    await triggerSearch(result.current.setQuery, 'documents');

    act(() => { result.current.clearQuery(); });

    expect(result.current.query).toBe('');
    expect(result.current.results).toEqual([]);
    expect(result.current.parsedRequest).toBeNull();
  });

  it('loading is true during fetch and false after', async () => {
    let resolve!: (v: unknown) => void;
    mockInvoke.mockReturnValue(new Promise((r) => { resolve = r; }));

    const { result } = renderHook(() => useSearch());
    act(() => { result.current.setQuery('contracts'); });
    await act(async () => { vi.advanceTimersByTime(300); });

    expect(result.current.loading).toBe(true);

    await act(async () => { resolve(makeResponse()); });
    expect(result.current.loading).toBe(false);
  });

  it('surfaces error when invoke rejects', async () => {
    mockInvoke.mockRejectedValue('search failed');
    const { result } = renderHook(() => useSearch());
    await triggerSearch(result.current.setQuery, 'something');

    expect(result.current.error).toBe('search failed');
    expect(result.current.loading).toBe(false);
  });

  it('re-searches when index://changed event fires', async () => {
    let eventHandler: ((e: { payload: unknown }) => void) | null = null;
    mockListen.mockImplementation((_event, handler) => {
      eventHandler = handler as (e: { payload: unknown }) => void;
      return Promise.resolve(() => {});
    });
    mockInvoke.mockResolvedValue(makeResponse([makeResult(1)], 1));

    const { result } = renderHook(() => useSearch());
    await triggerSearch(result.current.setQuery, 'reports');

    mockInvoke.mockResolvedValue(makeResponse([makeResult(2)], 1));
    await act(async () => { eventHandler?.({ payload: null }); });
    await act(async () => {});

    expect(result.current.results[0]?.fileId).toBe(2);
  });

  it('mode change triggers re-search with new mode', async () => {
    mockInvoke.mockResolvedValue(makeResponse());
    const { result } = renderHook(() => useSearch());
    await triggerSearch(result.current.setQuery, 'reports');

    act(() => { result.current.setMode('keyword'); });
    await act(async () => { vi.advanceTimersByTime(300); });
    await act(async () => {});

    const calls = mockInvoke.mock.calls;
    const lastCall = calls[calls.length - 1];
    expect((lastCall[1] as { query: { mode: string } }).query.mode).toBe('keyword');
  });

  describe('LLM fallback (two-phase search)', () => {
    // A query that exceeds both thresholds: >5 words, >3 unresolved tokens, non-keyword mode
    const AMBIGUOUS_QUERY = 'find the document about the project planning meeting notes';

    it('fires a second search with LLM-refined ParsedQuery when deterministic parse is insufficient', async () => {
      const llmParsed = {
        queryText: 'project planning meeting notes',
        mediaTypes: [],
        rootScope: [],
        dateFrom: '',
        dateTo: '',
        minConfidence: 0,
        mode: 'hybrid' as const,
      };
      mockParseQueryLlm.mockResolvedValue(llmParsed);
      // Phase 1 response, then phase 2 response
      mockInvoke
        .mockResolvedValueOnce(makeResponse([makeResult(1)], 1))
        .mockResolvedValueOnce(makeResponse([makeResult(2)], 1));

      const { result } = renderHook(() => useSearch());
      await triggerSearch(result.current.setQuery, AMBIGUOUS_QUERY);

      expect(mockParseQueryLlm).toHaveBeenCalledWith(AMBIGUOUS_QUERY, 'hybrid');
      // Second invoke call should use the LLM-refined queryText
      const secondCall = mockInvoke.mock.calls[1];
      expect((secondCall[1] as { query: { queryText: string } }).query.queryText)
        .toBe('project planning meeting notes');
      // Final results come from the LLM-refined search
      expect(result.current.results[0]?.fileId).toBe(2);
    });

    it('does not call LLM for short or well-parsed queries', async () => {
      mockInvoke.mockResolvedValue(makeResponse());
      const { result } = renderHook(() => useSearch());

      await triggerSearch(result.current.setQuery, 'invoices pdf 2024');

      expect(mockParseQueryLlm).not.toHaveBeenCalled();
    });

    it('does not call LLM in keyword mode even for long ambiguous queries', async () => {
      mockInvoke.mockResolvedValue(makeResponse());
      const { result } = renderHook(() => useSearch());

      act(() => { result.current.setMode('keyword'); });
      await triggerSearch(result.current.setQuery, AMBIGUOUS_QUERY);

      expect(mockParseQueryLlm).not.toHaveBeenCalled();
    });

    it('keeps phase-1 results if LLM call fails', async () => {
      mockParseQueryLlm.mockRejectedValue(new Error('ollama timeout'));
      mockInvoke.mockResolvedValue(makeResponse([makeResult(1)], 1));

      const { result } = renderHook(() => useSearch());
      await triggerSearch(result.current.setQuery, AMBIGUOUS_QUERY);

      // Error from LLM path should surface
      expect(result.current.error).toBeTruthy();
    });
  });
});
