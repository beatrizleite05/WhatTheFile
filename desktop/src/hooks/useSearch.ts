import { useState, useEffect, useRef, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { search } from '../api/search';
import { parseQueryLlm } from '../api/queryParser';
import { parseNaturalLanguageQuery } from '../core/queryParser';
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

  const runSearch = useCallback(async (q: string, m: ParsedQuery['mode'], off: number, append: boolean) => {
    if (q.length < MIN_QUERY_LEN) {
      setResults([]);
      setTotal(0);
      setParsedRequest(null);
      return;
    }

    const gen = ++generationRef.current;
    const { parsed, needsLlmFallback } = parseNaturalLanguageQuery(q, m);
    setParsedRequest(parsed);
    setLoading(true);
    setError(null);

    try {
      // Phase 1: fire immediately with deterministic parse result
      const response = await search(toSearchQuery(parsed, off));
      if (gen !== generationRef.current) return;

      if (append) {
        setResults((prev) => [...prev, ...response.results]);
      } else {
        setResults(response.results);
      }
      setTotal(response.total);

      // Phase 2: if the deterministic pass left too much unresolved, ask the
      // LLM to re-parse and fire a second search with the refined intent.
      if (needsLlmFallback) {
        const llmParsed = await parseQueryLlm(q, m);
        if (gen !== generationRef.current) return;

        setParsedRequest(llmParsed);

        const llmResponse = await search(toSearchQuery(llmParsed, off));
        if (gen !== generationRef.current) return;

        if (append) {
          setResults((prev) => [...prev, ...llmResponse.results]);
        } else {
          setResults(llmResponse.results);
        }
        setTotal(llmResponse.total);
      }
    } catch (e) {
      if (gen !== generationRef.current) return;
      setError(errorMessage(e));
    } finally {
      if (gen === generationRef.current) {
        setLoading(false);
      }
    }
  }, []);

  // Debounced effect on query/mode change
  useEffect(() => {
    if (query.length < MIN_QUERY_LEN) {
      setResults([]);
      setTotal(0);
      setParsedRequest(null);
      return;
    }
    setOffset(0);
    const timer = setTimeout(() => {
      runSearch(query, mode, 0, false);
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [query, mode, runSearch]);

  // Listen for index changes and re-run current query
  useEffect(() => {
    let mounted = true;
    let unlisten: (() => void) | null = null;

    listen('index://changed', () => {
      if (mounted && query.length >= MIN_QUERY_LEN) {
        setOffset(0);
        runSearch(query, mode, 0, false);
      }
    }).then((fn) => {
      if (!mounted) { fn(); return; }
      unlisten = fn;
    });

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, [query, mode, runSearch]);

  const setQuery = useCallback((q: string) => {
    setQueryState(q);
  }, []);

  const setMode = useCallback((m: ParsedQuery['mode']) => {
    setModeState(m);
  }, []);

  const loadMore = useCallback(() => {
    const nextOffset = offset + PAGE_SIZE;
    setOffset(nextOffset);
    runSearch(query, mode, nextOffset, true);
  }, [query, mode, offset, runSearch]);

  const clearQuery = useCallback(() => {
    generationRef.current++;
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
