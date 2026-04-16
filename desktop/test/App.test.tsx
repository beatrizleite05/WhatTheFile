import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import App from '../src/App';

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  mockListen.mockResolvedValue(() => {});
  // Default: no roots, ollama unreachable
  mockInvoke.mockResolvedValue([]);

  Object.defineProperty(HTMLElement.prototype, 'getBoundingClientRect', {
    configurable: true,
    value: () => ({ width: 600, height: 480, top: 0, left: 0, bottom: 480, right: 600 }),
  });
  Object.defineProperty(HTMLElement.prototype, 'offsetHeight', { configurable: true, get: () => 480 });
});

describe('App', () => {
  it('shows OnboardingFlow when not yet onboarded', async () => {
    render(<App />);
    // Wait for hooks to settle
    await new Promise((r) => setTimeout(r, 0));
    expect(screen.getByTestId('folder-picker')).toBeInTheDocument();
  });

  it('shows FramelessOverlay when already onboarded', async () => {
    localStorage.setItem('wtf:onboarded', 'true');
    mockInvoke.mockResolvedValue({ ollamaReachable: false, modelsLoaded: [] });
    render(<App />);
    await new Promise((r) => setTimeout(r, 0));
    expect(screen.getByRole('searchbox')).toBeInTheDocument();
  });

  it('does not show both onboarding and overlay simultaneously', async () => {
    render(<App />);
    await new Promise((r) => setTimeout(r, 0));
    const onboarding = screen.queryByTestId('folder-picker');
    const overlay = screen.queryByRole('searchbox');
    expect(onboarding === null || overlay === null).toBe(true);
  });

  it('shows persistent skip warning when onboarded via skip and no roots exist', async () => {
    localStorage.setItem('wtf:onboarded', 'true');
    localStorage.setItem('wtf:skippedOnboarding', 'true');

    mockInvoke.mockImplementation(async (command) => {
      if (command === 'list_roots') return [];
      if (command === 'get_runtime_status') return { ollamaReachable: false, modelsLoaded: [] };
      return [];
    });

    render(<App />);
    await new Promise((r) => setTimeout(r, 0));
    expect(screen.getByTestId('skip-warning-banner')).toBeInTheDocument();
  });

  it('hides skip warning when at least one root is configured', async () => {
    localStorage.setItem('wtf:onboarded', 'true');
    localStorage.setItem('wtf:skippedOnboarding', 'true');

    mockInvoke.mockImplementation(async (command) => {
      if (command === 'list_roots') {
        return [{ id: 1, path: '/Users/test/Documents', label: 'Documents', active: true, createdAt: 1, lastIndexedAt: null }];
      }
      if (command === 'get_runtime_status') return { ollamaReachable: false, modelsLoaded: [] };
      return [];
    });

    render(<App />);
    await new Promise((r) => setTimeout(r, 0));
    expect(screen.queryByTestId('skip-warning-banner')).not.toBeInTheDocument();
  });
});
