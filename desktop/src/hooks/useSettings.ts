import { useState, useEffect, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { listRoots, removeRoot as apiRemoveRoot, deleteIndex as apiDeleteIndex, type RootPayload } from '../api/settings';
import { addRoot as apiAddRoot, startIndexing } from '../api/indexing';
import { errorMessage } from '../utils';

interface SettingsState {
  roots: RootPayload[];
  loading: boolean;
  error: string | null;
  addRoot: (path: string) => Promise<void>;
  removeRoot: (id: number) => Promise<void>;
  reindex: (rootId: number) => void;
  deleteIndex: () => Promise<void>;
}

export function useSettings(): SettingsState {
  const [roots, setRoots] = useState<RootPayload[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadRoots = useCallback(async () => {
    try {
      const r = await listRoots();
      setRoots(r);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadRoots();

    let mounted = true;
    let unlisten: (() => void) | null = null;

    listen('index://changed', () => {
      if (mounted) loadRoots();
    }).then((fn) => {
      if (!mounted) { fn(); return; }
      unlisten = fn;
    });

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, [loadRoots]);

  const addRoot = useCallback(async (path: string) => {
    setError(null);
    try {
      const root = await apiAddRoot(path);
      setRoots((prev) => [...prev, root]);
      startIndexing(root.id).catch((e) => setError(errorMessage(e)));
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);

  const removeRoot = useCallback(async (id: number) => {
    setError(null);
    try {
      await apiRemoveRoot(id);
      setRoots((prev) => prev.filter((r) => r.id !== id));
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);

  const reindex = useCallback((rootId: number) => {
    startIndexing(rootId).catch((e) => setError(errorMessage(e)));
  }, []);

  const deleteIndex = useCallback(async () => {
    setError(null);
    await apiDeleteIndex();
    setRoots((prev) => prev.map((r) => ({ ...r, lastIndexedAt: null })));
  }, []);

  return { roots, loading, error, addRoot, removeRoot, reindex, deleteIndex };
}
