import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { SearchBar } from '../../src/components/SearchBar';

const defaultProps = {
  query: '',
  onChange: vi.fn(),
  onClear: vi.fn(),
  onFocusResults: vi.fn(),
  loading: false,
  parsedRequest: null,
  mode: 'hybrid' as const,
  onModeChange: vi.fn(),
  onRemovePill: vi.fn(),
};

describe('SearchBar', () => {
  it('renders input with placeholder', () => {
    render(<SearchBar {...defaultProps} />);
    expect(screen.getByPlaceholderText(/search/i)).toBeInTheDocument();
  });

  it('calls onChange when user types', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const onChange = vi.fn();
    render(<SearchBar {...defaultProps} onChange={onChange} />);
    await user.type(screen.getByRole('searchbox'), 'hello');
    expect(onChange).toHaveBeenCalled();
  });

  it('calls onClear when Escape is pressed and query is non-empty', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const onClear = vi.fn();
    render(<SearchBar {...defaultProps} query="something" onClear={onClear} />);
    await user.keyboard('{Escape}');
    expect(onClear).toHaveBeenCalled();
  });

  it('calls onFocusResults when ArrowDown is pressed', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const onFocusResults = vi.fn();
    render(<SearchBar {...defaultProps} onFocusResults={onFocusResults} />);
    await user.keyboard('{ArrowDown}');
    expect(onFocusResults).toHaveBeenCalled();
  });

  it('shows spinner icon when loading', () => {
    render(<SearchBar {...defaultProps} loading={true} />);
    expect(screen.getByTestId('search-loading')).toBeInTheDocument();
  });

  it('shows search icon when not loading', () => {
    render(<SearchBar {...defaultProps} loading={false} />);
    expect(screen.getByTestId('search-icon')).toBeInTheDocument();
  });

  it('renders mode toggle buttons', () => {
    render(<SearchBar {...defaultProps} />);
    expect(screen.getByRole('button', { name: /hybrid/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /keyword/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /semantic/i })).toBeInTheDocument();
  });

  it('calls onModeChange when mode button is clicked', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const onModeChange = vi.fn();
    render(<SearchBar {...defaultProps} onModeChange={onModeChange} />);
    await user.click(screen.getByRole('button', { name: /keyword/i }));
    expect(onModeChange).toHaveBeenCalledWith('keyword');
  });
});
