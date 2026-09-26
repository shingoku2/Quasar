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

  // PR #68 review: tunnels take every SSH-type credential, including legacy `password` ones.
  it('offers legacy password credentials for tunnels', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'list_credentials') {
        return Promise.resolve([
          { id: 'p', name: 'Legacy pw', username: 'u', credential_type: 'password' },
          { id: 'a', name: 'API cred', username: 'u', credential_type: 'api' },
        ]);
      }
      return Promise.resolve([]);
    });
    render(<SshTunnelsView />);
    await waitFor(() => expect(screen.getByRole('option', { name: /Legacy pw/ })).toBeInTheDocument());
    expect(screen.queryByRole('option', { name: /API cred/ })).not.toBeInTheDocument();
  });
});

describe('SshTunnelsView starting and closing tunnels', () => {
  type Handler = () => unknown;

  async function mockCommands(handlers: Record<string, Handler>) {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      const h = handlers[cmd];
      if (!h) return Promise.resolve([]);
      try {
        return Promise.resolve(h());
      } catch (e) {
        return Promise.reject(e);
      }
    });
    return vi.mocked(invoke);
  }

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('starts a tunnel with lowerCamelCase args and the selected credential', async () => {
    const invoke = await mockCommands({
      list_credentials: () => [{ id: 'c1', name: 'Box key', username: 'root', credential_type: 'ssh_key' }],
    });
    render(<SshTunnelsView />);
    await waitFor(() => expect(screen.getByRole('option', { name: 'Box key' })).toBeInTheDocument());

    fireEvent.change(screen.getByLabelText('SSH host'), { target: { value: 'jump.example' } });
    fireEvent.change(screen.getByLabelText('SSH port (1-65535)'), { target: { value: '2222' } });
    fireEvent.change(screen.getByLabelText('Credential (optional)'), { target: { value: 'c1' } });
    fireEvent.change(screen.getByLabelText('Local port (1-65535)'), { target: { value: '5432' } });
    fireEvent.change(screen.getByLabelText('Remote host'), { target: { value: 'db.internal' } });
    fireEvent.change(screen.getByLabelText('Remote port (1-65535)'), { target: { value: '5433' } });
    fireEvent.click(screen.getByText('Start tunnel'));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('start_ssh_tunnel', expect.anything()));
    const call = invoke.mock.calls.find((c) => c[0] === 'start_ssh_tunnel');
    const args = call?.[1] as Record<string, unknown>;
    expect(args).toMatchObject({
      sshHost: 'jump.example',
      sshPort: 2222,
      sshUser: '',
      credentialId: 'c1',
      localPort: 5432,
      remoteHost: 'db.internal',
      remotePort: 5433,
    });
    expect(String(args.tunnelId)).toMatch(/^tunnel-\d+$/);
    expect(args.password).toBeUndefined();
    expect(Object.keys(args).some((k) => k.includes('_'))).toBe(false);

    // The list is refreshed and the credential choice is reset afterwards.
    await waitFor(() => expect((screen.getByLabelText('Credential (optional)') as HTMLSelectElement).value).toBe(''));
    expect(invoke.mock.calls.filter((c) => c[0] === 'list_ssh_tunnels').length).toBeGreaterThanOrEqual(2);
  });

  it('a username alone is enough and sends no credential id', async () => {
    const invoke = await mockCommands({});
    render(<SshTunnelsView />);
    fireEvent.change(screen.getByLabelText('SSH host'), { target: { value: 'h' } });
    fireEvent.change(screen.getByLabelText('SSH username'), { target: { value: 'alice' } });
    fireEvent.click(screen.getByText('Start tunnel'));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('start_ssh_tunnel', expect.objectContaining({ sshUser: 'alice', credentialId: undefined })));
  });

  it('a whitespace-only host is rejected without calling the backend', async () => {
    const invoke = await mockCommands({});
    render(<SshTunnelsView />);
    fireEvent.change(screen.getByLabelText('SSH host'), { target: { value: '   ' } });
    fireEvent.click(screen.getByText('Start tunnel'));
    expect(await screen.findByRole('alert')).toHaveTextContent('SSH host is required.');
    expect(invoke).not.toHaveBeenCalledWith('start_ssh_tunnel', expect.anything());
  });

  it.each([
    ['SSH port (1-65535)', '70000', 65535],
    ['Local port (1-65535)', '0', 1],
    ['Remote port (1-65535)', '-5', 1],
  ])('%s clamps %s to %i', async (label, input, expected) => {
    await mockCommands({});
    render(<SshTunnelsView />);
    const field = screen.getByLabelText(label) as HTMLInputElement;
    fireEvent.change(field, { target: { value: input } });
    expect(field.value).toBe(String(expected));
  });

  it('a non-numeric port keeps the previous value', async () => {
    await mockCommands({});
    render(<SshTunnelsView />);
    const field = screen.getByLabelText('Local port (1-65535)') as HTMLInputElement;
    fireEvent.change(field, { target: { value: '' } });
    expect(field.value).toBe('1080');
  });

  it('shows the backend error when starting fails and re-enables the button', async () => {
    await mockCommands({
      start_ssh_tunnel: () => {
        throw new Error('Port 1080 already in use');
      },
    });
    render(<SshTunnelsView />);
    fireEvent.change(screen.getByLabelText('SSH host'), { target: { value: 'h' } });
    fireEvent.change(screen.getByLabelText('SSH username'), { target: { value: 'u' } });
    fireEvent.click(screen.getByText('Start tunnel'));
    expect(await screen.findByRole('alert')).toHaveTextContent('Port 1080 already in use');
    expect(screen.getByText('Start tunnel').closest('button')).not.toBeDisabled();
  });

  it('shows the backend error when closing a tunnel fails', async () => {
    await mockCommands({
      list_ssh_tunnels: () => [{ id: 't1', ssh_host: 'h', ssh_port: 22, local_port: 1080, remote_host: 'r', remote_port: 80 }],
      close_ssh_tunnel: () => {
        throw new Error('no such tunnel');
      },
    });
    render(<SshTunnelsView />);
    fireEvent.click(await screen.findByLabelText('Close tunnel'));
    expect(await screen.findByRole('alert')).toHaveTextContent('no such tunnel');
  });

  it('shows an error when credentials cannot be listed', async () => {
    await mockCommands({
      list_credentials: () => {
        throw new Error('Vault is locked');
      },
    });
    render(<SshTunnelsView />);
    expect(await screen.findByRole('alert')).toHaveTextContent('Vault is locked');
  });

  it('treats a non-array tunnel list as empty', async () => {
    await mockCommands({ list_ssh_tunnels: () => null });
    render(<SshTunnelsView />);
    expect(await screen.findByText(/No active tunnels/)).toBeInTheDocument();
  });
});
