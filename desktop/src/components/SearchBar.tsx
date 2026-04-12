import { useRef } from 'react';
import { Search, Loader2 } from 'lucide-react';
import { QueryPills } from './QueryPills';
import type { SearchRequest } from '../core/types';

interface SearchBarProps {
  query: string;
  onChange: (value: string) => void;
  onClear: () => void;
  onFocusResults: () => void;
  loading: boolean;
  parsedRequest: SearchRequest | null;
  mode: SearchRequest['mode'];
  onModeChange: (mode: SearchRequest['mode']) => void;
  onRemovePill: (field: keyof SearchRequest, value: string) => void;
}

const MODES: SearchRequest['mode'][] = ['hybrid', 'keyword', 'semantic'];

export function SearchBar({
  query, onChange, onClear, onFocusResults, loading,
  parsedRequest, mode, onModeChange, onRemovePill,
}: SearchBarProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '0 16px' }}>
        <span style={{ color: 'var(--text-tertiary)', display: 'flex', flexShrink: 0 }}>
          {loading
            ? <Loader2 size={16} className="spin" data-testid="search-loading" />
            : <Search size={16} data-testid="search-icon" />
          }
        </span>

        <input
          ref={inputRef}
          role="searchbox"
          type="text"
          autoFocus
          value={query}
          placeholder="Search your files…"
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Escape') { e.preventDefault(); onClear(); }
            if (e.key === 'ArrowDown') { e.preventDefault(); onFocusResults(); }
          }}
          style={{
            flex: 1,
            background: 'none',
            border: 'none',
            outline: 'none',
            color: 'var(--text-primary)',
            fontSize: 'var(--font-size-input)',
            fontWeight: 600,
            fontFamily: 'var(--font-system)',
          }}
        />

        <div style={{ display: 'flex', gap: 4 }}>
          {MODES.map((m) => (
            <button
              key={m}
              aria-label={m}
              onClick={() => onModeChange(m)}
              style={{
                background: mode === m ? 'var(--selection-bg)' : 'none',
                border: '1px solid',
                borderColor: mode === m ? 'var(--selection-border)' : 'transparent',
                borderRadius: 'var(--radius-pill)',
                color: mode === m ? 'var(--accent)' : 'var(--text-secondary)',
                fontSize: 'var(--font-size-xs)',
                fontFamily: 'var(--font-system)',
                padding: '2px 10px',
                cursor: 'pointer',
              }}
            >
              {m}
            </button>
          ))}
        </div>
      </div>

      {parsedRequest && (
        <div style={{ paddingLeft: 42, paddingRight: 16 }}>
          <QueryPills parsedRequest={parsedRequest} onRemove={onRemovePill} />
        </div>
      )}
    </div>
  );
}
