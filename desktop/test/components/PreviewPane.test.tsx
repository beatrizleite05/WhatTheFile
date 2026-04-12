import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { PreviewPane } from '../../src/components/PreviewPane';
import type { FileResult } from '../../src/core/types';

const result: FileResult = {
  fileId: 1, rootId: 1, path: '/Users/alice/Documents/report.pdf',
  filename: 'report.pdf', mediaType: 'pdf', sizeBytes: 512000,
  indexedAt: 1700000000, confidence: 1, snippet: 'Annual performance report Q4', score: 0.9,
};

describe('PreviewPane', () => {
  it('shows placeholder when result is null', () => {
    render(<PreviewPane result={null} onOpen={vi.fn()} />);
    expect(screen.getByTestId('preview-placeholder')).toBeInTheDocument();
  });

  it('shows filename when result is provided', () => {
    render(<PreviewPane result={result} onOpen={vi.fn()} />);
    expect(screen.getByText('report.pdf')).toBeInTheDocument();
  });

  it('shows full path', () => {
    render(<PreviewPane result={result} onOpen={vi.fn()} />);
    expect(screen.getByText(result.path)).toBeInTheDocument();
  });

  it('shows snippet', () => {
    render(<PreviewPane result={result} onOpen={vi.fn()} />);
    expect(screen.getByText(/Annual performance report/)).toBeInTheDocument();
  });

  it('Open File button calls onOpen with result', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const onOpen = vi.fn();
    render(<PreviewPane result={result} onOpen={onOpen} />);
    await user.click(screen.getByRole('button', { name: /open file/i }));
    expect(onOpen).toHaveBeenCalledWith(result);
  });
});
