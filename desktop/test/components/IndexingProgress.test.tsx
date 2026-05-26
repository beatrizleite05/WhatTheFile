import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { IndexingProgress } from '../../src/components/IndexingProgress';
import type { IndexingJob } from '../../src/hooks/useIndexing';

const job: IndexingJob = {
  jobId: 1, rootId: 10, phase: 'extracting',
  filesTotal: 200, filesDone: 80, filesAdded: 30, filesUpdated: 5,
  filesMoved: 0, filesDeleted: 0, errorCount: 0, progressPercent: 40, isComplete: false, completedAt: null,
  currentFile: null, extractionTotal: 0, extractionDone: 0,
};

describe('IndexingProgress', () => {
  it('renders nothing when activeJob is null', () => {
    const { container } = render(<IndexingProgress activeJob={null} />);
    expect(container.firstChild).toBeNull();
  });

  it('shows phase label', () => {
    render(<IndexingProgress activeJob={job} />);
    expect(screen.getByText(/extracting/i)).toBeInTheDocument();
  });

  it('shows filesDone / filesTotal count', () => {
    render(<IndexingProgress activeJob={job} />);
    expect(screen.getByText(/80/)).toBeInTheDocument();
    expect(screen.getByText(/200/)).toBeInTheDocument();
  });

  it('shows root label when provided', () => {
    render(<IndexingProgress activeJob={job} rootLabel="Documents" />);
    expect(screen.getByText(/Documents/)).toBeInTheDocument();
  });

  it('renders a progress bar element', () => {
    render(<IndexingProgress activeJob={job} />);
    expect(screen.getByRole('progressbar')).toBeInTheDocument();
  });
});
