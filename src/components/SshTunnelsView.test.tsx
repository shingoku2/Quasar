import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import SshTunnelsView from './SshTunnelsView';
import '@testing-library/jest-dom';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}));

describe('SshTunnelsView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the heading and form', async () => {
    render(<SshTunnelsView />);
    expect(screen.getByText(/SSH Tunnels/i)).toBeInTheDocument();
    expect(screen.getByLabelText('SSH host')).toBeInTheDocument();
    expect(screen.getByText('Start tunnel')).toBeInTheDocument();
  });

  it('shows error when SSH host is empty and start is clicked', async () => {
    render(<SshTunnelsView />);
    fireEvent.click(screen.getByText('Start tunnel'));
    await waitFor(() => {
      expect(screen.getByText('SSH host is required.')).toBeInTheDocument();
    });
  });

  it('shows error when SSH host is set but neither credential nor username is provided', async () => {
    render(<SshTunnelsView />);
    fireEvent.change(screen.getByLabelText('SSH host'), { target: { value: '10.0.0.1' } });
    fireEvent.click(screen.getByText('Start tunnel'));
    await waitFor(() => {
      expect(screen.getByText(/Either select a credential or enter SSH username/i)).toBeInTheDocument();
    });
  });

  it('shows "No active tunnels" when list is empty', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue([]);
    render(<SshTunnelsView />);
    await waitFor(() => {
      expect(screen.getByText(/No active tunnels/i)).toBeInTheDocument();
    });
  });

  it('renders active tunnels returned by backend', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'list_ssh_tunnels') {
        return Promise.resolve([
          { id: 't1', ssh_host: '10.0.0.1', ssh_port: 22, local_port: 8080, remote_host: 'localhost', remote_port: 80 },
        ]);
      }
      return Promise.resolve([]);
    });
    render(<SshTunnelsView />);
    await waitFor(() => {
      expect(screen.getByText(/10\.0\.0\.1:22/)).toBeInTheDocument();
    });
  });

  it('calls close_ssh_tunnel when trash button is clicked', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'list_ssh_tunnels') {
        return Promise.resolve([
          { id: 't1', ssh_host: '10.0.0.1', ssh_port: 22, local_port: 8080, remote_host: 'localhost', remote_port: 80 },
        ]);
      }
      return Promise.resolve([]);
    });
    render(<SshTunnelsView />);
    await waitFor(() => screen.getByLabelText('Close tunnel'));
    fireEvent.click(screen.getByLabelText('Close tunnel'));
    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('close_ssh_tunnel', { tunnelId: 't1' });
    });
  });

  it('shows error when backend call fails', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockRejectedValueOnce('Connection refused');
    render(<SshTunnelsView />);
    await waitFor(() => {
      expect(screen.getByText(/Connection refused/i)).toBeInTheDocument();
    });
  });
});
