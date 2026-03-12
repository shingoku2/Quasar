import { render, screen, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import MonitoringView from './MonitoringView';
import '@testing-library/jest-dom';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

const mockMetrics = {
  cpu_usage_percent: 42,
  memory_used_mb: 4096,
  memory_total_mb: 8192,
  memory_usage_percent: 50,
  disk_read_mb: 1,
  disk_write_mb: 2,
  network_rx_mb: 0.5,
  network_tx_mb: 0.3,
  timestamp: Date.now(),
  uptime_seconds: 86400,
  load_average_1m: 0.5,
  load_average_5m: 0.4,
  load_average_15m: 0.3,
  process_count: 150,
  boot_time: 0,
  cpu_count: 4,
  cpu_per_core: [40, 44, 38, 46],
  cpu_frequency_mhz: 2400,
  disk_total_gb: 500,
  disk_used_gb: 250,
  disk_free_gb: 250,
  disk_usage_percent: 50,
  network_packets_rx: 1000,
  network_packets_tx: 800,
  network_errors_rx: 0,
  network_errors_tx: 0,
  top_cpu_processes: [],
  top_memory_processes: [],
};

describe('MonitoringView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the System Monitoring heading', () => {
    render(<MonitoringView />);
    expect(screen.getByText('System Monitoring')).toBeInTheDocument();
  });

  it('renders remote hosts section', () => {
    render(<MonitoringView />);
    expect(screen.getByText('Remote hosts')).toBeInTheDocument();
  });

  it('displays CPU usage when metrics are loaded', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_system_metrics') return Promise.resolve(mockMetrics);
      return Promise.resolve([]);
    });
    render(<MonitoringView />);
    await waitFor(() => {
      expect(screen.getByText(/42/)).toBeInTheDocument();
    });
  });

  it('renders disk space section', async () => {
    render(<MonitoringView />);
    await waitFor(() => {
      expect(screen.getByText('Disk Space')).toBeInTheDocument();
    });
  });

  it('shows "No saved hosts" when host list is empty', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_saved_hosts') return Promise.resolve([]);
      return Promise.resolve(null);
    });
    render(<MonitoringView />);
    await waitFor(() => {
      expect(screen.getByText(/No saved hosts/i)).toBeInTheDocument();
    });
  });
});
