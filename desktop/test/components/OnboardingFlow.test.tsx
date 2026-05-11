import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { open } from '@tauri-apps/plugin-dialog';
import { OnboardingFlow } from '../../src/components/OnboardingFlow';

const mockOpen = vi.mocked(open);

const mockSettings = {
  roots: [],
  loading: false,
  error: null,
  indexingRootIds: new Set<number>(),
  addRoot: vi.fn(),
  removeRoot: vi.fn(),
  reindex: vi.fn(),
  deleteIndex: vi.fn().mockResolvedValue(undefined),
};

beforeEach(() => {
  vi.clearAllMocks();
  mockOpen.mockResolvedValue(null);
  localStorage.clear();
});

describe('OnboardingFlow', () => {
  it('renders step 1 (folder selection) by default', () => {
    render(<OnboardingFlow onComplete={vi.fn()} settings={mockSettings} />);
    expect(screen.getByTestId('folder-picker')).toBeInTheDocument();
  });

  it('shows preset folder options', () => {
    render(<OnboardingFlow onComplete={vi.fn()} settings={mockSettings} />);
    expect(screen.getByRole('checkbox', { name: /documents/i })).toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: /desktop/i })).toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: /downloads/i })).toBeInTheDocument();
  });

  it('advances to privacy notice on Next', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    render(<OnboardingFlow onComplete={vi.fn()} settings={mockSettings} />);
    await user.click(screen.getByRole('button', { name: /next/i }));
    expect(screen.getByTestId('privacy-notice')).toBeInTheDocument();
  });

  it('calls onComplete when Get Started is clicked', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const onComplete = vi.fn();
    render(<OnboardingFlow onComplete={onComplete} settings={mockSettings} />);
    await user.click(screen.getByRole('button', { name: /next/i }));
    await user.click(screen.getByRole('button', { name: /get started/i }));
    expect(onComplete).toHaveBeenCalledWith(false);
  });

  it('calls addRoot for each selected preset when Get Started is clicked', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const addRoot = vi.fn().mockResolvedValue(undefined);
    const settings = { ...mockSettings, addRoot };
    render(<OnboardingFlow onComplete={vi.fn()} settings={settings} />);

    await user.click(screen.getByRole('checkbox', { name: /documents/i }));
    await user.click(screen.getByRole('button', { name: /next/i }));
    await user.click(screen.getByRole('button', { name: /get started/i }));

    expect(addRoot).toHaveBeenCalledWith('/Users/test/Documents');
  });

  it('adds selected custom folder on Get Started', async () => {
    mockOpen.mockResolvedValue('/Users/test/Custom');
    const user = userEvent.setup({ advanceTimers: () => {} });
    const addRoot = vi.fn().mockResolvedValue(undefined);
    const settings = { ...mockSettings, addRoot };
    render(<OnboardingFlow onComplete={vi.fn()} settings={settings} />);

    await user.click(screen.getByRole('button', { name: /add custom folder/i }));
    expect(await screen.findByTestId('custom-folder-item')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /next/i }));
    await user.click(screen.getByRole('button', { name: /get started/i }));

    expect(addRoot).toHaveBeenCalledWith('/Users/test/Custom');
  });

  it('Skip for now button calls onComplete with warning shown', async () => {
    const user = userEvent.setup({ advanceTimers: () => {} });
    const onComplete = vi.fn();
    render(<OnboardingFlow onComplete={onComplete} settings={mockSettings} />);
    await user.click(screen.getByRole('button', { name: /skip/i }));
    expect(onComplete).toHaveBeenCalledWith(true);
  });
});
