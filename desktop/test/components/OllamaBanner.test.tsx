import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { OllamaBanner } from '../../src/components/OllamaBanner';

describe('OllamaBanner', () => {
  it('renders nothing when reachable is true', () => {
    const { container } = render(<OllamaBanner reachable={true} modelsLoaded={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders warning when reachable is false', () => {
    render(<OllamaBanner reachable={false} modelsLoaded={[]} />);
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.getByText(/ollama/i)).toBeInTheDocument();
  });

  it('shows dismiss button and hides banner when clicked', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    render(<OllamaBanner reachable={false} modelsLoaded={[]} />);

    const btn = screen.getByRole('button', { name: /dismiss/i });
    await user.click(btn);

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});
