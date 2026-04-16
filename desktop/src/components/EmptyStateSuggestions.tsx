import type { FileResult } from '../core/types';

interface EmptyStateSuggestionsProps {
  suggestions: FileResult[];
  onApplySuggestion: (query: string) => void;
}

export function EmptyStateSuggestions({ suggestions, onApplySuggestion }: EmptyStateSuggestionsProps) {
  if (suggestions.length === 0) {
    return (
      <span style={{ fontSize: 'var(--font-size-xs)' }}>
        Try broader terms, switch search mode, or remove one filter.
      </span>
    );
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 8, width: '100%' }}>
      <span style={{ fontSize: 'var(--font-size-xs)', color: 'var(--text-secondary)' }}>
        Suggestions from recent indexed files
      </span>
      <div style={{ display: 'flex', flexWrap: 'wrap', justifyContent: 'center', gap: 6 }}>
        {suggestions.slice(0, 4).map((item) => (
          <button
            key={item.fileId}
            data-testid="empty-suggestion"
            onClick={() => onApplySuggestion(item.filename)}
            style={{
              border: '1px solid var(--surface-border)',
              background: 'rgba(255,255,255,0.05)',
              color: 'var(--text-primary)',
              borderRadius: 'var(--radius-pill)',
              padding: '4px 10px',
              fontSize: 'var(--font-size-xs)',
              cursor: 'pointer',
            }}
          >
            {item.filename}
          </button>
        ))}
      </div>
      <span style={{ fontSize: 'var(--font-size-xs)' }}>
        Pivot tip: try file type terms like pdf, docx, or csv.
      </span>
    </div>
  );
}
