import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import RemoteHostsList from './RemoteHostsList';
import type { CredentialSummary, MonitoredHost, RemoteHostMetric } from './types';
import '@testing-library/jest-dom';

const host = (id: string, overrides: Partial<RemoteHostMetric> = {}): RemoteHostMetric => ({
  id,
  name: `Server ${id}`,
  address: `10.0.0.${id}`,
  port: 22,
  reachable: true,
  latency_ms: 10,
  error: null,
  ...overrides,
});

describe('RemoteHostsList', () => {
  // FE-021 review: each host's label must name its own picker, not the first one's.
  it('gives every host its own labelled credential picker', () => {
    render(
      <RemoteHostsList
        remoteHosts={[host('1'), host('2')]}
        savedHosts={[]}
        credentials={[]}
        setHostCredential={vi.fn()}
      />,
    );
    const pickers = screen.getAllByLabelText('SSH metrics credential');
    expect(pickers).toHaveLength(2);
    expect(new Set(pickers.map((p) => p.id)).size).toBe(2);
  });

  it('renders empty state when remoteHosts is empty', () => {
    render(
      <RemoteHostsList
        remoteHosts={[]}
        savedHosts={[]}
        credentials={[]}
        setHostCredential={vi.fn()}
      />,
    );
    expect(
      screen.getByText('No saved hosts. Add hosts in Remote to see them here.'),
    ).toBeInTheDocument();
  });

  it('renders online and offline status, latency, and errors', () => {
    const onlineHost = host('1', { reachable: true, latency_ms: 42 });
    const offlineHost = host('2', {
      reachable: false,
      latency_ms: null,
      error: 'Host unreachable',
    });
    const offlineNoError = host('3', {
      reachable: false,
      latency_ms: null,
      error: null,
    });

    render(
      <RemoteHostsList
        remoteHosts={[onlineHost, offlineHost, offlineNoError]}
        savedHosts={[]}
        credentials={[]}
        setHostCredential={vi.fn()}
      />,
    );

    expect(screen.getByText('42 ms')).toBeInTheDocument();
    expect(screen.getByText('Host unreachable')).toBeInTheDocument();
    expect(screen.getByText('—')).toBeInTheDocument();
    expect(screen.getAllByText('Online')).toHaveLength(1);
    expect(screen.getAllByText('Offline')).toHaveLength(2);
  });

  it('renders host metrics when available and non-zero', () => {
    const hostWithMetrics = host('1', {
      metrics: {
        cpu_percent: 75.4,
        memory_used_mb: 2048,
        memory_total_mb: 4096,
        disk_used_gb: 50,
        disk_total_gb: 100,
      },
    });

    render(
      <RemoteHostsList
        remoteHosts={[hostWithMetrics]}
        savedHosts={[]}
        credentials={[]}
        setHostCredential={vi.fn()}
      />,
    );

    expect(screen.getByText('75%')).toBeInTheDocument();
    expect(screen.getAllByText('50%')).toHaveLength(2); // Mem: 50%, Disk: 50%
  });

  it('filters credentials to SSH_CREDENTIAL_TYPES only and updates on selection', () => {
    const setHostCredential = vi.fn();
    const savedHosts: MonitoredHost[] = [
      { id: '1', name: 'Server 1', address: '10.0.0.1', port: 22, protocol: 'ssh', credential_id: 'c-ssh' },
    ];
    const credentials: CredentialSummary[] = [
      { id: 'c-ssh', name: 'SSH Cred', username: 'root', credential_type: 'ssh' },
      { id: 'c-key', name: 'Key Cred', username: 'root', credential_type: 'ssh_key' },
      { id: 'c-pw', name: 'Password Cred', username: 'root', credential_type: 'password' },
      { id: 'c-api', name: 'API Cred', username: 'root', credential_type: 'api' },
      { id: 'c-rdp', name: 'RDP Cred', username: 'root', credential_type: 'rdp' },
    ];

    render(
      <RemoteHostsList
        remoteHosts={[host('1')]}
        savedHosts={savedHosts}
        credentials={credentials}
        setHostCredential={setHostCredential}
      />,
    );

    const select = screen.getByLabelText('SSH metrics credential') as HTMLSelectElement;
    expect(select.value).toBe('c-ssh');

    // 'api' and 'rdp' credentials should not be in the options
    expect(screen.getByRole('option', { name: 'SSH Cred' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'Key Cred' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'Password Cred' })).toBeInTheDocument();
    expect(screen.queryByRole('option', { name: 'API Cred' })).not.toBeInTheDocument();
    expect(screen.queryByRole('option', { name: 'RDP Cred' })).not.toBeInTheDocument();

    // Selecting a new credential
    fireEvent.change(select, { target: { value: 'c-key' } });
    expect(setHostCredential).toHaveBeenCalledWith('1', 'c-key');

    // Selecting None (value: "")
    fireEvent.change(select, { target: { value: '' } });
    expect(setHostCredential).toHaveBeenCalledWith('1', null);
  });
});
