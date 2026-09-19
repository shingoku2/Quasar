import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import HostList from './HostList';
import { resetTailscaleStatusCache } from '../hooks/useTailscaleStatus';
import '@testing-library/jest-dom';

// Mock Tauri invoke
const { mockInvoke, defaultInvoke } = vi.hoisted(() => {
  const mockHosts = [
    { id: '1', name: 'Prod Web', address: '10.0.0.10', protocol: 'ssh', port: 22 },
    { id: '2', name: 'Dev DB', address: '10.0.0.20', protocol: 'database', port: 5432 },
  ];

  const defaultInvoke = (cmd: string, _args?: unknown): Promise<unknown> => {
    if (cmd === 'get_saved_hosts') return Promise.resolve(mockHosts);
    if (cmd === 'remove_saved_hosts') return Promise.resolve();
    if (cmd === 'get_tailscale_status') {
      return Promise.resolve({ installed: false, backend_state: '', magic_dns_enabled: false, magic_dns_suffix: null, self_node: null, peers: [] });
    }
    return Promise.resolve(null);
  };
  return { mockInvoke: vi.fn(defaultInvoke), defaultInvoke, mockHosts };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: mockInvoke }));

// Mock subcomponents
vi.mock('./Discovery', () => ({
  default: () => <div data-testid="discovery-mock" />,
}));

vi.mock('./HealthCheckBadge', () => ({
  default: ({ host }: { host: string }) => <span data-testid={`health-badge-${host}`}>OK</span>,
}));

