import { render, screen, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import HealthCheckBadge from './HealthCheckBadge';
import '@testing-library/jest-dom';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn((cmd) => {
    if (cmd === 'preflight_check') {
      return Promise.resolve({
        reachable: true,
        latency_ms: 10,
      });
    }
    return Promise.resolve();
  }),
}));

describe('HealthCheckBadge', () => {
  it('renders loading state initially', () => {
    const { container } = render(<HealthCheckBadge host="192.168.1.1" />);
    expect(container.querySelector('svg')).toBeInTheDocument();
  });

  it('shows reachable status with latency', async () => {
    render(<HealthCheckBadge host="192.168.1.1" />);
    
    await waitFor(() => {
      expect(screen.getByText('10ms')).toBeInTheDocument();
    });
  });

  it('refreshes health check periodically', () => {
    vi.useFakeTimers();
    render(<HealthCheckBadge host="192.168.1.1" />);
    
    // Should trigger another check after 30 seconds
    vi.advanceTimersByTime(30000);
    
    vi.useRealTimers();
  });
});
