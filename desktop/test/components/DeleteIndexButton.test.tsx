import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import { DeleteIndexButton } from '../../src/components/DeleteIndexButton';

const mockInvoke = vi.mocked(invoke);

describe('DeleteIndexButton', () => {
  it('renders a delete button', () => {
    render(<DeleteIndexButton />);
    expect(screen.getByRole('button', { name: /delete/i })).toBeInTheDocument();
  });

  it('shows confirmation UI before calling delete_index', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    render(<DeleteIndexButton />);
    await user.click(screen.getByRole('button', { name: /delete/i }));
    expect(screen.getByRole('button', { name: /confirm/i })).toBeInTheDocument();
  });

  it('calls delete_index when confirmed', async () => {
    mockInvoke.mockResolvedValue(undefined);
    const user = userEvent.setup({ advanceTimers: () => {} });
    render(<DeleteIndexButton />);
    await user.click(screen.getByRole('button', { name: /delete/i }));
    await user.click(screen.getByRole('button', { name: /confirm/i }));
    expect(mockInvoke).toHaveBeenCalledWith('delete_index');
  });

  it('shows error when delete fails', async () => {
    mockInvoke.mockRejectedValue('disk full');
    const user = userEvent.setup({ advanceTimers: () => {} });
    render(<DeleteIndexButton />);
    await user.click(screen.getByRole('button', { name: /delete/i }));
    await user.click(screen.getByRole('button', { name: /confirm/i }));
    expect(await screen.findByText(/disk full/)).toBeInTheDocument();
  });

  it('cancel hides confirmation UI', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    render(<DeleteIndexButton />);
    await user.click(screen.getByRole('button', { name: /delete/i }));
    await user.click(screen.getByRole('button', { name: /cancel/i }));
    expect(screen.queryByRole('button', { name: /confirm/i })).not.toBeInTheDocument();
  });
});
