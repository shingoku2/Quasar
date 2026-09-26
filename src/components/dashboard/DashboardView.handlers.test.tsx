import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import DashboardView from './DashboardView';
import type { ScanResult } from '../NetworkScanner';
import type { AddHostInitialValues } from '../AddHostDialog';
import '@testing-library/jest-dom';

// The children are replaced with thin stubs that expose the callbacks DashboardView
// hands them, so these tests exercise DashboardView's own handlers.
interface TopologyProps {
  hosts: ScanResult[];
  onHostClick: (h: ScanResult) => void;
  onHostConnect: (h: ScanResult) => void;
}
let topologyProps: TopologyProps | null = null;
vi.mock('../NetworkTopologyView', () => ({
  default: (props: TopologyProps) => {
    topologyProps = props;
    return <div data-testid="topology">{props.hosts.map((h) => h.ip).join(',')}</div>;
  },
}));

interface DetailProps {
  host: ScanResult | null;
  onClose: () => void;
  onConnect: (h: ScanResult) => void;
  onSave: (h: ScanResult) => void;
  onDelete: (ip: string) => void;
}
let detailProps: DetailProps | null = null;
vi.mock('../HostDetailDialog', () => ({
  default: (props: DetailProps) => {
    detailProps = props;
    return props.host ? <div data-testid="detail">{props.host.ip}</div> : null;
  },
}));

let addHostValues: AddHostInitialValues | undefined;
let addHostOnAdded: (() => void) | undefined;
vi.mock('../AddHostDialog', () => ({
  default: (props: { initialValues?: AddHostInitialValues; onClose: () => void; onAdded: () => void }) => {
    addHostValues = props.initialValues;
    addHostOnAdded = props.onAdded;
    return <div data-testid="add-host">{props.initialValues?.protocol}:{props.initialValues?.port}</div>;
  },
}));

vi.mock('./SystemHealthWidget', () => ({ default: () => null }));
vi.mock('./AlertFeed', () => ({ default: () => null }));
vi.mock('./QuickConnectWidget', () => ({ default: () => null }));

const persisted = {
  id: 'd1',
  ip: '10.0.0.5',
  hostname: null,
  mac_address: 'aa:bb',
  device_type: 'server',
  vendor: null,
  first_seen: 1,
  last_seen: 2,
  scan_count: 1,
  services: [{ port: 22, protocol: 'tcp', service: 'ssh', version: null }],
};

function host(overrides: Partial<ScanResult> = {}): ScanResult {
  return { ip: '10.0.0.9', is_alive: true, open_ports: [22], ...overrides } as ScanResult;
}

