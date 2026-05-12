import { formatRelativeTime } from '../utils';
import type { IndexingJob } from '../hooks/useIndexing';

interface ActivityLogProps {
  jobs: IndexingJob[];
}

export function ActivityLog({ jobs }: ActivityLogProps) {
  const completed = jobs
    .filter((j) => j.isComplete && j.completedAt !== null)
    .sort((a, b) => (b.completedAt ?? 0) - (a.completedAt ?? 0));

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <h2 style={{ fontSize: 'var(--font-size-md)', fontWeight: 700, color: 'var(--text-primary)', margin: 0 }}>
        Activity Log
      </h2>

      {completed.length === 0 && (
        <p style={{ color: 'var(--text-tertiary)', fontSize: 'var(--font-size-sm)', margin: 0 }}>
          No completed index jobs yet.
        </p>
      )}

      {completed.map((job) => (
        <div
          key={job.jobId}
          style={{
            padding: '8px 12px',
            background: 'rgba(255,255,255,0.03)',
            borderRadius: 'var(--radius-element)',
            border: '1px solid var(--divider)',
            display: 'flex',
            flexDirection: 'column',
            gap: 2,
          }}
        >
          <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 'var(--font-size-xs)', color: 'var(--text-secondary)' }}>
            <span>Root #{job.rootId}</span>
            <span style={{ fontFamily: 'var(--font-mono)', color: 'var(--text-tertiary)' }}>
              {job.completedAt !== null ? formatRelativeTime(job.completedAt) : '—'}
            </span>
          </div>
          <div style={{ fontSize: 'var(--font-size-xs)', color: 'var(--text-tertiary)', fontFamily: 'var(--font-mono)' }}>
            +{job.filesAdded} added · {job.filesUpdated} updated · {job.filesDeleted} deleted
            {job.errorCount > 0 ? ` · ${job.errorCount} errors` : ''}
          </div>
        </div>
      ))}
    </div>
  );
}