describe('HostList Component', () => {
  const mockProps = {
    onConnect: vi.fn(),
    onSftp: vi.fn(),
    onAddHost: vi.fn(),
  };

  let confirmSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(defaultInvoke);
    vi.clearAllMocks();
    // The Tailscale status poll is shared process-wide; drop it so one test's
    // peers can't leak into the next.
    resetTailscaleStatusCache();
    confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
  });

  afterEach(() => {
    confirmSpy.mockRestore();
  });

  it('renders loading state initially', async () => {
    // We don't await the resolve here to catch the loading state
    render(<HostList {...mockProps} />);
    expect(screen.getByText('Loading hosts...')).toBeInTheDocument();

    // Let the fetch complete to avoid unhandled state updates
    await waitFor(() => {
      expect(screen.queryByText('Loading hosts...')).not.toBeInTheDocument();
    });
  });

  it('renders hosts list after fetching', async () => {
    render(<HostList {...mockProps} />);

    await waitFor(() => {
      expect(screen.queryByText('Loading hosts...')).not.toBeInTheDocument();
    });

    expect(screen.getByText('Prod Web')).toBeInTheDocument();
    expect(screen.getByText('Dev DB')).toBeInTheDocument();
    expect(screen.getByText('10.0.0.10:22')).toBeInTheDocument();
    expect(screen.getByText('10.0.0.20:5432')).toBeInTheDocument();
  });

  it('handles filtering of hosts', async () => {
    render(<HostList {...mockProps} />);

    await waitFor(() => {
      expect(screen.getByText('Prod Web')).toBeInTheDocument();
    });

    const filterInput = screen.getByPlaceholderText('Filter hosts...');
    fireEvent.change(filterInput, { target: { value: 'dev' } });

    expect(screen.queryByText('Prod Web')).not.toBeInTheDocument();
    expect(screen.getByText('Dev DB')).toBeInTheDocument();
  });

  it('handles empty host list', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_saved_hosts') return Promise.resolve([]);
      return Promise.resolve(null);
    });

    render(<HostList {...mockProps} />);

    await waitFor(() => {
      expect(screen.getByText('No hosts added yet.')).toBeInTheDocument();
    });
  });

  it('handles filtering with no match', async () => {
    render(<HostList {...mockProps} />);
    await waitFor(() => {
      expect(screen.getByText('Prod Web')).toBeInTheDocument();
    });

    const filterInput = screen.getByPlaceholderText('Filter hosts...');
    fireEvent.change(filterInput, { target: { value: 'nonexistent' } });

    expect(screen.getByText('No hosts match your filter.')).toBeInTheDocument();
  });

  it('handles duplicate hosts and remove functionality', async () => {
    const duplicatesList = [
      { id: '1', name: 'Prod Web', address: '10.0.0.10', protocol: 'ssh', port: 22 },
      { id: '2', name: 'Prod Web Duplicate', address: '10.0.0.10', protocol: 'ssh', port: 22 }, // Duplicate based on logic
    ];
    const afterRemoval = [duplicatesList[0]];

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_saved_hosts') {
        return Promise.resolve(mockInvoke.mock.calls.some(([c]) => c === 'remove_saved_hosts') ? afterRemoval : duplicatesList);
      }
      if (cmd === 'remove_saved_hosts') return Promise.resolve();
      return Promise.resolve(null);
    });

    render(<HostList {...mockProps} />);

    await waitFor(() => {
      expect(screen.getByText('1 duplicate detected')).toBeInTheDocument();
    });

    const removeBtn = screen.getByText('Remove duplicates (1)');
    expect(removeBtn).not.toBeDisabled();

    fireEvent.click(removeBtn);

    expect(window.confirm).toHaveBeenCalledWith('Remove 1 duplicate saved host(s)?');

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('remove_saved_hosts', { ids: ['2'] });
    });

    // Refresh after removal should drop the duplicate from the rendered list.
    await waitFor(() => {
      expect(screen.queryByText('Prod Web Duplicate')).not.toBeInTheDocument();
      expect(screen.queryByText('1 duplicate detected')).not.toBeInTheDocument();
    });
  });

  it('handles removing a single host', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_saved_hosts') {
        const removed = mockInvoke.mock.calls.some(([c]) => c === 'remove_saved_hosts');
        return Promise.resolve(
          removed
            ? [{ id: '2', name: 'Dev DB', address: '10.0.0.20', protocol: 'database', port: 5432 }]
            : [
                { id: '1', name: 'Prod Web', address: '10.0.0.10', protocol: 'ssh', port: 22 },
                { id: '2', name: 'Dev DB', address: '10.0.0.20', protocol: 'database', port: 5432 },
              ],
        );
      }
      if (cmd === 'remove_saved_hosts') return Promise.resolve();
      return Promise.resolve(null);
    });

    render(<HostList {...mockProps} />);

    await waitFor(() => {
      expect(screen.getByText('Prod Web')).toBeInTheDocument();
    });

    const removeButtons = screen.getAllByText('Remove');
    fireEvent.click(removeButtons[0]);

    expect(window.confirm).toHaveBeenCalledWith('Remove Prod Web from saved hosts?');

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('remove_saved_hosts', { ids: ['1'] });
    });

    // Refresh after removal should drop the removed host from the rendered list.
    await waitFor(() => {
      expect(screen.queryByText('Prod Web')).not.toBeInTheDocument();
    });
    expect(screen.getByText('Dev DB')).toBeInTheDocument();
  });

  it('calls respective action handlers and gates them by protocol', async () => {
    render(<HostList {...mockProps} />);

    await waitFor(() => {
      expect(screen.getByText('Prod Web')).toBeInTheDocument();
    });

    const rows = screen.getAllByRole('row');
    // Header row + 2 host rows.
    const sshRow = rows.find((row) => within(row).queryByText('Prod Web'))!;
    const dbRow = rows.find((row) => within(row).queryByText('Dev DB'))!;

    // SSH Host has Connect, SFTP, Edit.
    expect(within(sshRow).getByText('Connect')).toBeInTheDocument();
    expect(within(sshRow).getByText('SFTP')).toBeInTheDocument();

    // Database host is not connectable and has no SFTP support.
    expect(within(dbRow).queryByText('Connect')).not.toBeInTheDocument();
    expect(within(dbRow).queryByText('SFTP')).not.toBeInTheDocument();

    fireEvent.click(within(sshRow).getByText('Connect'));
    expect(mockProps.onConnect).toHaveBeenCalledWith(expect.objectContaining({ id: '1', name: 'Prod Web' }));

    fireEvent.click(within(sshRow).getByText('SFTP'));
    expect(mockProps.onSftp).toHaveBeenCalledWith(expect.objectContaining({ id: '1' }));

    fireEvent.click(within(sshRow).getByText('Edit'));
    expect(mockProps.onAddHost).toHaveBeenCalledWith(expect.objectContaining({ id: '1' }));
  });

  it('shows a Tailscale badge and online dot for a host matching a tailnet peer', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_saved_hosts') return Promise.resolve([
        { id: '1', name: 'Prod Web', address: '10.0.0.10', protocol: 'ssh', port: 22 },
        { id: '2', name: 'Dev DB', address: '10.0.0.20', protocol: 'database', port: 5432 },
      ]);
      if (cmd === 'get_tailscale_status') {
        return Promise.resolve({
          installed: true,
          backend_state: 'Running',
          magic_dns_enabled: true,
          magic_dns_suffix: 'tail1234.ts.net',
          self_node: null,
          peers: [
            {
              id: 'nVPS',
              hostname: 'Prod Web',
              dns_name: '',
              ipv4: '10.0.0.10',
              os: 'linux',
              online: true,
              tailscale_ssh: true,
              exit_node: false,
              preferred_address: '10.0.0.10',
            },
          ],
        });
      }
      return Promise.resolve(null);
    });

    render(<HostList {...mockProps} />);

    // Match on the address cell: the peer shares its name with the saved host,
    // so 'Prod Web' also appears in the Tailscale peers panel.
    await waitFor(() => {
      expect(screen.getByText('10.0.0.10:22')).toBeInTheDocument();
    });

    const rows = screen.getAllByRole('row');
    const sshRow = rows.find((row) => within(row).queryByText('Prod Web'))!;
    const dbRow = rows.find((row) => within(row).queryByText('Dev DB'))!;

    await waitFor(() => {
      expect(within(sshRow).getByText('Tailscale')).toBeInTheDocument();
    });
    // Dev DB (10.0.0.20) has no matching peer.
    expect(within(dbRow).queryByText('Tailscale')).not.toBeInTheDocument();
  });
});
