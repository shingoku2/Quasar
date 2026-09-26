import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import RemoteManager from './RemoteManager';
import type { Host } from './HostList';
import { resetTailscaleStatusCache } from '../hooks/useTailscaleStatus';
import { SFTP_CREDENTIAL_TYPES, SSH_CREDENTIAL_TYPES } from '../lib/utils';
import '@testing-library/jest-dom';

// Children are stubs that expose the callbacks RemoteManager passes them, so these
// tests drive RemoteManager's own connect / credential / tab logic.
let hostListProps: { onConnect: (h: Host) => void; onSftp: (h: Host) => void; onAddHost: (v: unknown) => void } | null = null;
vi.mock('./HostList', () => ({
  default: (props: NonNullable<typeof hostListProps>) => {
    hostListProps = props;
    return <div data-testid="host-list" />;
  },
}));

interface TerminalProps { sessionId: string; host: string; port: number; username: string; password?: string; credentialId?: string }
const terminals: TerminalProps[] = [];
vi.mock('./TerminalComponent', () => ({
  default: (props: TerminalProps) => {
    if (!terminals.some((t) => t.sessionId === props.sessionId)) terminals.push(props);
    return <div data-testid="terminal">{`${props.username}@${props.host}:${props.port}`}</div>;
  },
}));

interface SftpProps { host: string; port: number; username: string; password?: string; credentialId?: string }
const sftpSessions: SftpProps[] = [];
vi.mock('./SshFileManager', () => ({
  default: (props: SftpProps) => {
    sftpSessions.push(props);
    return <div data-testid="sftp">{`${props.username}@${props.host}`}</div>;
  },
}));

vi.mock('./SshTunnelsView', () => ({ default: () => <div data-testid="tunnels" /> }));

interface Cred { id: string; name: string; username: string; credential_type: string }
let selectorProps: { hostAddress: string; allowedTypes: readonly string[]; onSelect: (c: Cred) => void; onCancel: () => void; onManualEntry: () => void } | null = null;
vi.mock('./vault/CredentialSelector', () => ({
  default: (props: NonNullable<typeof selectorProps>) => {
    selectorProps = props;
    return <div data-testid="selector">{props.hostAddress}</div>;
  },
}));

type SubmitFn = (u: string, p: string, o?: { saveCredential: boolean; credentialName?: string }) => Promise<void> | void;
let promptProps: { hostName: string; initialUsername?: string; allowSaveCredential?: boolean; allowNoPassword?: boolean; onSubmit: SubmitFn; onCancel: () => void } | null = null;
vi.mock('./CredentialPrompt', () => ({
  default: (props: NonNullable<typeof promptProps>) => {
    promptProps = props;
    return <div data-testid="prompt">{props.hostName}</div>;
  },
}));

const sshHost: Host = { id: 'h1', name: 'web', address: '10.0.0.2', protocol: 'ssh', port: 2222, username: 'admin' };

function mockVault(locked: boolean | Error, extra: Record<string, () => unknown> = {}) {
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === 'is_vault_locked') return locked instanceof Error ? Promise.reject(locked) : Promise.resolve(locked);
    const h = extra[cmd];
    if (h) {
      try {
        return Promise.resolve(h());
      } catch (e) {
        return Promise.reject(e);
      }
    }
    return Promise.resolve(null);
  });
}

