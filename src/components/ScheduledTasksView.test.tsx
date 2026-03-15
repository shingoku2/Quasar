import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import ScheduledTasksView from './ScheduledTasksView';
import '@testing-library/jest-dom';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}));

const mockTask = {
  id: 'task-1',
  name: 'Daily Backup',
  cron_expression: '0 0 9 * * *',
  host_id: 'host-1',
  command: 'backup.sh',
  credential_id: null,
  enabled: true,
  task_type: 'ssh',
  local_path: null,
  remote_path: null,
  last_run_at: null,
  last_run_status: null,
  last_run_error: null,
  last_run_output: null,
  created_at: 1700000000,
  updated_at: 1700000000,
};

const mockHost = { id: 'host-1', name: 'server1', address: '10.0.0.1', port: 22, username: 'admin', protocol: 'ssh', credential_id: null };

describe('ScheduledTasksView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the heading', async () => {
    render(<ScheduledTasksView />);
    expect(screen.getByText('Scheduled Tasks')).toBeInTheDocument();
  });

  it('shows loading state initially', () => {
    render(<ScheduledTasksView />);
    expect(screen.getByText(/Loading tasks/i)).toBeInTheDocument();
  });

  it('renders tasks returned by backend', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'list_scheduled_tasks') return Promise.resolve([mockTask]);
      if (cmd === 'get_saved_hosts') return Promise.resolve([mockHost]);
      if (cmd === 'list_credentials') return Promise.resolve([]);
      return Promise.resolve([]);
    });
    render(<ScheduledTasksView />);
    await waitFor(() => {
      expect(screen.getByText('Daily Backup')).toBeInTheDocument();
    });
  });

  it('shows "New task" form when Add task button is clicked', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue([]);
    render(<ScheduledTasksView />);
    await waitFor(() => screen.getByText('Add task'));
    fireEvent.click(screen.getByText('Add task'));
    await waitFor(() => {
      expect(screen.getByPlaceholderText('e.g. Daily backup')).toBeInTheDocument();
    });
  });

  it('shows validation error when saving without name', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue([]);
    render(<ScheduledTasksView />);
    await waitFor(() => screen.getByText('Add task'));
    fireEvent.click(screen.getByText('Add task'));
    await waitFor(() => screen.getByText('Add'));
    fireEvent.click(screen.getByText('Add'));
    await waitFor(() => {
      expect(screen.getByText(/Name, schedule, and host are required/i)).toBeInTheDocument();
    });
  });

  it('shows error state when backend call fails', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockRejectedValue(new Error('Network error'));
    render(<ScheduledTasksView />);
    await waitFor(() => {
      expect(screen.getByText(/Network error/i)).toBeInTheDocument();
    });
  });

  it('shows task cron expression in the list', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'list_scheduled_tasks') return Promise.resolve([mockTask]);
      if (cmd === 'get_saved_hosts') return Promise.resolve([mockHost]);
      return Promise.resolve([]);
    });
    render(<ScheduledTasksView />);
    await waitFor(() => {
      expect(screen.getByText('0 0 9 * * *')).toBeInTheDocument();
    });
  });
});
