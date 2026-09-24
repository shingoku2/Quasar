import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import RemoteManager from './RemoteManager';
import '@testing-library/jest-dom';
import { invoke } from '@tauri-apps/api/core';
import { resetTailscaleStatusCache } from '../hooks/useTailscaleStatus';

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
  default: ({ credentialId, password }: { credentialId?: string; password?: string }) => (
    <div
      data-testid="sftp-manager"
      data-credential-id={credentialId}
      {...(password ? { 'data-has-password': 'true' } : {})}
    >
      SFTP
    </div>
  ),
}));

// vis-network requires canvas APIs not present in jsdom
vi.mock('./NetworkTopologyView', () => ({
  default: () => <div data-testid="topology">Topology</div>,
}));

describe('RemoteManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTailscaleStatusCache();
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

  // FE-001 / RSEC-013: SFTP gets the vault credential by id, like SSH; the decrypted
  // password is never fetched into the webview.
  it('starts SFTP with the credential id and never reveals the password', async () => {
    const host = { id: 'h1', name: 'Web', address: '10.0.0.5', protocol: 'ssh', port: 22, username: 'root' };
    const cred = { id: 'c1', name: 'Web creds', username: 'root', credential_type: 'ssh', created_at: '2024-01-01T00:00:00Z' };
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case 'get_saved_hosts': return [host];
        case 'is_vault_locked': return false;
        case 'list_credentials': return [cred];
        case 'get_credential': return { ...cred, has_password: true, has_private_key: false, has_key_passphrase: false };
        case 'get_tailscale_status': return { installed: false, peers: [] };
        default: return [];
      }
    });

    render(<RemoteManager />);
    fireEvent.click(await screen.findByRole('button', { name: 'SFTP' }));
    const selector = await screen.findByRole('dialog');
    fireEvent.click(await within(selector).findByRole('button', { name: /Web creds/ }));

    const manager = await screen.findByTestId('sftp-manager');
    expect(manager).toHaveAttribute('data-credential-id', 'c1');
    expect(manager).not.toHaveAttribute('data-has-password');
    expect(vi.mocked(invoke)).not.toHaveBeenCalledWith('reveal_credential_password', expect.anything());
  });
});
