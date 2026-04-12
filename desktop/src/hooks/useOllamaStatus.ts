import { useState, useEffect, useCallback, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getRuntimeStatus } from '../api/runtime';

export interface UseOllamaStatusReturn {
  reachable: boolean;
  modelsLoaded: string[];
  loading: boolean;
  checkNow: () => void;
}

interface OllamaState {
  reachable: boolean;
  modelsLoaded: string[];
  loading: boolean;
}

interface RuntimeStatusPayload {
  ollamaReachable: boolean;
  modelsLoaded: string[];
}

export function useOllamaStatus(): UseOllamaStatusReturn {
  const [state, setState] = useState<OllamaState>({
    reachable: false,
    modelsLoaded: [],
    loading: true,
  });
  const unlistenRef = useRef<(() => void) | null>(null);

  const fetch = useCallback(async () => {
    try {
      const status = await getRuntimeStatus();
      setState({ reachable: status.ollamaReachable, modelsLoaded: status.modelsLoaded, loading: false });
    } catch {
      setState({ reachable: false, modelsLoaded: [], loading: false });
    }
  }, []);

  useEffect(() => {
    fetch();

    let mounted = true;
    listen<RuntimeStatusPayload>('runtime://status', (event) => {
      if (!mounted) return;
      setState({
        reachable: event.payload.ollamaReachable,
        modelsLoaded: event.payload.modelsLoaded,
        loading: false,
      });
    }).then((unlisten) => {
      if (!mounted) {
        unlisten();
        return;
      }
      unlistenRef.current = unlisten;
    });

    const interval = setInterval(fetch, 30_000);

    return () => {
      mounted = false;
      unlistenRef.current?.();
      clearInterval(interval);
    };
  }, [fetch]);

  return { ...state, checkNow: fetch };
}
