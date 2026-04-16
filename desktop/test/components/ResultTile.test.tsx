import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ResultTile } from '../../src/components/ResultTile';
import type { FileResult } from '../../src/core/types';

const result: FileResult = {
  fileId: 1, rootId: 1, path: '/Users/alice/Documents/invoice.pdf',
  filename: 'invoice.pdf', mediaType: 'pdf', sizeBytes: 204800,
  indexedAt: Math.floor(Date.now() / 1000) - 3600,
  confidence: 1, snippet: 'Invoice for services rendered in Q4', score: 0.95,
};

describe('ResultTile', () => {
  it('renders filename', () => {
    render(<ResultTile result={result} isSelected={false} onSelect={vi.fn()} onOpen={vi.fn()} />);
    expect(screen.getByText('invoice.pdf')).toBeInTheDocument();
  });

  it('renders the directory path', () => {
    render(<ResultTile result={result} isSelected={false} onSelect={vi.fn()} onOpen={vi.fn()} />);
    expect(screen.getByText('/Users/alice/Documents')).toBeInTheDocument();
  });

  it('renders the snippet', () => {
    render(<ResultTile result={result} isSelected={false} onSelect={vi.fn()} onOpen={vi.fn()} />);
    expect(screen.getByText(/Invoice for services/)).toBeInTheDocument();
  });

  it('calls onSelect when clicked', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const onSelect = vi.fn();
    const onOpen = vi.fn();
    render(<ResultTile result={result} isSelected={false} onSelect={onSelect} onOpen={onOpen} />);
    await user.click(screen.getByTestId('result-tile'));
    expect(onSelect).toHaveBeenCalledWith(result);
    expect(onOpen).toHaveBeenCalledWith(result);
  });

  it('calls onOpen when Enter is pressed while selected', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const onOpen = vi.fn();
    render(<ResultTile result={result} isSelected={true} onSelect={vi.fn()} onOpen={onOpen} />);
    screen.getByTestId('result-tile').focus();
    await user.keyboard('{Enter}');
    expect(onOpen).toHaveBeenCalledWith(result);
  });

  it('applies selected style when isSelected is true', () => {
    render(<ResultTile result={result} isSelected={true} onSelect={vi.fn()} onOpen={vi.fn()} />);
    const tile = screen.getByTestId('result-tile');
    expect(tile).toHaveAttribute('data-selected', 'true');
  });
});
