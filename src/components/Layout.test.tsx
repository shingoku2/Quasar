import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import Layout from './Layout';
import '@testing-library/jest-dom';

vi.mock('./vault/VaultProvider', () => ({
  useVault: () => ({ isVaultLocked: false, lockVault: vi.fn(), unlockVault: vi.fn() }),
  VaultProvider: ({ children }: any) => children,
}));

vi.mock('./NetworkTopologyView', () => ({
  default: () => <div data-testid="topology-mock">Topology Mock</div>,
}));

// Override invoke for Layout-specific commands
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn((cmd: string) => {
    if (cmd === 'check_ai_status') return Promise.resolve(false);
    if (cmd === 'get_system_metrics') return Promise.resolve({ cpu_usage_percent: 10, memory_usage_percent: 50, disk_usage_percent: 30, uptime_seconds: 1000, hostname: 'test', os_info: 'test', cpu_count: 4, total_memory_mb: 8192, used_memory_mb: 4096, total_disk_mb: 500000, used_disk_mb: 150000, network_rx_bytes: 0, network_tx_bytes: 0, load_average_1m: 0.5, load_average_5m: 0.5, load_average_15m: 0.5, timestamp: Date.now(), top_cpu_processes: [], top_memory_processes: [] });
    if (cmd === 'get_alert_rules') return Promise.resolve([]);
    if (cmd === 'get_discovered_hosts') return Promise.resolve([]);
    if (cmd === 'get_remote_hosts_health') return Promise.resolve([]);
    if (cmd === 'list_credentials') return Promise.resolve([]);
    if (cmd === 'get_known_ssh_hosts') return Promise.resolve([]);
    if (cmd === 'get_audit_logs') return Promise.resolve([]);
    if (cmd === 'get_audit_log_count') return Promise.resolve(0);
    if (cmd === 'get_vault_settings') return Promise.resolve({ auto_lock_timeout: 15 });
<<<<<<< HEAD
    if (cmd === 'list_scheduled_tasks') return Promise.resolve([]);
    if (cmd === 'get_saved_hosts') return Promise.resolve([]);
=======
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
    return Promise.resolve();
  }),
}));

describe('Layout Component', () => {
  it('renders sidebar and top bar', async () => {
    render(<Layout />);
    expect(screen.getByText('QUASAR')).toBeInTheDocument();
  });

  it('can switch views via sidebar', async () => {
    render(<Layout />);

    const remoteButton = screen.getByTitle('Remote');
    fireEvent.click(remoteButton);

    await waitFor(() => {
      expect(screen.getByText('Remote Hosts')).toBeInTheDocument();
    });
  });
});