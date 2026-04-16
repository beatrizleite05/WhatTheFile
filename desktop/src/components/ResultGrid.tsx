import { useState, useRef, useCallback, useImperativeHandle, forwardRef, useEffect } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { ResultTile } from './ResultTile';
import { EmptyStateSuggestions } from './EmptyStateSuggestions';
import { getRecentFiles } from '../api/files';
import type { FileResult } from '../core/types';

interface ResultGridProps {
  results: FileResult[];
  query: string;
  hasMore: boolean;
  loading: boolean;
  onOpen: (result: FileResult) => void;
  onSelect?: (result: FileResult) => void;
  onApplySuggestion?: (query: string) => void;
  onLoadMore: () => void;
}

export interface ResultGridHandle {
  focus: () => void;
}

export const ResultGrid = forwardRef<ResultGridHandle, ResultGridProps>(
  function ResultGrid({ results, query, hasMore, loading, onOpen, onSelect, onApplySuggestion, onLoadMore }, ref) {
    const [selectedIndex, setSelectedIndex] = useState(-1);
    const [suggestions, setSuggestions] = useState<FileResult[]>([]);
    const containerRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
      let mounted = true;

      if (results.length === 0 && query.length > 0) {
        getRecentFiles(4)
          .then((items) => {
            if (mounted) setSuggestions(Array.isArray(items) ? items : []);
          })
          .catch(() => {
            if (mounted) setSuggestions([]);
          });
      }

      return () => {
        mounted = false;
      };
    }, [results.length, query]);

    const virtualizer = useVirtualizer({
      count: results.length,
      getScrollElement: () => containerRef.current,
      estimateSize: () => 80,
      overscan: 5,
    });

    useImperativeHandle(ref, () => ({
      focus: () => {
        setSelectedIndex(0);
        containerRef.current?.focus();
      },
    }));

    const handleScroll = useCallback(() => {
      const items = virtualizer.getVirtualItems();
      if (items.length === 0) return;
      const lastVisible = items[items.length - 1].index;
      if (hasMore && !loading && lastVisible >= results.length - 3) {
        onLoadMore();
      }
    }, [virtualizer, hasMore, loading, results.length, onLoadMore]);

    function selectIndex(i: number) {
      setSelectedIndex(i);
      if (i >= 0 && results[i]) onSelect?.(results[i]);
    }

    const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
      if (results.length === 0) return;
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        selectIndex(Math.min(selectedIndex + 1, results.length - 1));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        selectIndex(selectedIndex <= 0 ? -1 : selectedIndex - 1);
      } else if (e.key === 'Enter' && selectedIndex >= 0) {
        e.preventDefault();
        onOpen(results[selectedIndex]);
      }
    }, [results, selectedIndex, onOpen, onSelect]);

    if (results.length === 0 && query.length > 0) {
      return (
        <div
          data-testid="empty-state"
          style={{
            flex: 1,
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            gap: 8,
            color: 'var(--text-tertiary)',
            fontSize: 'var(--font-size-sm)',
            padding: 24,
          }}
        >
          <span>No results for "{query}"</span>
          <EmptyStateSuggestions
            suggestions={suggestions}
            onApplySuggestion={(suggestionQuery) => {
              onApplySuggestion?.(suggestionQuery);
            }}
          />
        </div>
      );
    }

    const virtualItems = virtualizer.getVirtualItems();

    return (
      <div
        ref={containerRef}
        data-testid="result-grid"
        tabIndex={0}
        onKeyDown={handleKeyDown}
        onScroll={handleScroll}
        style={{
          flex: 1,
          overflowY: 'auto',
          minHeight: 0,
          outline: 'none',
          position: 'relative',
        }}
      >
        <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
          {virtualItems.map((vItem) => {
            const result = results[vItem.index];
            return (
              <div
                key={result.fileId}
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  transform: `translateY(${vItem.start}px)`,
                }}
              >
                <ResultTile
                  result={result}
                  isSelected={vItem.index === selectedIndex}
                  onSelect={() => selectIndex(vItem.index)}
                  onOpen={onOpen}
                />
              </div>
            );
          })}
        </div>
        {loading && (
          <div style={{ padding: '8px 16px', color: 'var(--text-tertiary)', fontSize: 'var(--font-size-xs)' }}>
            Loading…
          </div>
        )}
      </div>
    );
  }
);
