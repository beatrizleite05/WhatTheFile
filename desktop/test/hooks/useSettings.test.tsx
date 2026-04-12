import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useSettings } from '../../src/hooks/useSettings';

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

const mockRoot = { id: 1, path: '/home/user/docs', label: 'docs', active: true, createdAt: 1000, lastIndexedAt: null };

beforeEach(() => {
  vi.clearAllMocks();
  mockListen.mockResolvedValue(() => {});
});

describe('useSettings', () => {
  it('starts with empty roots and loading true', () => {
    mockInvoke.mockResolvedValue([]);
    const { result } = renderHook(() => useSettings());
    expect(result.current.roots).toEqual([]);
    expect(result.current.loading).toBe(true);
  });

  it('populates roots after mount', async () => {
    mockInvoke.mockResolvedValue([mockRoot]);
    const { result } = renderHook(() => useSettings());
    await act(async () => {});
    expect(result.current.roots).toEqual([mockRoot]);
    expect(result.current.loading).toBe(false);
  });

  it('addRoot calls add_root then start_indexing and appends root', async () => {
    const newRoot = { id: 2, path: '/home/user/pictures', label: 'pictures', active: true, createdAt: 2000, lastIndexedAt: null };
    mockInvoke
      .mockResolvedValueOnce([mockRoot])   // list_roots on mount
      .mockResolvedValueOnce(newRoot)      // add_root
      .mockResolvedValueOnce(42);          // start_indexing

    const { result } = renderHook(() => useSettings());
    await act(async () => {});

    await act(async () => {
      await result.current.addRoot('/home/user/pictures');
    });

    expect(mockInvoke).toHaveBeenCalledWith('add_root', { path: '/home/user/pictures' });
    expect(mockInvoke).toHaveBeenCalledWith('start_indexing', { rootId: 2 });
    expect(result.current.roots).toContainEqual(newRoot);
  });

  it('removeRoot calls remove_root API and removes from state', async () => {
    mockInvoke
      .mockResolvedValueOnce([mockRoot])  // list_roots on mount
      .mockResolvedValueOnce(undefined);  // remove_root

    const { result } = renderHook(() => useSettings());
    await act(async () => {});

    await act(async () => {
      await result.current.removeRoot(1);
    });

    expect(mockInvoke).toHaveBeenCalledWith('remove_root', { id: 1 });
    expect(result.current.roots).toEqual([]);
  });

  it('reindex calls start_indexing with rootId', async () => {
    mockInvoke
      .mockResolvedValueOnce([mockRoot])
      .mockResolvedValueOnce(99);

    const { result } = renderHook(() => useSettings());
    await act(async () => {});

    await act(async () => {
      result.current.reindex(1);
    });

    expect(mockInvoke).toHaveBeenCalledWith('start_indexing', { rootId: 1 });
  });

  it('surfaces error when addRoot API fails', async () => {
    mockInvoke
      .mockResolvedValueOnce([])
      .mockRejectedValueOnce('path does not exist');

    const { result } = renderHook(() => useSettings());
    await act(async () => {});

    await act(async () => {
      await result.current.addRoot('/invalid/path');
    });

    expect(result.current.error).toBe('path does not exist');
  });

  it('listens to index://changed and refreshes roots', async () => {
    let eventHandler: ((e: { payload: unknown }) => void) | null = null;
    mockListen.mockImplementation((_event, handler) => {
      eventHandler = handler as (e: { payload: unknown }) => void;
      return Promise.resolve(() => {});
    });
    const updatedRoot = { ...mockRoot, lastIndexedAt: 9999 };
    mockInvoke
      .mockResolvedValueOnce([mockRoot])    // initial list_roots
      .mockResolvedValueOnce([updatedRoot]); // refresh after event

    const { result } = renderHook(() => useSettings());
    await act(async () => {});

    await act(async () => {
      eventHandler?.({ payload: null });
    });
    await act(async () => {});

    expect(result.current.roots).toEqual([updatedRoot]);
  });
});
