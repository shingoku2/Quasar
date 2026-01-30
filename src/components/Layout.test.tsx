import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import Layout from './Layout';
import '@testing-library/jest-dom';

// Mock Tauri invoke
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn((cmd) => {
    if (cmd === 'connect_ssh' || cmd === 'connect_rdp') {
      return Promise.resolve();
    }
    return Promise.resolve();
  }),
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
  it('renders sidebar and host inventory', async () => {
    render(<Layout />);
    expect(screen.getByText('TITAN')).toBeInTheDocument();
    expect(screen.getAllByText('Dashboard').length).toBeGreaterThan(0);
    
    await waitFor(() => {
      expect(screen.getByText('Host Inventory')).toBeInTheDocument();
    });
  });

  it('can add and switch between tabs', async () => {
    render(<Layout />);
    
    await waitFor(() => expect(screen.getByText('Host Inventory')).toBeInTheDocument());

    const addButton = screen.getByTitle('New Session');
    fireEvent.click(addButton);

    expect(screen.getByText('New Session 1')).toBeInTheDocument();
  });

  it('can remove tabs', async () => {
    render(<Layout />);
    
    await waitFor(() => expect(screen.getByText('Host Inventory')).toBeInTheDocument());

    const addButton = screen.getByTitle('New Session');
    fireEvent.click(addButton);

    const tabItem = screen.getByText('New Session 1').parentElement;
    const closeBtn = tabItem?.querySelector('button');
    if (closeBtn) {
      fireEvent.click(closeBtn);
    }

    expect(screen.queryByText('New Session 1')).not.toBeInTheDocument();
  });
});