import { useState, useEffect, useRef, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { search } from '../api/search';
import { parseQueryDefault } from './parseQueryDefault';
import { errorMessage } from '../utils';
import type { FileResult, ParsedQuery } from '../core/types';

const DEBOUNCE_MS = 300;
const PAGE_SIZE = 50;
const MIN_QUERY_LEN = 2;

export interface UseSearchReturn {
  query: string;
  setQuery: (q: string) => void;
  mode: ParsedQuery['mode'];
  setMode: (m: ParsedQuery['mode']) => void;
  results: FileResult[];
  total: number;
  hasMore: boolean;
  loading: boolean;
  error: string | null;
  parsedRequest: ParsedQuery | null;
  loadMore: () => void;
  clearQuery: () => void;
}

function toSearchQuery(parsed: ParsedQuery, off: number) {
  return {
    queryText: parsed.queryText,
    mediaTypes: parsed.mediaTypes,
    rootScope: parsed.rootScope,
    dateFrom: parsed.dateFrom || undefined,
    dateTo: parsed.dateTo || undefined,
    minConfidence: parsed.minConfidence,
    mode: parsed.mode,
    limit: PAGE_SIZE,
    offset: off,
  };
}

export function useSearch(): UseSearchReturn {
  const [query, setQueryState] = useState('');
  const [mode, setModeState] = useState<ParsedQuery['mode']>('hybrid');
  const [results, setResults] = useState<FileResult[]>([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [parsedRequest, setParsedRequest] = useState<ParsedQuery | null>(null);

  const generationRef = useRef(0);
  const cachedParseRef = useRef<{ key: string; parsed: ParsedQuery } | null>(null);

  const parseKey = (q: string, m: ParsedQuery['mode']) => `${m}␟${q}`;

  const runSearchWithParsed = useCallback(async (parsed: ParsedQuery, off: number, append: boolean, gen: number) => {
    try {
      const response = await search(toSearchQuery(parsed, off));
      if (gen !== generationRef.current) return;
      if (append) {
        setResults((prev) => [...prev, ...response.results]);
      } else {
        setResults(response.results);
      }
      setTotal(response.total);
    } catch (e) {
      if (gen !== generationRef.current) return;
      setError(errorMessage(e));
    } finally {
      if (gen === generationRef.current) setLoading(false);
    }
  }, []);

  const runFreshSearch = useCallback(async (q: string, m: ParsedQuery['mode'], off: number, append: boolean) => {
    if (q.length < MIN_QUERY_LEN) {
      setResults([]);
      setTotal(0);
      setParsedRequest(null);
      cachedParseRef.current = null;
      return;
    }

    const gen = ++generationRef.current;
    setLoading(true);
    setError(null);

    const parsed = await parseQueryDefault(q, m);
    if (gen !== generationRef.current) return;

    cachedParseRef.current = { key: parseKey(q, m), parsed };
    setParsedRequest(parsed);
    await runSearchWithParsed(parsed, off, append, gen);
  }, [runSearchWithParsed]);

  // Debounced effect on query/mode change
  useEffect(() => {
    if (query.length < MIN_QUERY_LEN) {
      setResults([]);
      setTotal(0);
      setParsedRequest(null);
      cachedParseRef.current = null;
      return;
    }
    setOffset(0);
    const timer = setTimeout(() => {
      runFreshSearch(query, mode, 0, false);
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [query, mode, runFreshSearch]);

  // Listen for index changes and re-run current query using the cached parse
  useEffect(() => {
    let mounted = true;
    let unlisten: (() => void) | null = null;

    listen('index://changed', () => {
      if (!mounted || query.length < MIN_QUERY_LEN) return;
      const cached = cachedParseRef.current;
      if (cached && cached.key === parseKey(query, mode)) {
        const gen = ++generationRef.current;
        setLoading(true);
        setError(null);
        setOffset(0);
        runSearchWithParsed(cached.parsed, 0, false, gen);
      } else {
        runFreshSearch(query, mode, 0, false);
      }
    }).then((fn) => {
      if (!mounted) { fn(); return; }
      unlisten = fn;
    });

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, [query, mode, runFreshSearch, runSearchWithParsed]);

  const setQuery = useCallback((q: string) => {
    setQueryState(q);
  }, []);

  const setMode = useCallback((m: ParsedQuery['mode']) => {
    setModeState(m);
  }, []);

  const loadMore = useCallback(() => {
    const nextOffset = offset + PAGE_SIZE;
    setOffset(nextOffset);
    const cached = cachedParseRef.current;
    if (cached && cached.key === parseKey(query, mode)) {
      const gen = ++generationRef.current;
      setLoading(true);
      setError(null);
      runSearchWithParsed(cached.parsed, nextOffset, true, gen);
    } else {
      runFreshSearch(query, mode, nextOffset, true);
    }
  }, [query, mode, offset, runFreshSearch, runSearchWithParsed]);

  const clearQuery = useCallback(() => {
    generationRef.current++;
    cachedParseRef.current = null;
    setQueryState('');
    setResults([]);
    setTotal(0);
    setOffset(0);
    setError(null);
    setParsedRequest(null);
    setLoading(false);
  }, []);

  const hasMore = results.length < total;

  return { query, setQuery, mode, setMode, results, total, hasMore, loading, error, parsedRequest, loadMore, clearQuery };
}
