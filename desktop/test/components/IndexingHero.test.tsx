import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { IndexingHero } from '../../src/components/IndexingHero';
import type { IndexingJob } from '../../src/hooks/useIndexing';

vi.mock('../../src/api/indexing', () => ({
  cancelIndexing: vi.fn(),
}));

function makeJob(overrides: Partial<IndexingJob> = {}): IndexingJob {
  return {
    jobId: 1,
    rootId: 10,
    phase: 'extracting',
    filesTotal: 8,
    filesDone: 8,
    filesAdded: 8,
    filesUpdated: 0,
    filesMoved: 0,
    filesDeleted: 0,
    errorCount: 0,
    currentFile: null,
    extractionTotal: 0,
    extractionDone: 0,
    progressPercent: 0,
    isComplete: false,
    completedAt: null,
    ...overrides,
  };
}

describe('IndexingHero', () => {
  it('renders nothing when activeJob is null', () => {
    const { container } = render(<IndexingHero activeJob={null} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders the basename of currentFile when present', () => {
    const job = makeJob({ currentFile: 'reports/2026/q1-summary.pdf' });
    render(<IndexingHero activeJob={job} />);
    expect(screen.getByTestId('indexing-hero-current-file')).toHaveTextContent('q1-summary.pdf');
  });

  it('omits the current-file element when currentFile is null', () => {
    render(<IndexingHero activeJob={makeJob({ currentFile: null })} />);
    expect(screen.queryByTestId('indexing-hero-current-file')).toBeNull();
  });

  it('shows extraction sub-counts during extracting phase', () => {
    const job = makeJob({ phase: 'extracting', extractionTotal: 12, extractionDone: 5 });
    render(<IndexingHero activeJob={job} />);
    expect(screen.getByText(/5\s*\/\s*12\s*files/)).toBeInTheDocument();
  });

  it('shows discovery counts when not in extracting phase', () => {
    const job = makeJob({ phase: 'discovering', filesDone: 3, filesTotal: 11, extractionTotal: 0 });
    render(<IndexingHero activeJob={job} />);
    expect(screen.getByText(/3\s*\/\s*11\s*files/)).toBeInTheDocument();
  });
});