describe('DashboardView handlers', () => {
  const onNavigate = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    topologyProps = null;
    detailProps = null;
    addHostValues = undefined;
    addHostOnAdded = undefined;
    sessionStorage.clear();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('maps persisted discovered hosts into scan results', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([persisted]);
    render(<DashboardView onNavigate={onNavigate} />);
    await waitFor(() => expect(screen.getByTestId('topology')).toHaveTextContent('10.0.0.5'));
    expect(invoke).toHaveBeenCalledWith('get_discovered_hosts', { limit: 500 });
    const mapped = topologyProps?.hosts[0];
    expect(mapped?.open_ports).toEqual([22]);
    expect(mapped?.hostname).toBeUndefined();
    expect(mapped?.mac_address).toBe('aa:bb');
    expect(mapped?.services?.[0].version).toBeUndefined();
  });

  it('shows no hosts when loading the persisted list fails', async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error('db'));
    render(<DashboardView onNavigate={onNavigate} />);
    await waitFor(() => expect(invoke).toHaveBeenCalled());
    expect(topologyProps?.hosts).toEqual([]);
  });

  it('treats a non-array response as no hosts', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(undefined);
    render(<DashboardView onNavigate={onNavigate} />);
    await waitFor(() => expect(invoke).toHaveBeenCalled());
    expect(topologyProps?.hosts).toEqual([]);
  });

  it('connecting to an SSH host navigates to remote and hands the host over', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([]);
    const listener = vi.fn();
    window.addEventListener('quickConnectTriggered', listener);
    try {
      render(<DashboardView onNavigate={onNavigate} />);
      act(() => { topologyProps?.onHostConnect(host({ hostname: 'box', open_ports: [2222, 80] })); });
      expect(onNavigate).toHaveBeenCalledWith('remote');
      const stored = JSON.parse(sessionStorage.getItem('quickConnectHost') ?? '{}');
      expect(stored).toMatchObject({ name: 'box', address: '10.0.0.9', port: 2222, protocol: 'ssh', username: null });
      expect(listener).toHaveBeenCalledTimes(1);
    } finally {
      window.removeEventListener('quickConnectTriggered', listener);
    }
  });

  it('a host with port 3389 open connects as RDP and falls back to its IP as the name', () => {
    vi.mocked(invoke).mockResolvedValueOnce([]);
    render(<DashboardView onNavigate={onNavigate} />);
    act(() => { topologyProps?.onHostConnect(host({ open_ports: [3389] })); });
    const stored = JSON.parse(sessionStorage.getItem('quickConnectHost') ?? '{}');
    expect(stored).toMatchObject({ name: '10.0.0.9', protocol: 'rdp', port: 3389 });
  });

  it('a host with no open ports connects on port 22', () => {
    vi.mocked(invoke).mockResolvedValueOnce([]);
    render(<DashboardView onNavigate={onNavigate} />);
    act(() => { topologyProps?.onHostConnect(host({ open_ports: [] })); });
    const stored = JSON.parse(sessionStorage.getItem('quickConnectHost') ?? '{}');
    expect(stored.port).toBe(22);
  });

  it('clicking a host opens the detail dialog, and closing clears it', () => {
    vi.mocked(invoke).mockResolvedValueOnce([]);
    render(<DashboardView onNavigate={onNavigate} />);
    act(() => { topologyProps?.onHostClick(host()); });
    expect(screen.getByTestId('detail')).toHaveTextContent('10.0.0.9');
    act(() => { detailProps?.onClose(); });
    expect(screen.queryByTestId('detail')).not.toBeInTheDocument();
  });

  it.each([
    { ports: [80, 22], protocol: 'ssh', port: 22 },
    { ports: [22, 3389], protocol: 'rdp', port: 3389 },
    { ports: [8022], protocol: 'ssh', port: 8022 },
    { ports: [], protocol: 'ssh', port: 22 },
  ])('saving a host with ports $ports prefills $protocol:$port', ({ ports, protocol, port }) => {
    vi.mocked(invoke).mockResolvedValueOnce([]);
    render(<DashboardView onNavigate={onNavigate} />);
    act(() => { topologyProps?.onHostClick(host({ open_ports: ports })); });
    act(() => { detailProps?.onSave(host({ open_ports: ports })); });
    expect(screen.getByTestId('add-host')).toHaveTextContent(`${protocol}:${port}`);
    expect(addHostValues).toMatchObject({ name: '10.0.0.9', address: '10.0.0.9', username: '' });
  });

  it('adding the saved host closes both dialogs', () => {
    vi.mocked(invoke).mockResolvedValueOnce([]);
    render(<DashboardView onNavigate={onNavigate} />);
    act(() => { topologyProps?.onHostClick(host()); });
    act(() => { detailProps?.onSave(host()); });
    act(() => { addHostOnAdded?.(); });
    expect(screen.queryByTestId('add-host')).not.toBeInTheDocument();
    expect(screen.queryByTestId('detail')).not.toBeInTheDocument();
  });

  it('deleting a discovered host removes it from the list', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([persisted]).mockResolvedValueOnce(undefined);
    render(<DashboardView onNavigate={onNavigate} />);
    await waitFor(() => expect(screen.getByTestId('topology')).toHaveTextContent('10.0.0.5'));
    await act(async () => { await detailProps?.onDelete('10.0.0.5'); });
    expect(invoke).toHaveBeenCalledWith('delete_discovered_host', { ip: '10.0.0.5' });
    expect(screen.getByTestId('topology')).not.toHaveTextContent('10.0.0.5');
  });

  it('a failed delete keeps the host and tells the user', async () => {
    const alertSpy = vi.spyOn(window, 'alert').mockImplementation(() => {});
    vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.mocked(invoke).mockResolvedValueOnce([persisted]).mockRejectedValueOnce(new Error('nope'));
    render(<DashboardView onNavigate={onNavigate} />);
    await waitFor(() => expect(screen.getByTestId('topology')).toHaveTextContent('10.0.0.5'));
    await act(async () => { await detailProps?.onDelete('10.0.0.5'); });
    expect(alertSpy).toHaveBeenCalledWith('Failed to delete host');
    expect(screen.getByTestId('topology')).toHaveTextContent('10.0.0.5');
  });

  it('the list view toggle reflects the active mode', () => {
    vi.mocked(invoke).mockResolvedValue([]);
    render(<DashboardView onNavigate={onNavigate} />);
    fireEvent.click(screen.getByText('List'));
    expect(screen.queryByTestId('topology')).not.toBeInTheDocument();
    expect(screen.getByText('List').closest('button')).toHaveClass('bg-accent');
  });
});
