import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ActivityLog } from '../../src/components/ActivityLog';
import type { IndexingJob } from '../../src/hooks/useIndexing';

const baseJob: IndexingJob = {
  jobId: 1,
  rootId: 10,
  phase: 'completed',
  filesTotal: 100,
  filesDone: 100,
  filesAdded: 20,
  filesUpdated: 5,
  filesMoved: 0,
  filesDeleted: 2,
  errorCount: 0,
  currentFile: null,
  extractionTotal: 0,
  extractionDone: 0,
  progressPercent: 100,
  isComplete: true,
  completedAt: 1700000000,
};

describe('ActivityLog', () => {
  it('shows empty state when there are no completed jobs', () => {
    render(<ActivityLog jobs={[]} />);
    expect(screen.getByText(/No completed index jobs yet/i)).toBeInTheDocument();
  });

  it('renders completed job metrics', () => {
    render(<ActivityLog jobs={[baseJob]} />);
    expect(screen.getByText(/Root #10/)).toBeInTheDocument();
    expect(screen.getByText(/\+20 added/i)).toBeInTheDocument();
    expect(screen.getByText(/5 updated/i)).toBeInTheDocument();
    expect(screen.getByText(/2 deleted/i)).toBeInTheDocument();
  });

  it('renders error count when job has errors', () => {
    render(<ActivityLog jobs={[{ ...baseJob, jobId: 2, errorCount: 3 }]} />);
    expect(screen.getByText(/3 errors/i)).toBeInTheDocument();
  });
});
