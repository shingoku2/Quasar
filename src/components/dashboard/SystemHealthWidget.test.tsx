import { render, screen, act, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import SystemHealthWidget from './SystemHealthWidget';
import '@testing-library/jest-dom';

// Mock the Tauri APIs
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

// Mock the visibility polling hook to just execute the callback
vi.mock('../../hooks/useViewVisibility', () => ({
  useVisiblePolling: vi.fn((callback) => {
    // We'll call this manually in tests or we can auto-call it here.
    // For predictability in testing, let's just make it available to be called
    // or run it once immediately. Let's auto-run it once for simplicity if it's not null.
    if (callback) {
      callback();
    }
  }),
}));

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

describe('SystemHealthWidget setup', () => {
  let listeners: Record<string, Function> = {};

  beforeEach(() => {
    vi.clearAllMocks();
    listeners = {};

    // Setup generic listen mock to capture the registered callbacks
    mockListen.mockImplementation((event, callback) => {
      listeners[event as string] = callback;
      return Promise.resolve(() => {
        delete listeners[event as string];
      });
    });

    // Default invoke implementations
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_system_metrics') {
        return Promise.resolve({
          cpu_usage_percent: 45,
          memory_usage_percent: 50,
          memory_used_mb: 8192,
          memory_total_mb: 16384,
          disk_usage_percent: 60,
          disk_used_gb: 300,
          disk_total_gb: 500,
          uptime_seconds: 100000,
          disks: []
        });
      }
      if (cmd === 'get_remote_hosts_health') {
        return Promise.resolve([]);
      }
      if (cmd === 'get_vault_settings') {
        return Promise.resolve({
          auto_lock_timeout_minutes: 15,
          require_password_on_credential_use: true,
          vault_initialized: true
        });
      }
      return Promise.reject(new Error(`Unhandled mock for ${cmd}`));
    });
  });

  describe('variant="summary"', () => {
    it('renders the "System Health" title and summary items', async () => {
      await act(async () => {
        render(<SystemHealthWidget variant="summary" />);
      });
      expect(screen.getByText('System Health')).toBeInTheDocument();
      expect(screen.getByText('Hosts Online')).toBeInTheDocument();
      expect(screen.getByText('Alerts')).toBeInTheDocument();
      expect(screen.getByText('Vault Auto-lock')).toBeInTheDocument();
    });

    it('calculates hosts online count correctly', async () => {
      mockInvoke.mockImplementation((cmd) => {
        if (cmd === 'get_remote_hosts_health') {
          return Promise.resolve([
            { reachable: true },
            { reachable: false },
            { reachable: true }
          ]);
        }
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<SystemHealthWidget variant="summary" />);
      });

      // Based on the above mock, online count should be 2
      await waitFor(() => {
        expect(screen.getByText('2')).toBeInTheDocument();
      });
    });

    it('shows the mocked vault timeout in minutes', async () => {
      mockInvoke.mockImplementation((cmd) => {
        if (cmd === 'get_vault_settings') {
          return Promise.resolve({
            auto_lock_timeout_minutes: 42,
            require_password_on_credential_use: true,
            vault_initialized: true
          });
        }
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<SystemHealthWidget variant="summary" />);
      });

      await waitFor(() => {
        expect(screen.getByText('42 min')).toBeInTheDocument();
      });
    });

    it('updates active alert count via events', async () => {
      await act(async () => {
        render(<SystemHealthWidget variant="summary" />);
      });

      // 0 hosts online, 0 alerts initially
      const zeroes = screen.getAllByText('0');
      expect(zeroes.length).toBeGreaterThan(0);
      expect(zeroes[0]).toBeInTheDocument();

      // Trigger 2 new alerts
      await act(async () => {
        if (listeners['alerts-triggered']) {
          listeners['alerts-triggered']({
            payload: [
              { id: '1', rule_id: 'rule1', severity: 'critical' },
              { id: '2', rule_id: 'rule2', severity: 'warning' },
            ]
          });
        }
      });

      expect(screen.getByText('2')).toBeInTheDocument();

      // Recover rule1 (should decrement active count to 1)
      await act(async () => {
        if (listeners['alerts-recovered']) {
          listeners['alerts-recovered']({
            payload: [{ rule_id: 'rule1' }]
          });
        }
      });

      expect(screen.getByText('1')).toBeInTheDocument();
    });
  });

  describe('variant="metrics" (default)', () => {
    it('renders the "Real-time Metrics" title and categories', async () => {
      await act(async () => {
        render(<SystemHealthWidget variant="metrics" />);
      });

      // Based on our default mock get_system_metrics return
      await waitFor(() => {
        expect(screen.getByText('Real-time Metrics')).toBeInTheDocument();
        expect(screen.getByText('CPU')).toBeInTheDocument();
        expect(screen.getByText('Memory')).toBeInTheDocument();
      });
    });

    it('calculates and renders memory and cpu usage', async () => {
      await act(async () => {
        render(<SystemHealthWidget />); // default variant is metrics
      });

      await waitFor(() => {
        expect(screen.getByText('45%')).toBeInTheDocument(); // CPU usage
        expect(screen.getByText('8.0GB/16GB')).toBeInTheDocument(); // Memory used/total
      });
    });

    it('updates metrics when system-metrics event is fired', async () => {
      await act(async () => {
        render(<SystemHealthWidget />);
      });

      await waitFor(() => {
        expect(screen.getByText('45%')).toBeInTheDocument();
      });

      await act(async () => {
        if (listeners['system-metrics']) {
          listeners['system-metrics']({
            payload: {
              cpu_usage_percent: 85, // Updated CPU
              memory_usage_percent: 60,
              memory_used_mb: 4096,
              memory_total_mb: 8192,
              disk_usage_percent: 50,
              disk_used_gb: 100,
              disk_total_gb: 200,
              uptime_seconds: 1000,
              disks: []
            }
          });
        }
      });

      expect(screen.getByText('85%')).toBeInTheDocument();
      expect(screen.getByText('4.0GB/8GB')).toBeInTheDocument();
    });

    it('renders multiple disks if provided in metrics', async () => {
      mockInvoke.mockImplementation((cmd) => {
        if (cmd === 'get_system_metrics') {
          return Promise.resolve({
            cpu_usage_percent: 10,
            memory_usage_percent: 20,
            memory_used_mb: 1024,
            memory_total_mb: 8192,
            disk_usage_percent: 0,
            disk_used_gb: 0,
            disk_total_gb: 0,
            uptime_seconds: 1000,
            disks: [
              { name: 'Root', mount_point: '/', total_gb: 100, used_gb: 50, free_gb: 50, usage_percent: 50 },
              { name: 'Data', mount_point: '/data', total_gb: 500, used_gb: 400, free_gb: 100, usage_percent: 80 }
            ]
          });
        }
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<SystemHealthWidget />);
      });

      await waitFor(() => {
        expect(screen.getByText('/')).toBeInTheDocument();
        expect(screen.getByText('/data')).toBeInTheDocument();
        expect(screen.getByText('50GB/100GB')).toBeInTheDocument();
        expect(screen.getByText('400GB/500GB')).toBeInTheDocument();
      });
    });
  });
});
