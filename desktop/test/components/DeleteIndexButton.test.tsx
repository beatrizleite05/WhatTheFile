import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DeleteIndexButton } from '../../src/components/DeleteIndexButton';

describe('DeleteIndexButton', () => {
  it('renders a delete button', () => {
    render(<DeleteIndexButton onDeleted={vi.fn().mockResolvedValue(undefined)} />);
    expect(screen.getByRole('button', { name: /delete/i })).toBeInTheDocument();
  });

  it('shows confirmation UI before calling onDeleted', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    render(<DeleteIndexButton onDeleted={vi.fn().mockResolvedValue(undefined)} />);
    await user.click(screen.getByRole('button', { name: /delete/i }));
    expect(screen.getByRole('button', { name: /confirm/i })).toBeInTheDocument();
  });

  it('calls onDeleted when confirmed', async () => {
    const onDeleted = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup({ advanceTimers: () => {} });
    render(<DeleteIndexButton onDeleted={onDeleted} />);
    await user.click(screen.getByRole('button', { name: /delete/i }));
    await user.click(screen.getByRole('button', { name: /confirm/i }));
    expect(onDeleted).toHaveBeenCalledOnce();
  });

  it('shows error when delete fails', async () => {
    const onDeleted = vi.fn().mockRejectedValue('disk full');
    const user = userEvent.setup({ advanceTimers: () => {} });
    render(<DeleteIndexButton onDeleted={onDeleted} />);
    await user.click(screen.getByRole('button', { name: /delete/i }));
    await user.click(screen.getByRole('button', { name: /confirm/i }));
    expect(await screen.findByText(/disk full/)).toBeInTheDocument();
  });

  it('cancel hides confirmation UI', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    render(<DeleteIndexButton onDeleted={vi.fn().mockResolvedValue(undefined)} />);
    await user.click(screen.getByRole('button', { name: /delete/i }));
    await user.click(screen.getByRole('button', { name: /cancel/i }));
    expect(screen.queryByRole('button', { name: /confirm/i })).not.toBeInTheDocument();
  });
});
