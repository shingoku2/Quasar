import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import Discovery from './Discovery';
import '@testing-library/jest-dom';

interface TrackedHost {
  ip: string;
  hostname?: string;
  services: Array<{ port: number }>;
}

const { mockInvoke, listeners, unlistenFns } = vi.hoisted(() => {
  const listeners = new Map<string, (event: { payload: unknown }) => void>();
  const unlistenFns: Array<ReturnType<typeof vi.fn>> = [];
  return { mockInvoke: vi.fn(), listeners, unlistenFns };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: mockInvoke }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, callback: (event: { payload: unknown }) => void) => {
    listeners.set(name, callback);
    const unlisten = vi.fn();
    unlistenFns.push(unlisten);
    return unlisten;
  }),
  emit: vi.fn(async () => undefined),
  once: vi.fn(async () => () => {}),
}));

const tracked = (ip: string, hostname: string | undefined, ports: number[]): TrackedHost => ({
  ip,
  hostname,
  services: ports.map((port) => ({ port })),
});

describe('Discovery', () => {
  let discoveredHosts: TrackedHost[] = [];

  beforeEach(() => {
    mockInvoke.mockReset();
    listeners.clear();
    unlistenFns.length = 0;
    discoveredHosts = [];
    mockInvoke.mockImplementation(async (command: string) => {
      if (command === 'get_discovered_hosts') return discoveredHosts;
      return undefined;
    });
  });

  it('shows the empty state and starts discovery on mount', async () => {
    render(<Discovery onAddHost={vi.fn()} />);

    expect(await screen.findByText(/No devices found yet/i)).toBeInTheDocument();
    expect(screen.getByText('Scanning...')).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith('get_discovered_hosts', { limit: 100 });
    expect(mockInvoke).toHaveBeenCalledWith('start_discovery');
  });

  it('treats an undefined persisted list as empty without crashing', async () => {
    mockInvoke.mockImplementation(async (command: string) => {
      if (command === 'get_discovered_hosts') return undefined;
      return undefined;
    });

    render(<Discovery onAddHost={vi.fn()} />);

    expect(await screen.findByText(/No devices found yet/i)).toBeInTheDocument();
  });

  it('maps persisted scan hosts: prefers port 22, falls back, and uses the IP as the name', async () => {
    discoveredHosts = [
      tracked('10.0.0.9', undefined, [2222, 22]),
      tracked('10.0.0.10', 'printer.local', [8080]),
      tracked('10.0.0.11', undefined, []),
    ];

    render(<Discovery onAddHost={vi.fn()} />);

    expect(await screen.findByText('10.0.0.9:22')).toBeInTheDocument();
    expect(screen.getByText('printer.local')).toBeInTheDocument();
    expect(screen.getByText('10.0.0.10:8080')).toBeInTheDocument();
    expect(screen.getByText('10.0.0.11:22')).toBeInTheDocument();
    expect(screen.getByText('10.0.0.9')).toBeInTheDocument();
  });

  it('calls onAddHost with the mapped host when Add is clicked', async () => {
    discoveredHosts = [tracked('10.0.0.9', 'nas.local', [22])];
    const onAddHost = vi.fn();

    render(<Discovery onAddHost={onAddHost} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Add' }));

    expect(onAddHost).toHaveBeenCalledWith({
      name: 'nas.local',
      address: '10.0.0.9',
      port: 22,
      service_type: 'network-scan',
    });
  });

  it('disables Add when no onAddHost handler is provided', async () => {
    discoveredHosts = [tracked('10.0.0.9', 'nas.local', [22])];

    render(<Discovery />);

    expect(await screen.findByRole('button', { name: 'Add' })).toBeDisabled();
  });

  it('merges mDNS events without duplicating rows for the same address', async () => {
    discoveredHosts = [tracked('10.0.0.5', 'nas', [22])];

    render(<Discovery onAddHost={vi.fn()} />);

    await screen.findByText('10.0.0.5:22');

    act(() => {
      listeners.get('host-discovered')?.({
        payload: { name: 'nas.local', address: '10.0.0.5', port: 22, service_type: 'discovered' },
      });
    });
    expect(screen.getAllByRole('listitem')).toHaveLength(1);

    // The mDNS dedupe is by address: a second port on the same address is dropped.
    act(() => {
      listeners.get('host-discovered')?.({
        payload: { name: 'nas-alt', address: '10.0.0.5', port: 2222, service_type: 'discovered' },
      });
    });
    expect(screen.getAllByRole('listitem')).toHaveLength(1);
    expect(screen.queryByText('10.0.0.5:2222')).not.toBeInTheDocument();

    // A new address does add a row.
    act(() => {
      listeners.get('host-discovered')?.({
        payload: { name: 'fresh', address: '10.0.0.99', port: 22, service_type: 'discovered' },
      });
    });
    expect(screen.getAllByRole('listitem')).toHaveLength(2);
    expect(screen.getByText('10.0.0.99:22')).toBeInTheDocument();
  });

  it('refetches persisted hosts when the scan completes', async () => {
    discoveredHosts = [tracked('10.0.0.5', 'nas', [22])];

    render(<Discovery onAddHost={vi.fn()} />);

    await screen.findByText('10.0.0.5:22');
    discoveredHosts = [tracked('10.0.0.5', 'nas', [22]), tracked('10.0.0.6', 'web', [80])];

    await act(async () => {
      await listeners.get('scan_complete')?.({ payload: null });
    });

    expect(await screen.findByText('10.0.0.6:80')).toBeInTheDocument();
    const fetchCount = mockInvoke.mock.calls.filter(
      (call) => call[0] === 'get_discovered_hosts',
    ).length;
    expect(fetchCount).toBe(2);
  });

  it('unlistens both events on unmount', async () => {
    const { unmount } = render(<Discovery onAddHost={vi.fn()} />);

    await waitFor(() => {
      expect(listeners.has('host-discovered')).toBe(true);
      expect(listeners.has('scan_complete')).toBe(true);
    });
    expect(unlistenFns).toHaveLength(2);

    unmount();

    unlistenFns.forEach((unlisten) => expect(unlisten).toHaveBeenCalledTimes(1));
  });

  it('survives a failed persisted-hosts fetch and stays usable', async () => {
    mockInvoke.mockImplementation(async (command: string) => {
      if (command === 'get_discovered_hosts') throw new Error('db unavailable');
      return undefined;
    });
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});

    render(<Discovery onAddHost={vi.fn()} />);

    expect(await screen.findByText(/No devices found yet/i)).toBeInTheDocument();
    expect(warnSpy).toHaveBeenCalled();
    warnSpy.mockRestore();
  });
});
