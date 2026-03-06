import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import AlertFeed from './AlertFeed';
import type { Alert } from './AlertFeed';
import '@testing-library/jest-dom';

const mockInvoke = vi.mocked(invoke);

const makeAlert = (overrides: Partial<Alert> = {}): Alert => ({
  id: '1',
  source: 'System',
  message: 'CPU usage is high',
  severity: 'warning',
  timestamp: '12:00:00',
  acknowledged: false,
  ...overrides,
});

describe('AlertFeed', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Default: get_system_metrics returns null / rejects silently
    mockInvoke.mockRejectedValue(new Error('no metrics'));
  });

  it('shows "All systems nominal" when there are no alerts', () => {
    render(<AlertFeed alerts={[]} />);
    expect(screen.getByText(/All systems nominal/i)).toBeInTheDocument();
  });

  it('renders alert messages', () => {
    const alerts = [makeAlert({ id: '1', message: 'High CPU usage detected' })];
    render(<AlertFeed alerts={alerts} />);
    expect(screen.getByText('High CPU usage detected')).toBeInTheDocument();
  });

  it('renders multiple alerts', () => {
    const alerts = [
      makeAlert({ id: '1', message: 'Alert one', source: 'CPU Monitor' }),
      makeAlert({ id: '2', message: 'Alert two', source: 'Memory Monitor', severity: 'critical' }),
    ];
    render(<AlertFeed alerts={alerts} />);
    expect(screen.getByText('Alert one')).toBeInTheDocument();
    expect(screen.getByText('Alert two')).toBeInTheDocument();
  });

  it('shows unacknowledged count in footer', () => {
    const alerts = [
      makeAlert({ id: '1', acknowledged: false }),
      makeAlert({ id: '2', acknowledged: true }),
    ];
    render(<AlertFeed alerts={alerts} />);
    expect(screen.getByText(/1 unacknowledged/)).toBeInTheDocument();
  });

  it('removes an alert when dismissed', () => {
    const alerts = [makeAlert({ id: '1', message: 'Dismissable alert' })];
    render(<AlertFeed alerts={alerts} />);

    const dismissBtn = screen.getByTitle('Dismiss');
    fireEvent.click(dismissBtn);

    expect(screen.queryByText('Dismissable alert')).not.toBeInTheDocument();
    expect(screen.getByText(/All systems nominal/i)).toBeInTheDocument();
  });

  it('clears all alerts when Clear All is clicked', () => {
    const alerts = [
      makeAlert({ id: '1', message: 'Alert one' }),
      makeAlert({ id: '2', message: 'Alert two' }),
    ];
    render(<AlertFeed alerts={alerts} />);

    fireEvent.click(screen.getByRole('button', { name: /Clear All/i }));

    expect(screen.queryByText('Alert one')).not.toBeInTheDocument();
    expect(screen.getByText(/All systems nominal/i)).toBeInTheDocument();
  });

  it('marks an alert as acknowledged when Acknowledge is clicked', () => {
    const alerts = [makeAlert({ id: '1', message: 'Ack this alert', acknowledged: false })];
    render(<AlertFeed alerts={alerts} />);

    fireEvent.click(screen.getByTitle('Acknowledge'));

    // After acknowledgement the count drops to 0
    expect(screen.getByText(/0 unacknowledged/)).toBeInTheDocument();
  });

  it('shows metrics summary when system metrics are available', async () => {
    mockInvoke.mockResolvedValueOnce({
      cpu_usage_percent: 45.5,
      memory_usage_percent: 60,
      disk_read_mb: 1,
      disk_write_mb: 2,
    } as any);

    render(<AlertFeed />);

    await waitFor(() => {
      expect(screen.getByText(/45\.5%/)).toBeInTheDocument();
    });
  });

  it('shows LIVE badge', () => {
    render(<AlertFeed />);
    expect(screen.getByText('LIVE')).toBeInTheDocument();
  });

  it('shows Recent Activity heading', () => {
    render(<AlertFeed />);
    expect(screen.getByText('Recent Activity')).toBeInTheDocument();
  });
});
