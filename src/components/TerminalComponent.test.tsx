import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import TerminalComponent from './TerminalComponent';
import '@testing-library/jest-dom';

// Mock Tauri invoke and event
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Mock ResizeObserver (not available in jsdom)
window.ResizeObserver = class {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
} as unknown as typeof ResizeObserver;

// Mock @xterm/xterm
vi.mock('@xterm/xterm', () => {
  return {
    Terminal: class {
      open = vi.fn();
      loadAddon = vi.fn();
      dispose = vi.fn();
      write = vi.fn();
      onData = vi.fn(() => ({ dispose: vi.fn() }));
    },
  };
});

// Mock @xterm/addon-fit
vi.mock('@xterm/addon-fit', () => {
  return {
    FitAddon: class {
      fit = vi.fn();
    },
  };
});

describe('TerminalComponent', () => {
  const defaultProps = {
    sessionId: 'test-session',
    host: 'localhost',
    username: 'user',
  };

  it('renders the terminal wrapper and container', () => {
    render(<TerminalComponent {...defaultProps} />);
    expect(screen.getByTestId('terminal-wrapper')).toBeInTheDocument();
    expect(screen.getByTestId('terminal-container')).toBeInTheDocument();
  });
});