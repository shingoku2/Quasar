import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import KnownHostsManager from './KnownHostsManager';
import '@testing-library/jest-dom';

const mockInvoke = vi.mocked(invoke);

const mockHosts = [
  {
    id: 1,
    host: 'prod-server.example.com',
    port: 22,
    key_type: 'ed25519',
    fingerprint: 'SHA256:abcdefg1234567',
    trust_status: 'trusted',
    first_seen: '2024-01-01T00:00:00Z',
    last_seen: '2024-06-01T00:00:00Z',
  },
  {
    id: 2,
    host: 'dev-box.local',
    port: 2222,
    key_type: 'rsa',
    fingerprint: 'SHA256:xyz9876543',
    trust_status: 'rejected',
    first_seen: '2024-02-01T00:00:00Z',
  },
  {
    id: 3,
    host: 'staging.example.com',
    port: 22,
    key_type: 'ecdsa',
    fingerprint: 'SHA256:changed123',
    trust_status: 'changed',
    first_seen: '2024-03-01T00:00:00Z',
  },
];

describe('KnownHostsManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue(mockHosts as any);
  });

  it('shows loading state initially', () => {
    mockInvoke.mockReturnValue(new Promise(() => {}));
    render(<KnownHostsManager />);
    expect(screen.getByText(/Loading known hosts/i)).toBeInTheDocument();
  });

  it('renders known hosts after loading', async () => {
    render(<KnownHostsManager />);
    await waitFor(() => {
      expect(screen.getByText(/prod-server.example.com:22/)).toBeInTheDocument();
      expect(screen.getByText(/dev-box.local:2222/)).toBeInTheDocument();
    });
  });

  it('shows empty state when no hosts exist', async () => {
    mockInvoke.mockResolvedValueOnce([]);
    render(<KnownHostsManager />);
    await waitFor(() => {
      expect(screen.getByText(/No known hosts yet/i)).toBeInTheDocument();
    });
  });

  it('shows error when loading fails', async () => {
    mockInvoke.mockRejectedValueOnce('Database error');
    render(<KnownHostsManager />);
    await waitFor(() => {
      expect(screen.getByText('Database error')).toBeInTheDocument();
    });
  });

  it('shows host count in the header', async () => {
    render(<KnownHostsManager />);
    await waitFor(() => {
      expect(screen.getByText(/3 hosts/)).toBeInTheDocument();
    });
  });

  it('filters hosts by host name search', async () => {
    render(<KnownHostsManager />);
    await waitFor(() => expect(screen.getByText(/prod-server.example.com/)).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText(/Search by host or fingerprint/i), {
      target: { value: 'dev-box' },
    });

    expect(screen.queryByText(/prod-server.example.com/)).not.toBeInTheDocument();
    expect(screen.getByText(/dev-box.local/)).toBeInTheDocument();
  });

  it('filters hosts by fingerprint search', async () => {
    render(<KnownHostsManager />);
    await waitFor(() => expect(screen.getByText(/prod-server.example.com/)).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText(/Search by host or fingerprint/i), {
      target: { value: 'xyz9876543' },
    });

    expect(screen.queryByText(/prod-server.example.com/)).not.toBeInTheDocument();
    expect(screen.getByText(/dev-box.local/)).toBeInTheDocument();
  });

  it('shows no results message when search matches nothing', async () => {
    render(<KnownHostsManager />);
    await waitFor(() => expect(screen.getByText(/prod-server.example.com/)).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText(/Search by host or fingerprint/i), {
      target: { value: 'nonexistent-host' },
    });

    expect(screen.getByText(/No matching hosts found/i)).toBeInTheDocument();
  });

  it('calls remove_ssh_host_key after confirmation', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    mockInvoke
      .mockResolvedValueOnce(mockHosts as any)
      .mockResolvedValueOnce(undefined) // remove
      .mockResolvedValueOnce([mockHosts[1], mockHosts[2]] as any); // reload

    render(<KnownHostsManager />);
    await waitFor(() => expect(screen.getByText(/prod-server.example.com/)).toBeInTheDocument());

    const removeButtons = screen.getAllByTitle('Remove host key');
    fireEvent.click(removeButtons[0]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('remove_ssh_host_key', {
        host: 'prod-server.example.com',
        port: 22,
      });
    });
    confirmSpy.mockRestore();
  });

  it('does not remove host when confirmation is cancelled', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false);
    render(<KnownHostsManager />);
    await waitFor(() => expect(screen.getByText(/prod-server.example.com/)).toBeInTheDocument());

    const removeButtons = screen.getAllByTitle('Remove host key');
    fireEvent.click(removeButtons[0]);

    expect(mockInvoke).not.toHaveBeenCalledWith('remove_ssh_host_key', expect.anything());
    confirmSpy.mockRestore();
  });

  it('shows Mark as Trusted button for non-trusted hosts', async () => {
    render(<KnownHostsManager />);
    await waitFor(() => expect(screen.getByText(/dev-box.local/)).toBeInTheDocument());

    expect(screen.getAllByRole('button', { name: /Mark as Trusted/i }).length).toBeGreaterThan(0);
  });

  it('does not show Mark as Trusted button for trusted hosts', async () => {
    // prod-server has trust_status 'trusted', so its actions row should be hidden
    render(<KnownHostsManager />);
    await waitFor(() => expect(screen.getByText(/prod-server.example.com/)).toBeInTheDocument());

    // The number of Mark as Trusted buttons should be less than total hosts
    const trustBtns = screen.queryAllByRole('button', { name: /Mark as Trusted/i });
    // 2 hosts (rejected, changed) should have it; 1 (trusted) should not
    expect(trustBtns.length).toBe(2);
  });

  it('calls update_ssh_host_trust when Mark as Trusted is clicked', async () => {
    mockInvoke
      .mockResolvedValueOnce(mockHosts as any)
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce(mockHosts as any);

    render(<KnownHostsManager />);
    await waitFor(() => expect(screen.getAllByRole('button', { name: /Mark as Trusted/i }).length).toBeGreaterThan(0));

    fireEvent.click(screen.getAllByRole('button', { name: /Mark as Trusted/i })[0]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('update_ssh_host_trust', expect.objectContaining({
        trustStatus: 'trusted',
      }));
    });
  });
});
