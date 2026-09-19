import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import TailscalePeers from './TailscalePeers';
import { Host } from './HostList';
import { resetTailscaleStatusCache } from '../hooks/useTailscaleStatus';
import '@testing-library/jest-dom';

const { mockInvoke } = vi.hoisted(() => ({ mockInvoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: mockInvoke }));

const hosts: Host[] = [];

const peer = (overrides: Partial<Record<string, unknown>> = {}) => ({
  id: 'nVPS',
  hostname: 'vps-a8fa83ff',
  dns_name: 'vps-a8fa83ff.tail1234.ts.net',
  ipv4: '100.64.0.2',
  os: 'linux',
  online: true,
  tailscale_ssh: true,
  exit_node: false,
  preferred_address: 'vps-a8fa83ff.tail1234.ts.net',
  ...overrides,
});

describe('TailscalePeers', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    // The status poll is shared process-wide; drop it between tests.
    resetTailscaleStatusCache();
  });

  it('shows an install hint when the CLI is not found', async () => {
    mockInvoke.mockResolvedValue({
      installed: false, backend_state: '', magic_dns_enabled: false,
      magic_dns_suffix: null, self_node: null, peers: [],
    });

    render(<TailscalePeers hosts={hosts} onAddHost={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/Tailscale CLI not found/i)).toBeInTheDocument();
    });
  });

  it('shows a needs-login hint when the backend is not running', async () => {
    mockInvoke.mockResolvedValue({
      installed: true, backend_state: 'NeedsLogin', magic_dns_enabled: true,
      magic_dns_suffix: 'tail1234.ts.net', self_node: null, peers: [],
    });

    render(<TailscalePeers hosts={hosts} onAddHost={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/needs attention/i)).toBeInTheDocument();
      expect(screen.getByText(/NeedsLogin/)).toBeInTheDocument();
    });
  });

  it('lists peers with an SSH chip and online indicator', async () => {
    mockInvoke.mockResolvedValue({
      installed: true, backend_state: 'Running', magic_dns_enabled: true,
      magic_dns_suffix: 'tail1234.ts.net', self_node: null,
      peers: [peer(), peer({ id: 'nPHONE', hostname: 'Pixel', tailscale_ssh: false, online: false, preferred_address: '100.64.0.3', dns_name: '', ipv4: '100.64.0.3' })],
    });

    render(<TailscalePeers hosts={hosts} onAddHost={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('vps-a8fa83ff')).toBeInTheDocument();
      expect(screen.getByText('Pixel')).toBeInTheDocument();
    });
    expect(screen.getByText('SSH')).toBeInTheDocument();
  });

  it('calls onAddHost with the preferred address when Add is clicked', async () => {
    mockInvoke.mockResolvedValue({
      installed: true, backend_state: 'Running', magic_dns_enabled: true,
      magic_dns_suffix: 'tail1234.ts.net', self_node: null, peers: [peer()],
    });
    const onAddHost = vi.fn();

    render(<TailscalePeers hosts={hosts} onAddHost={onAddHost} />);

    await waitFor(() => expect(screen.getByText('vps-a8fa83ff')).toBeInTheDocument());
    fireEvent.click(screen.getByText('Add'));

    expect(onAddHost).toHaveBeenCalledWith({
      name: 'vps-a8fa83ff',
      address: 'vps-a8fa83ff.tail1234.ts.net',
      protocol: 'ssh',
      port: 22,
      username: '',
    });
  });

  it('shows "Saved" instead of Add for a peer matching an existing saved host', async () => {
    mockInvoke.mockResolvedValue({
      installed: true, backend_state: 'Running', magic_dns_enabled: true,
      magic_dns_suffix: 'tail1234.ts.net', self_node: null, peers: [peer()],
    });
    const savedHosts: Host[] = [
      { id: '1', name: 'VPS', address: 'vps-a8fa83ff.tail1234.ts.net', protocol: 'ssh', port: 22 },
    ];

    render(<TailscalePeers hosts={savedHosts} onAddHost={vi.fn()} />);

    await waitFor(() => expect(screen.getByText('vps-a8fa83ff')).toBeInTheDocument());
    expect(screen.getByText('Saved')).toBeInTheDocument();
    expect(screen.queryByText('Add')).not.toBeInTheDocument();
  });

  it('shows an empty state when there are no peers', async () => {
    mockInvoke.mockResolvedValue({
      installed: true, backend_state: 'Running', magic_dns_enabled: true,
      magic_dns_suffix: 'tail1234.ts.net', self_node: null, peers: [],
    });

    render(<TailscalePeers hosts={hosts} onAddHost={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/No other devices on your tailnet/i)).toBeInTheDocument();
    });
  });
});
