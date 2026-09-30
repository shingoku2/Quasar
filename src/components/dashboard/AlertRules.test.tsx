import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import AlertRules from './AlertRules';
import { ViewVisibilityProvider } from '../../hooks/useViewVisibility';
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

    const deleteButtons = screen.getAllByRole('button', { name: /^Delete alert rule:/ });
    fireEvent.click(deleteButtons[deleteButtons.length - 1]);

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('remove_alert_rule', { ruleId: 'rule-1' });
    });
  });

  // RUST-002: the backend validates rules; a rejected rule is removed and the reason shown.
  it('shows the backend validation error when a rule is rejected', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === 'add_alert_rule') throw 'Alert threshold must be a percentage between 0 and 100';
      return [];
    });
    render(<AlertRules />);

    fireEvent.click(screen.getByRole('button', { name: /Add Rule/i }));
    await waitFor(() => screen.getByRole('button', { name: /Save/i }));
    fireEvent.click(screen.getByRole('button', { name: /Save/i }));

    expect(await screen.findByRole('alert')).toHaveTextContent('percentage between 0 and 100');
  });

  // PR #68 review: the Monitoring view stays mounted while hidden, and an import can replace
  // the rules meanwhile, so the editor reloads them each time it's shown.
  it('reloads the rules when its view is shown again', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    const { rerender } = render(
      <ViewVisibilityProvider visible={true}><AlertRules /></ViewVisibilityProvider>
    );
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('get_alert_rules'));
    const loads = () => vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === 'get_alert_rules').length;
    const before = loads();
    rerender(<ViewVisibilityProvider visible={false}><AlertRules /></ViewVisibilityProvider>);
    rerender(<ViewVisibilityProvider visible={true}><AlertRules /></ViewVisibilityProvider>);
    await waitFor(() => expect(loads()).toBe(before + 1));
  });

  it('toggles rule enabled state with add_alert_rule and reverts on error', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    let shouldFail = false;
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === 'get_alert_rules') return mockBackendRules;
      if (cmd === 'add_alert_rule') {
        if (shouldFail) throw new Error('Database locked');
        return undefined;
      }
      return [];
    });

    render(<AlertRules />);
    await waitFor(() => expect(screen.getByText('CPU Usage')).toBeInTheDocument());

    const toggleBtn = screen.getByRole('switch');
    expect(toggleBtn).toHaveAttribute('aria-checked', 'true');

    // Toggle off succeeds
    fireEvent.click(toggleBtn);
    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('add_alert_rule', {
        rule: expect.objectContaining({ id: 'rule-1', enabled: false }),
      });
    });
    expect(toggleBtn).toHaveAttribute('aria-checked', 'false');

    // Toggle back on with error -> should revert to false and display error
    shouldFail = true;
    fireEvent.click(toggleBtn);
    expect(await screen.findByRole('alert')).toHaveTextContent('Database locked');
    expect(toggleBtn).toHaveAttribute('aria-checked', 'false');
  });

  it('reloads rules when removing an alert rule fails', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    let getCallCount = 0;
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === 'get_alert_rules') {
        getCallCount++;
        return mockBackendRules;
      }
      if (cmd === 'remove_alert_rule') {
        throw new Error('Foreign key violation');
      }
      return [];
    });

    render(<AlertRules />);
    await waitFor(() => expect(screen.getByText('CPU Usage')).toBeInTheDocument());

    const deleteBtn = screen.getByRole('button', { name: /^Delete alert rule:/ });
    fireEvent.click(deleteBtn);

    expect(await screen.findByRole('alert')).toHaveTextContent('Foreign key violation');
    await waitFor(() => expect(getCallCount).toBeGreaterThan(1));
  });

  it('configures new rule form selects and inputs and saves', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue([]);
    render(<AlertRules />);

    fireEvent.click(screen.getByRole('button', { name: /Add Rule/i }));

    const comboboxes = screen.getAllByRole('combobox');
    const metricSelect = comboboxes[0];
    const operatorSelect = comboboxes[1];
    const severitySelect = comboboxes[2];
    const thresholdInput = screen.getByPlaceholderText('Threshold %');

    fireEvent.change(metricSelect, { target: { value: 'MemoryUsage' } });
    fireEvent.change(operatorSelect, { target: { value: 'LessThan' } });
    fireEvent.change(thresholdInput, { target: { value: '75' } });
    fireEvent.change(severitySelect, { target: { value: 'Critical' } });

    fireEvent.click(screen.getByRole('button', { name: /Save/i }));

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('add_alert_rule', {
        rule: expect.objectContaining({
          metric: 'MemoryUsage',
          operator: 'LessThan',
          threshold: 75,
          severity: 'Critical',
          enabled: true,
          cooldown_seconds: 300,
        }),
      });
    });

    expect(screen.queryByRole('button', { name: /Save/i })).not.toBeInTheDocument();
  });

  it('cancels the add rule form when Cancel is clicked', async () => {
    render(<AlertRules />);
    fireEvent.click(screen.getByRole('button', { name: /Add Rule/i }));
    expect(screen.getByRole('button', { name: /Save/i })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /Cancel/i }));
    expect(screen.queryByRole('button', { name: /Save/i })).not.toBeInTheDocument();
  });
});
