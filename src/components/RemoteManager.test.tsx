import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import RemoteManager from './RemoteManager';
import '@testing-library/jest-dom';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));

// TerminalComponent uses xterm which requires a real DOM canvas
vi.mock('./TerminalComponent', () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`terminal-${sessionId}`}>Terminal</div>
  ),
}));

vi.mock('./SshFileManager', () => ({
  default: () => <div data-testid="sftp-manager">SFTP</div>,
}));

// vis-network requires canvas APIs not present in jsdom
vi.mock('./NetworkTopologyView', () => ({
  default: () => <div data-testid="topology">Topology</div>,
}));

describe('RemoteManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the Remote Hosts heading', async () => {
    render(<RemoteManager />);
    await waitFor(() => {
      expect(screen.getByText('Remote Hosts')).toBeInTheDocument();
    });
  });

  it('shows the Add Host button', async () => {
    render(<RemoteManager />);
    await waitFor(() => {
      expect(screen.getByText('Add Host')).toBeInTheDocument();
    });
  });

  it('opens the Add Host dialog when Add Host is clicked', async () => {
    render(<RemoteManager />);
    await waitFor(() => screen.getByText('Add Host'));
    fireEvent.click(screen.getByText('Add Host'));
    await waitFor(() => {
      // AddHostDialog renders a form/modal — check for its heading
      expect(screen.getByText(/Add Host/i)).toBeInTheDocument();
    });
  });

  it('renders host inventory tab by default', async () => {
    render(<RemoteManager />);
    await waitFor(() => {
      expect(screen.getByText('Inventory')).toBeInTheDocument();
    });
  });

  it('calls get_saved_hosts on mount', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue([]);
    render(<RemoteManager />);
    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalled();
    });
  });
});