describe('RemoteManager connect flows', () => {
  let alertSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.clearAllMocks();
    resetTailscaleStatusCache();
    hostListProps = null;
    selectorProps = null;
    promptProps = null;
    terminals.length = 0;
    sftpSessions.length = 0;
    sessionStorage.clear();
    alertSpy = vi.spyOn(window, 'alert').mockImplementation(() => {});
  });

  afterEach(() => {
    alertSpy.mockRestore();
    // Tests that silence console.error spy on it; put the real one back.
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it('an unlocked vault offers SSH credentials for the host', async () => {
    mockVault(false);
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onConnect(sshHost); });
    expect(screen.getByTestId('selector')).toHaveTextContent('10.0.0.2');
    expect(selectorProps?.allowedTypes).toEqual(SSH_CREDENTIAL_TYPES);
  });

  it('SFTP offers only SFTP credential types', async () => {
    mockVault(false);
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onSftp(sshHost); });
    expect(selectorProps?.allowedTypes).toEqual(SFTP_CREDENTIAL_TYPES);
  });

  it('choosing a credential opens a terminal tab that gets the credential id, not a password', async () => {
    mockVault(false);
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onConnect(sshHost); });
    await act(async () => { selectorProps?.onSelect({ id: 'c1', name: 'k', username: 'deploy', credential_type: 'ssh_key' }); });
    expect(await screen.findByText('SSH: web')).toBeInTheDocument();
    await waitFor(() => expect(terminals).toHaveLength(1));
    expect(terminals[0]).toMatchObject({ host: '10.0.0.2', port: 2222, username: 'deploy', credentialId: 'c1', password: undefined });
    expect(screen.queryByTestId('selector')).not.toBeInTheDocument();
  });

  it("a credential without a username falls back to the host's username", async () => {
    mockVault(false);
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onSftp(sshHost); });
    await act(async () => { selectorProps?.onSelect({ id: 'c2', name: 'pw', username: '', credential_type: 'ssh' }); });
    expect(screen.getByText('SFTP: web')).toBeInTheDocument();
    expect(sftpSessions[0]).toMatchObject({ username: 'admin', credentialId: 'c2', password: undefined });
  });

  it('with no username anywhere, choosing a credential switches to manual entry', async () => {
    mockVault(false);
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onConnect({ ...sshHost, username: undefined }); });
    await act(async () => { selectorProps?.onSelect({ id: 'c3', name: 'x', username: '', credential_type: 'ssh' }); });
    expect(screen.queryByTestId('selector')).not.toBeInTheDocument();
    expect(screen.getByTestId('prompt')).toHaveTextContent('web');
    expect(terminals).toHaveLength(0);
  });

  it('cancelling the selector opens nothing', async () => {
    mockVault(false);
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onConnect(sshHost); });
    act(() => { selectorProps?.onCancel(); });
    expect(screen.queryByTestId('selector')).not.toBeInTheDocument();
    expect(screen.queryByTestId('prompt')).not.toBeInTheDocument();
  });

  it('manual entry from the selector allows saving the credential', async () => {
    mockVault(false);
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onConnect(sshHost); });
    act(() => { selectorProps?.onManualEntry(); });
    expect(promptProps?.allowSaveCredential).toBe(true);
    expect(promptProps?.initialUsername).toBe('admin');
  });

  it.each([
    ['locked', true],
    ['unreadable', new Error('ipc')],
  ])('a %s vault goes straight to manual entry without offering to save', async (_label, locked) => {
    mockVault(locked);
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onConnect(sshHost); });
    expect(screen.queryByTestId('selector')).not.toBeInTheDocument();
    expect(promptProps?.allowSaveCredential).toBe(false);
    expect(promptProps?.allowNoPassword).toBe(false);
  });

  it('manual SSH entry opens a terminal with the typed password', async () => {
    mockVault(true);
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onConnect(sshHost); });
    await act(async () => { await promptProps?.onSubmit('root', 's3cret'); });
    await waitFor(() => expect(terminals).toHaveLength(1));
    expect(terminals[0]).toMatchObject({ username: 'root', password: 's3cret', credentialId: undefined });
    expect(screen.queryByTestId('prompt')).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith('add_credential', expect.anything());
  });

  it('manual SFTP entry opens a file manager tab', async () => {
    mockVault(true);
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onSftp(sshHost); });
    await act(async () => { await promptProps?.onSubmit('root', 'pw'); });
    expect(screen.getByText('SFTP: web')).toBeInTheDocument();
    expect(sftpSessions[0]).toMatchObject({ username: 'root', password: 'pw', port: 2222 });
  });

  it('saving a manually entered credential stores it bound to the host', async () => {
    mockVault(false, { add_credential: () => 'new-id' });
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onConnect({ ...sshHost, port: undefined }); });
    act(() => { selectorProps?.onManualEntry(); });
    await act(async () => { await promptProps?.onSubmit('root', 'pw', { saveCredential: true }); });
    expect(invoke).toHaveBeenCalledWith('add_credential', {
      name: 'web (root)',
      username: 'root',
      password: 'pw',
      credentialType: 'ssh',
      host: '10.0.0.2',
      port: 22,
      metadata: null,
    });
    expect(alertSpy).not.toHaveBeenCalled();
  });

  it('a custom credential name is used, and a failed save still leaves the session open', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    mockVault(false, {
      add_credential: () => {
        throw new Error('dup');
      },
    });
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onConnect(sshHost); });
    act(() => { selectorProps?.onManualEntry(); });
    await act(async () => { await promptProps?.onSubmit('root', 'pw', { saveCredential: true, credentialName: 'Mine' }); });
    expect(invoke).toHaveBeenCalledWith('add_credential', expect.objectContaining({ name: 'Mine' }));
    expect(alertSpy).toHaveBeenCalledWith(expect.stringContaining('Connected, but failed to save credential'));
    expect(screen.getByText('SSH: web')).toBeInTheDocument();
  });

  it('an empty username from manual entry is refused', async () => {
    mockVault(true);
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onConnect({ ...sshHost, username: undefined }); });
    await act(async () => { await promptProps?.onSubmit('', 'pw'); });
    expect(alertSpy).toHaveBeenCalledWith('Username is required to start an SSH session.');
    expect(terminals).toHaveLength(0);
  });

  it('an RDP host launches the external client and opens a status tab', async () => {
    mockVault(false);
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onConnect({ id: 'r', name: 'desk', address: '10.0.0.3', protocol: 'rdp' }); });
    expect(invoke).toHaveBeenCalledWith('connect_rdp', { address: '10.0.0.3' });
    expect(screen.getByText('RDP: desk')).toBeInTheDocument();
    expect(screen.getByText('Launched RDP client for 10.0.0.3')).toBeInTheDocument();
  });

  it('a failed RDP launch is reported and opens no tab', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    mockVault(false, {
      connect_rdp: () => {
        throw new Error('no client');
      },
    });
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onConnect({ id: 'r', name: 'desk', address: '10.0.0.3', protocol: 'rdp' }); });
    expect(alertSpy).toHaveBeenCalledWith(expect.stringContaining('Failed to launch session'));
    expect(screen.queryByText('RDP: desk')).not.toBeInTheDocument();
  });

  it('inventory-only protocols explain that there is no client', async () => {
    mockVault(false);
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onConnect({ id: 'd', name: 'pg', address: 'db', protocol: 'database' }); });
    expect(alertSpy).toHaveBeenCalledWith(expect.stringContaining('No built-in client for "database" hosts'));
    expect(invoke).not.toHaveBeenCalledWith('is_vault_locked');
  });

  it('closing the active session tab falls back to the last remaining tab', async () => {
    mockVault(false);
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onConnect(sshHost); });
    await act(async () => { selectorProps?.onSelect({ id: 'c1', name: 'k', username: 'u', credential_type: 'ssh' }); });
    fireEvent.click(screen.getByLabelText('Close SSH: web'));
    expect(screen.queryByText('SSH: web')).not.toBeInTheDocument();
    // Tunnels is the last tab left; it becomes active and visible.
    expect(screen.getByTestId('tunnels').parentElement).toHaveClass('block');
  });

  it('split view pairs the active tab with the toggled one and collapses below two', async () => {
    mockVault(false);
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onConnect(sshHost); });
    await act(async () => { selectorProps?.onSelect({ id: 'c1', name: 'k', username: 'u', credential_type: 'ssh' }); });
    await act(async () => { hostListProps?.onSftp(sshHost); });
    await act(async () => { selectorProps?.onSelect({ id: 'c2', name: 'p', username: 'u', credential_type: 'ssh' }); });

    const [sshToggle] = screen.getAllByLabelText('Toggle Split View');
    // SFTP is active; toggling SSH shows both side by side.
    fireEvent.click(sshToggle);
    expect(screen.getByTestId('terminal').parentElement).not.toHaveClass('hidden');
    expect(screen.getByTestId('sftp').parentElement).not.toHaveClass('hidden');
    expect(screen.getByTestId('host-list').closest('.hidden')).not.toBeNull();

    // Removing one of two leaves no split.
    fireEvent.click(screen.getAllByLabelText('Toggle Split View')[0]);
    expect(screen.getByTestId('terminal').parentElement).toHaveClass('hidden');
    expect(screen.getByTestId('sftp').parentElement).not.toHaveClass('hidden');
  });

  it('a new session opened during split view joins the split', async () => {
    mockVault(false);
    render(<RemoteManager />);
    await act(async () => { hostListProps?.onConnect(sshHost); });
    await act(async () => { selectorProps?.onSelect({ id: 'c1', name: 'k', username: 'u', credential_type: 'ssh' }); });
    fireEvent.click(screen.getAllByLabelText('Toggle Split View')[0]);
    await act(async () => { hostListProps?.onSftp(sshHost); });
    await act(async () => { selectorProps?.onSelect({ id: 'c2', name: 'p', username: 'u', credential_type: 'ssh' }); });
    expect(screen.getByTestId('terminal').parentElement).not.toHaveClass('hidden');
    expect(screen.getByTestId('sftp').parentElement).not.toHaveClass('hidden');
  });

  it('a quick-connect host stored before mount is connected once and cleared', async () => {
    vi.useFakeTimers();
    mockVault(false);
    sessionStorage.setItem('quickConnectHost', JSON.stringify({ id: 'q1', name: 'quick', address: '10.9.9.9', protocol: 'ssh', port: null, username: null }));
    render(<RemoteManager />);
    expect(sessionStorage.getItem('quickConnectHost')).toBeNull();
    await act(async () => { await vi.advanceTimersByTimeAsync(250); });
    expect(invoke).toHaveBeenCalledWith('is_vault_locked');
    expect(screen.getByTestId('selector')).toHaveTextContent('10.9.9.9');
  });

  it('a quick-connect event after mount triggers a connection', async () => {
    vi.useFakeTimers();
    mockVault(true);
    render(<RemoteManager />);
    sessionStorage.setItem('quickConnectHost', JSON.stringify({ id: 'q2', name: 'later', address: '10.8.8.8', protocol: 'ssh' }));
    act(() => { window.dispatchEvent(new Event('quickConnectTriggered')); });
    await act(async () => { await vi.advanceTimersByTimeAsync(250); });
    expect(promptProps?.hostName).toBe('later');
    expect(promptProps?.initialUsername).toBeUndefined();
  });

  it('malformed quick-connect data is ignored', () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    sessionStorage.setItem('quickConnectHost', '{not json');
    render(<RemoteManager />);
    expect(errorSpy).toHaveBeenCalledWith('Failed to parse quick connect host:', expect.anything());
    expect(screen.queryByTestId('selector')).not.toBeInTheDocument();
  });

  it('opening Add Host from the host list prefills the dialog', async () => {
    mockVault(false);
    render(<RemoteManager />);
    act(() => { hostListProps?.onAddHost({ name: 'pre', address: '1.2.3.4', protocol: 'ssh', port: 22, username: '' }); });
    expect(await screen.findByDisplayValue('1.2.3.4')).toBeInTheDocument();
  });
});
