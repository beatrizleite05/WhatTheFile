import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryPills } from '../../src/components/QueryPills';
import type { SearchRequest } from '../../src/core/types';

const baseRequest = (): SearchRequest => ({
  queryText: 'invoices',
  mediaTypes: [],
  rootScope: [],
  dateFrom: '',
  dateTo: '',
  minConfidence: 0,
  mode: 'hybrid',
  parserConfidence: 0.9,
});

describe('QueryPills', () => {
  it('renders nothing when parsedRequest is null', () => {
    const { container } = render(<QueryPills parsedRequest={null} onRemove={vi.fn()} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders a pill for each mediaType', () => {
    const req = { ...baseRequest(), mediaTypes: ['pdf', 'docx'] };
    render(<QueryPills parsedRequest={req} onRemove={vi.fn()} />);
    expect(screen.getByText('pdf')).toBeInTheDocument();
    expect(screen.getByText('docx')).toBeInTheDocument();
  });

  it('renders a date pill when dateFrom is set', () => {
    const req = { ...baseRequest(), dateFrom: '2024' };
    render(<QueryPills parsedRequest={req} onRemove={vi.fn()} />);
    expect(screen.getByText(/2024/)).toBeInTheDocument();
  });

  it('renders a scope pill for each rootScope entry', () => {
    const req = { ...baseRequest(), rootScope: ['Documents'] };
    render(<QueryPills parsedRequest={req} onRemove={vi.fn()} />);
    expect(screen.getByText(/Documents/)).toBeInTheDocument();
  });

  it('renders AI-interpreted badge when parserConfidence < 0.5', () => {
    const req = { ...baseRequest(), parserConfidence: 0.2 };
    render(<QueryPills parsedRequest={req} onRemove={vi.fn()} />);
    expect(screen.getByText(/AI/i)).toBeInTheDocument();
  });

  it('does not render AI badge when parserConfidence >= 0.5', () => {
    const req = { ...baseRequest(), mediaTypes: ['pdf'], parserConfidence: 0.9 };
    render(<QueryPills parsedRequest={req} onRemove={vi.fn()} />);
    expect(screen.queryByText(/AI/i)).not.toBeInTheDocument();
  });

  it('calls onRemove with field and value when × is clicked on a mediaType pill', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const onRemove = vi.fn();
    const req = { ...baseRequest(), mediaTypes: ['pdf'] };
    render(<QueryPills parsedRequest={req} onRemove={onRemove} />);

    const removeBtn = screen.getByRole('button', { name: /remove pdf/i });
    await user.click(removeBtn);

    expect(onRemove).toHaveBeenCalledWith('mediaTypes', 'pdf');
  });
});
