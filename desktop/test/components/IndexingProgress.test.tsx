import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { IndexingProgress } from '../../src/components/IndexingProgress';
import type { IndexingJob } from '../../src/hooks/useIndexing';

function makeJob(overrides: Partial<IndexingJob> = {}): IndexingJob {
  return {
    jobId: 1,
    rootId: 10,
    phase: 'extracting',
    filesTotal: 200,
    filesDone: 80,
    filesAdded: 30,
    filesUpdated: 5,
    filesMoved: 0,
    filesDeleted: 0,
    errorCount: 0,
    currentFile: null,
    extractionTotal: 0,
    extractionDone: 0,
    progressPercent: 40,
    isComplete: false,
    completedAt: null,
    ...overrides,
  };
}

describe('IndexingProgress', () => {
  it('renders nothing when activeJob is null', () => {
    const { container } = render(<IndexingProgress activeJob={null} />);
    expect(container.firstChild).toBeNull();
  });

  it('shows phase label', () => {
    render(<IndexingProgress activeJob={makeJob({ phase: 'discovering' })} />);
    expect(screen.getByText(/discovering/i)).toBeInTheDocument();
  });

  it('shows filesDone / filesTotal during discovering', () => {
    render(<IndexingProgress activeJob={makeJob({ phase: 'discovering', filesDone: 80, filesTotal: 200 })} />);
    expect(screen.getByText(/80/)).toBeInTheDocument();
    expect(screen.getByText(/200/)).toBeInTheDocument();
  });

  it('shows root label when provided', () => {
    render(<IndexingProgress activeJob={makeJob()} rootLabel="Documents" />);
    expect(screen.getByText(/Documents/)).toBeInTheDocument();
  });

  it('renders a progress bar element', () => {
    render(<IndexingProgress activeJob={makeJob()} />);
    expect(screen.getByRole('progressbar')).toBeInTheDocument();
  });

  it('shows extraction sub-counts during extracting phase', () => {
    const job = makeJob({ phase: 'extracting', extractionTotal: 12, extractionDone: 3 });
    render(<IndexingProgress activeJob={job} />);
    expect(screen.getByText(/3/)).toBeInTheDocument();
    expect(screen.getByText(/12/)).toBeInTheDocument();
  });
});
