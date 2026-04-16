import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { EmptyStateSuggestions } from '../../src/components/EmptyStateSuggestions';
import type { FileResult } from '../../src/core/types';

const makeResult = (id: number): FileResult => ({
  fileId: id,
  rootId: 1,
  path: `/docs/file${id}.txt`,
  filename: `file${id}.txt`,
  mediaType: 'txt',
  sizeBytes: 1024,
  indexedAt: 1000,
  confidence: 1,
  snippet: `snippet ${id}`,
  score: 0.9,
});

describe('EmptyStateSuggestions', () => {
  it('renders fallback copy when suggestions are empty', () => {
    render(<EmptyStateSuggestions suggestions={[]} onApplySuggestion={vi.fn()} />);
    expect(screen.getByText(/Try broader terms/i)).toBeInTheDocument();
  });

  it('renders up to four suggestion buttons', () => {
    render(
      <EmptyStateSuggestions
        suggestions={[makeResult(1), makeResult(2), makeResult(3), makeResult(4), makeResult(5)]}
        onApplySuggestion={vi.fn()}
      />
    );
    expect(screen.getAllByTestId('empty-suggestion')).toHaveLength(4);
  });

  it('applies suggestion query when clicked', async () => {
    const onApplySuggestion = vi.fn();
    const user = userEvent.setup({ advanceTimers: () => {} });

    render(
      <EmptyStateSuggestions
        suggestions={[makeResult(7)]}
        onApplySuggestion={onApplySuggestion}
      />
    );

    await user.click(screen.getByTestId('empty-suggestion'));
    expect(onApplySuggestion).toHaveBeenCalledWith('file7.txt');
  });
});
