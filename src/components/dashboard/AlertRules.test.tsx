import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import AlertRules from './AlertRules';
import '@testing-library/jest-dom';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}));

// Matches the wire format serde actually produces for these fieldless Rust enums:
// a bare string (e.g. "CpuUsage"), not a `{ CpuUsage: null }` object.
const mockBackendRules = [
  {
    id: 'rule-1',
    metric: 'CpuUsage',
    operator: 'GreaterThan',
    threshold: 90,
    severity: 'Critical',
    enabled: true,
    cooldown_seconds: 300,
  },
];

describe('AlertRules', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the Alert Rules heading', () => {
    render(<AlertRules />);
    expect(screen.getByText('Alert Rules')).toBeInTheDocument();
  });

  it('shows empty state when no rules are loaded', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue([]);
    render(<AlertRules />);
    await waitFor(() => {
      expect(screen.getByText(/No alert rules/i)).toBeInTheDocument();
    });
  });

  it('renders existing alert rules fetched from backend', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue(mockBackendRules);
    render(<AlertRules />);
    await waitFor(() => {
      expect(screen.getByText('CPU Usage')).toBeInTheDocument();
    });
  });

  it('shows the add rule form when Add Rule button is clicked', async () => {
    render(<AlertRules />);
    const addButton = screen.getByRole('button', { name: /Add Rule/i });
    fireEvent.click(addButton);
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /Save/i })).toBeInTheDocument();
    });
  });

  it('calls add_alert_rule when form is saved', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue([]);
    render(<AlertRules />);

    fireEvent.click(screen.getByRole('button', { name: /Add Rule/i }));
    await waitFor(() => screen.getByRole('button', { name: /Save/i }));
    fireEvent.click(screen.getByRole('button', { name: /Save/i }));

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('add_alert_rule', expect.anything());
    });
  });

  it('calls remove_alert_rule when delete button is clicked', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue(mockBackendRules);
    render(<AlertRules />);
    await waitFor(() => screen.getByText('CPU Usage'));

    // The delete button (Trash2 icon, no aria-label) is the last button in the rendered rule row.
    const allButtons = screen.getAllByRole('button');
    const deleteButton = allButtons[allButtons.length - 1];
    fireEvent.click(deleteButton);

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('remove_alert_rule', { ruleId: 'rule-1' });
    });
  });
});
