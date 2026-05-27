import { useRef, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { SearchBar } from './SearchBar';
import { ResultGrid, type ResultGridHandle } from './ResultGrid';
import { PreviewPane } from './PreviewPane';
import { OllamaBanner } from './OllamaBanner';
import { SkipWarningBanner } from './SkipWarningBanner';
import { IndexingProgress } from './IndexingProgress';
import { IndexingHero } from './IndexingHero';
import { Settings } from 'lucide-react';
import { openFile } from '../api/runtime';
import { removeFirstToken } from '../utils';
import type { UseSearchReturn } from '../hooks/useSearch';
import type { UseIndexingReturn } from '../hooks/useIndexing';
import type { UseOllamaStatusReturn } from '../hooks/useOllamaStatus';
import type { FileResult, ParsedQuery } from '../core/types';

interface FramelessOverlayProps {
  search: UseSearchReturn;
  indexing: UseIndexingReturn;
  ollamaStatus: UseOllamaStatusReturn;
  showSkipWarning?: boolean;
  onOpenSettings: () => void;
}

export function FramelessOverlay({ search, indexing, ollamaStatus, showSkipWarning = false, onOpenSettings }: FramelessOverlayProps) {
  const [previewResult, setPreviewResult] = useState<FileResult | null>(null);
  const [previewModalOpen, setPreviewModalOpen] = useState(false);
  const gridRef = useRef<ResultGridHandle>(null);

  function handleOpenSettings() {
    onOpenSettings();
  }

  function handleRemovePill(field: keyof ParsedQuery, value: string) {
    if (field === 'mediaTypes') {
      search.setQuery(removeFirstToken(search.query, value));
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
    if (e.key === ' ' && previewResult) {
      e.preventDefault();
      setPreviewModalOpen(true);
      return;
    }

    if (e.key === 'Escape') {
      if (previewModalOpen) {
        setPreviewModalOpen(false);
      } else if (previewResult) {
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
      {/* Drag region stops before the gear button area (32px from right) */}
      <div style={{ position: 'relative', height: 48, flexShrink: 0 }}>
        <div
          data-tauri-drag-region
          style={{
            position: 'absolute',
            top: 0,
            bottom: 0,
            left: 0,
            right: 64,
            cursor: 'default',
          }}
        />
        <button
          aria-label="Open settings"
          onClick={handleOpenSettings}
          onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') handleOpenSettings(); }}
          style={{
            position: 'absolute',
            right: 20,
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

      <OllamaBanner reachable={ollamaStatus.reachable} modelsLoaded={ollamaStatus.modelsLoaded} loading={ollamaStatus.loading} />
      <SkipWarningBanner visible={showSkipWarning} onOpenSettings={onOpenSettings} />

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
        <div
          data-testid="overlay-results-pane"
          style={{ display: 'flex', flex: previewResult ? 3 : 1, minWidth: 0 }}
        >
          {search.query.length === 0 && indexing.activeJob ? (
            <IndexingHero
              activeJob={indexing.activeJob}
              onCancel={indexing.cancelIndexing}
              cancelPending={indexing.cancelPending}
            />
          ) : (
            <ResultGrid
              ref={gridRef}
              results={search.results}
              query={search.query}
              hasMore={search.hasMore}
              loading={search.loading}
              indexingActive={indexing.activeJob !== null}
              onOpen={handleOpen}
              onSelect={handleSelect}
              onApplySuggestion={search.setQuery}
              onLoadMore={search.loadMore}
            />
          )}
        </div>

        {previewResult && (
          <div data-testid="overlay-preview-pane" style={{ display: 'flex', flex: 2, minWidth: 0 }}>
            <PreviewPane result={previewResult} onOpen={handleOpen} />
          </div>
        )}
      </div>

      {previewModalOpen && previewResult && (
        <div
          data-testid="preview-modal"
          role="dialog"
          aria-modal="true"
          style={{
            position: 'absolute',
            inset: 0,
            background: 'rgba(0,0,0,0.56)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            padding: 16,
          }}
        >
          <div style={{ width: 'min(780px, 100%)', maxHeight: '100%', display: 'flex' }}>
            <PreviewPane result={previewResult} onOpen={handleOpen} />
          </div>
        </div>
      )}

      <IndexingProgress activeJob={indexing.activeJob} />
    </div>
  );
}
