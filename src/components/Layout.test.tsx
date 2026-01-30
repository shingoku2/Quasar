import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import Layout from './Layout';
import '@testing-library/jest-dom';

// Mock Tauri invoke and event
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn((cmd) => {
    if (cmd === 'connect_ssh' || cmd === 'connect_rdp' || cmd === 'start_discovery') {
      return Promise.resolve();
    }
    if (cmd === 'check_ai_status') return Promise.resolve(false); // Default to offline for layout tests
    return Promise.resolve();
  }),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Mock SQL plugin
vi.mock('@tauri-apps/plugin-sql', () => ({
  default: {
    load: vi.fn().mockResolvedValue({
      execute: vi.fn().mockResolvedValue({ rowsAffected: 0 }),
      select: vi.fn().mockResolvedValue([]),
    }),
  },
}));

describe('Layout Component', () => {
  it('renders sidebar and top bar', async () => {
    render(<Layout />);
    expect(screen.getByText('TITAN NEXUS')).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/Search resources/)).toBeInTheDocument();
  });

  it('can switch views', async () => {
    render(<Layout />);
    
    const remoteButton = screen.getByTitle('Remote');
    fireEvent.click(remoteButton);

    await waitFor(() => {
      expect(screen.getByText('Remote Hosts')).toBeInTheDocument();
    });
  });
});