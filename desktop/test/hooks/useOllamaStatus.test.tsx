import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useOllamaStatus } from '../../src/hooks/useOllamaStatus';

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

beforeEach(() => {
  vi.clearAllMocks();
  mockListen.mockResolvedValue(() => {});
});

describe('useOllamaStatus', () => {
  it('starts with loading true and reachable false', () => {
    mockInvoke.mockResolvedValue({ ollamaReachable: true, modelsLoaded: [] });
    const { result } = renderHook(() => useOllamaStatus());
    expect(result.current.loading).toBe(true);
    expect(result.current.reachable).toBe(false);
    expect(result.current.modelsLoaded).toEqual([]);
  });

  it('populates state after invoke resolves', async () => {
    mockInvoke.mockResolvedValue({ ollamaReachable: true, modelsLoaded: ['nomic-embed-text'] });
    const { result } = renderHook(() => useOllamaStatus());
    await act(async () => {});
    expect(result.current.loading).toBe(false);
    expect(result.current.reachable).toBe(true);
    expect(result.current.modelsLoaded).toEqual(['nomic-embed-text']);
  });

  it('sets reachable false and loading false when invoke rejects', async () => {
    mockInvoke.mockRejectedValue(new Error('connection refused'));
    const { result } = renderHook(() => useOllamaStatus());
    await act(async () => {});
    expect(result.current.loading).toBe(false);
    expect(result.current.reachable).toBe(false);
  });

  it('subscribes to runtime://status events on mount', async () => {
    mockInvoke.mockResolvedValue({ ollamaReachable: false, modelsLoaded: [] });
    renderHook(() => useOllamaStatus());
    await act(async () => {});
    expect(mockListen).toHaveBeenCalledWith('runtime://status', expect.any(Function));
  });

  it('updates state when runtime://status event fires', async () => {
    let eventHandler: ((e: { payload: unknown }) => void) | null = null;
    mockListen.mockImplementation((_event, handler) => {
      eventHandler = handler as (e: { payload: unknown }) => void;
      return Promise.resolve(() => {});
    });
    mockInvoke.mockResolvedValue({ ollamaReachable: false, modelsLoaded: [] });

    const { result } = renderHook(() => useOllamaStatus());
    await act(async () => {});

    await act(async () => {
      eventHandler?.({ payload: { ollamaReachable: true, modelsLoaded: ['qwen2.5vl:7b'] } });
    });

    expect(result.current.reachable).toBe(true);
    expect(result.current.modelsLoaded).toEqual(['qwen2.5vl:7b']);
  });

  it('calls unlisten on unmount', async () => {
    const unlisten = vi.fn();
    mockListen.mockResolvedValue(unlisten);
    mockInvoke.mockResolvedValue({ ollamaReachable: false, modelsLoaded: [] });

    const { unmount } = renderHook(() => useOllamaStatus());
    await act(async () => {});
    unmount();
    expect(unlisten).toHaveBeenCalled();
  });

  it('checkNow triggers a fresh invoke', async () => {
    mockInvoke.mockResolvedValue({ ollamaReachable: false, modelsLoaded: [] });
    const { result } = renderHook(() => useOllamaStatus());
    await act(async () => {});

    mockInvoke.mockResolvedValue({ ollamaReachable: true, modelsLoaded: ['model-a'] });
    await act(async () => {
      result.current.checkNow();
    });
    await act(async () => {});

    expect(result.current.reachable).toBe(true);
  });
});
