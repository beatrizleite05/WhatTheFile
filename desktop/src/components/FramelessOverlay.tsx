import { useRef, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { SearchBar } from './SearchBar';
import { ResultGrid, type ResultGridHandle } from './ResultGrid';
import { PreviewPane } from './PreviewPane';
import { OllamaBanner } from './OllamaBanner';
import { IndexingProgress } from './IndexingProgress';
import { Settings } from 'lucide-react';
import { openFile, openSettings } from '../api/runtime';
import type { UseSearchReturn } from '../hooks/useSearch';
import type { UseIndexingReturn } from '../hooks/useIndexing';
import type { UseOllamaStatusReturn } from '../hooks/useOllamaStatus';
import type { FileResult, SearchRequest } from '../core/types';

interface FramelessOverlayProps {
  search: UseSearchReturn;
  indexing: UseIndexingReturn;
  ollamaStatus: UseOllamaStatusReturn;
}

export function FramelessOverlay({ search, indexing, ollamaStatus }: FramelessOverlayProps) {
  const [previewResult, setPreviewResult] = useState<FileResult | null>(null);
  const gridRef = useRef<ResultGridHandle>(null);

  function handleRemovePill(field: keyof SearchRequest, value: string) {
    if (field === 'mediaTypes') {
      const remaining = search.query.replace(value, '').trim();
      search.setQuery(remaining);
    } else {
      search.clearQuery();
    }
  }

  function handleOpen(result: FileResult) {
    openFile(result.path).catch(() => {});
  }

  function handleSelect(result: FileResult) {
    setPreviewResult(result);
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'Escape') {
      if (previewResult) {
        setPreviewResult(null);
      } else if (search.query.length > 0) {
        search.clearQuery();
      } else {
        getCurrentWindow().hide();
      }
    }
  }

  return (
    <div
      className="glass"
      onKeyDown={handleKeyDown}
      style={{
        width: '100%',
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
      }}
    >
      {/* Drag region — gear button sits outside the drag surface */}
      <div style={{ position: 'relative', height: 28, flexShrink: 0 }}>
        <div data-tauri-drag-region style={{ position: 'absolute', inset: 0, cursor: 'default' }} />
        <button
          aria-label="Open settings"
          onClick={() => openSettings().catch(() => {})}
          onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') openSettings().catch(() => {}); }}
          style={{
            position: 'absolute',
            right: 10,
            top: '50%',
            transform: 'translateY(-50%)',
            background: 'none',
            border: 'none',
            cursor: 'pointer',
            padding: 4,
            color: 'var(--text-tertiary)',
            display: 'flex',
            alignItems: 'center',
          }}
        >
          <Settings size={14} />
        </button>
      </div>

      <OllamaBanner reachable={ollamaStatus.reachable} modelsLoaded={ollamaStatus.modelsLoaded} />

      <div style={{ padding: '0 0 8px', flexShrink: 0 }}>
        <SearchBar
          query={search.query}
          onChange={search.setQuery}
          onClear={search.clearQuery}
          onFocusResults={() => gridRef.current?.focus()}
          loading={search.loading}
          parsedRequest={search.parsedRequest}
          mode={search.mode}
          onModeChange={search.setMode}
          onRemovePill={handleRemovePill}
        />
      </div>

      <div style={{ borderTop: '1px solid var(--divider)', flexShrink: 0 }} />

      <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
        <ResultGrid
          ref={gridRef}
          results={search.results}
          query={search.query}
          hasMore={search.hasMore}
          loading={search.loading}
          onOpen={handleOpen}
          onSelect={handleSelect}
          onLoadMore={search.loadMore}
        />

        {previewResult && (
          <PreviewPane result={previewResult} onOpen={handleOpen} />
        )}
      </div>

      <IndexingProgress activeJob={indexing.activeJob} />
    </div>
  );
}
