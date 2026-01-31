import { render, screen, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import PreflightDialog from './PreflightDialog';
import '@testing-library/jest-dom';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({
    host: '192.168.1.1',
    reachable: true,
    latency_ms: 10,
    metrics: {
      cpu_percent: 45.5,
      memory_used_mb: 4096,
      memory_total_mb: 8192,
      uptime_seconds: 86400
    }
  }))
}));

describe('PreflightDialog', () => {
  it('renders loading state initially', () => {
    render(
      <PreflightDialog
        host="192.168.1.1"
        port={22}
        username="admin"
        onConnect={() => {}}
        onCancel={() => {}}
      />
    );
    
    expect(screen.getByText('Pre-flight Check')).toBeInTheDocument();
  });

  it('shows reachable status', async () => {
    render(
      <PreflightDialog
        host="192.168.1.1"
        port={22}
        username="admin"
        onConnect={() => {}}
        onCancel={() => {}}
      />
    );
    
    await waitFor(() => {
      expect(screen.getByText('Host is reachable')).toBeInTheDocument();
    });
  });

  it('calls onCancel when cancel button clicked', () => {
    const onCancel = vi.fn();
    render(
      <PreflightDialog
        host="192.168.1.1"
        port={22}
        username="admin"
        onConnect={() => {}}
        onCancel={onCancel}
      />
    );
    
    screen.getByText('Cancel').click();
    expect(onCancel).toHaveBeenCalled();
  });
});
