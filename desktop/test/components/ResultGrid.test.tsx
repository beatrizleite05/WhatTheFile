import { describe, it, expect, vi, beforeAll } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ResultGrid } from '../../src/components/ResultGrid';
import type { FileResult } from '../../src/core/types';

// react-virtual needs a scroll container with real dimensions in jsdom
beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, 'getBoundingClientRect', {
    configurable: true,
    value: () => ({ width: 600, height: 480, top: 0, left: 0, bottom: 480, right: 600 }),
  });
  Object.defineProperty(HTMLElement.prototype, 'offsetHeight', { configurable: true, get: () => 480 });
  Object.defineProperty(HTMLElement.prototype, 'offsetWidth', { configurable: true, get: () => 600 });
});

const makeResult = (id: number): FileResult => ({
  fileId: id, rootId: 1, path: `/docs/file${id}.txt`, filename: `file${id}.txt`,
  mediaType: 'txt', sizeBytes: 1024, indexedAt: 1000, confidence: 1,
  snippet: `snippet ${id}`, score: 0.9,
});

const results = [makeResult(1), makeResult(2), makeResult(3)];

describe('ResultGrid', () => {
  it('renders a tile for each result', () => {
    render(
      <ResultGrid results={results} query="test" hasMore={false} loading={false}
        onOpen={vi.fn()} onLoadMore={vi.fn()} />
    );
    expect(screen.getAllByTestId('result-tile')).toHaveLength(3);
  });

  it('shows empty state when results are empty and query is non-empty', () => {
    render(
      <ResultGrid results={[]} query="something" hasMore={false} loading={false}
        onOpen={vi.fn()} onLoadMore={vi.fn()} />
    );
    expect(screen.getByTestId('empty-state')).toBeInTheDocument();
  });

  it('does not show empty state when query is empty', () => {
    render(
      <ResultGrid results={[]} query="" hasMore={false} loading={false}
        onOpen={vi.fn()} onLoadMore={vi.fn()} />
    );
    expect(screen.queryByTestId('empty-state')).not.toBeInTheDocument();
  });

  it('ArrowDown moves selection to next item', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    render(
      <ResultGrid results={results} query="test" hasMore={false} loading={false}
        onOpen={vi.fn()} onLoadMore={vi.fn()} />
    );
    const container = screen.getByTestId('result-grid');
    container.focus();
    await user.keyboard('{ArrowDown}');
    expect(screen.getAllByTestId('result-tile')[0]).toHaveAttribute('data-selected', 'true');
  });

  it('Enter on selected item calls onOpen', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const onOpen = vi.fn();
    render(
      <ResultGrid results={results} query="test" hasMore={false} loading={false}
        onOpen={onOpen} onLoadMore={vi.fn()} />
    );
    const container = screen.getByTestId('result-grid');
    container.focus();
    await user.keyboard('{ArrowDown}{Enter}');
    expect(onOpen).toHaveBeenCalledWith(results[0]);
  });
});
