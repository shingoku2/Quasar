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

  // PR #68 review: tasks run over SSH, so inventory-only hosts aren't offered.
  it('offers only SSH hosts for a task', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    const dbHost = { ...mockHost, id: 'host-db', name: 'postgres1', port: 5432, protocol: 'database' };
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_saved_hosts') return Promise.resolve([mockHost, dbHost]);
      return Promise.resolve([]);
    });
    render(<ScheduledTasksView />);
    await waitFor(() => screen.getByText('Add task'));
    fireEvent.click(screen.getByText('Add task'));
    await waitFor(() => screen.getByRole('option', { name: /server1/ }));
    expect(screen.queryByRole('option', { name: /postgres1/ })).not.toBeInTheDocument();
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

  it('shows validation error when saving without host', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_saved_hosts') return Promise.resolve([]);
      if (cmd === 'list_credentials') return Promise.resolve([]);
      return Promise.resolve([]);
    });
    render(<ScheduledTasksView />);

    await waitFor(() => screen.getByText('Add task'));
    fireEvent.click(screen.getByText('Add task'));

    await waitFor(() => screen.getByPlaceholderText('e.g. Daily backup'));
    fireEvent.change(screen.getByPlaceholderText('e.g. Daily backup'), { target: { value: 'Test Task' } });
    fireEvent.change(screen.getByPlaceholderText('0 0 9 * * *'), { target: { value: '0 0 9 * * *' } });

    fireEvent.click(screen.getByText('Add'));

    await waitFor(() => {
      expect(screen.getByText(/Name, schedule, and host are required/i)).toBeInTheDocument();
    });
  });

  it('shows validation error for invalid cron expression', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_saved_hosts') return Promise.resolve([mockHost]);
      if (cmd === 'list_credentials') return Promise.resolve([]);
      return Promise.resolve([]);
    });
    render(<ScheduledTasksView />);

    await waitFor(() => screen.getByText('Add task'));
    fireEvent.click(screen.getByText('Add task'));

    await waitFor(() => screen.getByPlaceholderText('e.g. Daily backup'));
    fireEvent.change(screen.getByPlaceholderText('e.g. Daily backup'), { target: { value: 'Test Task' } });
    fireEvent.change(screen.getByPlaceholderText('0 0 9 * * *'), { target: { value: 'invalid cron' } });

    // Select host so the first validation check passes
    const hostSelects = screen.getAllByRole('combobox');
    fireEvent.change(hostSelects[0], { target: { value: 'host-1' } });

    fireEvent.click(screen.getByText('Add'));

    await waitFor(() => {
      expect(screen.getByText(/Invalid cron expression/i)).toBeInTheDocument();
    });
  });

  // FE-005: steps like */5 are valid cron; the client check used to block "every 5 minutes".
  it('submits a step expression and leaves grammar checks to the backend', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_saved_hosts') return Promise.resolve([mockHost]);
      if (cmd === 'list_credentials') return Promise.resolve([]);
      if (cmd === 'add_scheduled_task') return Promise.resolve('task-new');
      return Promise.resolve([]);
    });
    render(<ScheduledTasksView />);

    await waitFor(() => screen.getByText('Add task'));
    fireEvent.click(screen.getByText('Add task'));
    await waitFor(() => screen.getByPlaceholderText('e.g. Daily backup'));
    fireEvent.change(screen.getByPlaceholderText('e.g. Daily backup'), { target: { value: 'Every 5' } });
    fireEvent.change(screen.getByPlaceholderText('0 0 9 * * *'), { target: { value: '0 */5 * * * *' } });
    fireEvent.change(screen.getByPlaceholderText('e.g. /opt/scripts/backup.sh'), { target: { value: 'uptime' } });
    fireEvent.change(screen.getAllByRole('combobox')[0], { target: { value: 'host-1' } });
    fireEvent.click(screen.getByText('Add'));

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith(
        'add_scheduled_task',
        expect.objectContaining({ cronExpression: '0 */5 * * * *' }),
      );
    });
    expect(screen.queryByText(/Invalid cron expression/i)).not.toBeInTheDocument();
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

describe('ScheduledTasksView flows', () => {
  type Handler = () => unknown;
  const sshKeyCred = { id: 'k1', name: 'Deploy key', username: 'deploy', credential_type: 'ssh_key' };
  const pwCred = { id: 'p1', name: 'Root pw', username: 'root', credential_type: 'ssh' };

  async function mockCommands(handlers: Record<string, Handler>) {
    const { invoke } = await import('@tauri-apps/api/core');
    const defaults: Record<string, Handler> = {
      list_scheduled_tasks: () => [mockTask],
      get_saved_hosts: () => [mockHost],
      list_credentials: () => [sshKeyCred, pwCred],
    };
    const all = { ...defaults, ...handlers };
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      const h = all[cmd];
      if (!h) return Promise.resolve(undefined);
      try {
        return Promise.resolve(h());
      } catch (e) {
        return Promise.reject(e);
      }
    });
    return vi.mocked(invoke);
  }

  // Selects in the form, in DOM order: host, credential, task type.
  const selects = () => screen.getAllByRole('combobox') as HTMLSelectElement[];

  async function openNewForm() {
    render(<ScheduledTasksView />);
    await screen.findByText('Daily Backup');
    fireEvent.click(screen.getByText('Add task'));
    fireEvent.change(screen.getByPlaceholderText('e.g. Daily backup'), { target: { value: '  Nightly  ' } });
  }

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('adds an SSH task with a trimmed lowerCamelCase payload and closes the form', async () => {
    const invoke = await mockCommands({});
    await openNewForm();
    // The first SSH host is preselected.
    expect(selects()[0].value).toBe('host-1');
    fireEvent.change(selects()[1], { target: { value: 'k1' } });
    fireEvent.change(screen.getByPlaceholderText('e.g. /opt/scripts/backup.sh'), { target: { value: ' uptime ' } });
    fireEvent.click(screen.getByText('Add'));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('add_scheduled_task', {
        name: 'Nightly',
        cronExpression: '0 0 9 * * *',
        hostId: 'host-1',
        command: 'uptime',
        credentialId: 'k1',
        enabled: true,
        taskType: 'ssh',
        localPath: null,
        remotePath: null,
      })
    );
    await waitFor(() => expect(screen.queryByText('New task')).not.toBeInTheDocument());
  });

  it('an SSH task needs a command', async () => {
    const invoke = await mockCommands({});
    await openNewForm();
    fireEvent.click(screen.getByText('Add'));
    expect(await screen.findByRole('alert')).toHaveTextContent('Command is required for SSH tasks.');
    expect(invoke).not.toHaveBeenCalledWith('add_scheduled_task', expect.anything());
  });

  it('a file transfer task needs both paths', async () => {
    await mockCommands({});
    await openNewForm();
    fireEvent.change(selects()[2], { target: { value: 'sftp_upload' } });
    fireEvent.change(screen.getByPlaceholderText('/home/user/file.zip'), { target: { value: '/tmp/x' } });
    fireEvent.click(screen.getByText('Add'));
    expect(await screen.findByRole('alert')).toHaveTextContent('Local path and remote path are required');
  });

  it('an upload task picks its local file through the backend dialog', async () => {
    const invoke = await mockCommands({ pick_local_file: () => '/home/me/backup.tar' });
    await openNewForm();
    fireEvent.change(selects()[2], { target: { value: 'sftp_upload' } });
    // The local path is never typed.
    expect(screen.getByLabelText('Local path')).toHaveAttribute('readonly');
    fireEvent.click(screen.getByText('Browse…'));
    await waitFor(() => expect(screen.getByLabelText('Local path')).toHaveValue('/home/me/backup.tar'));
    expect(invoke).toHaveBeenCalledWith('pick_local_file', { title: 'File to upload' });
    fireEvent.change(screen.getByPlaceholderText('/home/user/file.zip'), { target: { value: ' /srv/backup.tar ' } });
    fireEvent.click(screen.getByText('Add'));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        'add_scheduled_task',
        expect.objectContaining({ taskType: 'sftp_upload', command: '', localPath: '/home/me/backup.tar', remotePath: '/srv/backup.tar' })
      )
    );
  });

  it('a download task picks a save location, and a cancelled pick changes nothing', async () => {
    const invoke = await mockCommands({ pick_save_location: () => null });
    await openNewForm();
    fireEvent.change(selects()[2], { target: { value: 'sftp_download' } });
    fireEvent.click(screen.getByText('Browse…'));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('pick_save_location', { defaultName: null }));
    expect(screen.getByLabelText('Local path')).toHaveValue('');
  });

  it('shows an error when the file dialog fails', async () => {
    await mockCommands({
      pick_local_file: () => {
        throw new Error('dialog broke');
      },
    });
    await openNewForm();
    fireEvent.change(selects()[2], { target: { value: 'sftp_upload' } });
    fireEvent.click(screen.getByText('Browse…'));
    expect(await screen.findByRole('alert')).toHaveTextContent('dialog broke');
  });

  it('switching to SFTP drops a key credential and offers only password credentials', async () => {
    await mockCommands({});
    await openNewForm();
    expect(screen.getByRole('option', { name: /Deploy key/ })).toBeInTheDocument();
    fireEvent.change(selects()[1], { target: { value: 'k1' } });
    fireEvent.change(selects()[2], { target: { value: 'sftp_download' } });
    expect(selects()[1].value).toBe('');
    expect(screen.queryByRole('option', { name: /Deploy key/ })).not.toBeInTheDocument();
    expect(screen.getByRole('option', { name: /Root pw/ })).toBeInTheDocument();
  });

  it('switching to SFTP keeps a password credential', async () => {
    await mockCommands({});
    await openNewForm();
    fireEvent.change(selects()[1], { target: { value: 'p1' } });
    fireEvent.change(selects()[2], { target: { value: 'sftp_upload' } });
    expect(selects()[1].value).toBe('p1');
  });

  it('editing a task prefills the form and sends update_scheduled_task with its id', async () => {
    const task = { ...mockTask, credential_id: 'p1', enabled: false };
    const invoke = await mockCommands({ list_scheduled_tasks: () => [task] });
    render(<ScheduledTasksView />);
    await screen.findByText('Daily Backup');
    expect(screen.getByText('Paused')).toBeInTheDocument();
    expect(screen.getByText(/Credential: Root pw/)).toBeInTheDocument();
    fireEvent.click(screen.getByTitle('Edit'));
    expect(screen.getByText('Edit task')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('e.g. Daily backup')).toHaveValue('Daily Backup');
    expect(selects()[1].value).toBe('p1');
    expect(screen.getByRole('checkbox')).not.toBeChecked();
    fireEvent.click(screen.getByRole('checkbox'));
    fireEvent.click(screen.getByText('Update'));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        'update_scheduled_task',
        expect.objectContaining({ id: 'task-1', name: 'Daily Backup', credentialId: 'p1', enabled: true })
      )
    );
    expect(invoke).not.toHaveBeenCalledWith('add_scheduled_task', expect.anything());
  });

  it('editing an SFTP task keeps its type and paths', async () => {
    const task = { ...mockTask, task_type: 'sftp_download', command: '', local_path: '/l', remote_path: '/r' };
    await mockCommands({ list_scheduled_tasks: () => [task] });
    render(<ScheduledTasksView />);
    await screen.findByText('Daily Backup');
    expect(screen.getByText('Download')).toBeInTheDocument();
    expect(screen.getByText('/l → /r')).toBeInTheDocument();
    fireEvent.click(screen.getByTitle('Edit'));
    expect(selects()[2].value).toBe('sftp_download');
    expect(screen.getByLabelText('Local path')).toHaveValue('/l');
    expect(screen.getByPlaceholderText('/home/user/file.zip')).toHaveValue('/r');
  });

  it('shows the backend error when saving fails and keeps the form open', async () => {
    await mockCommands({
      add_scheduled_task: () => {
        throw new Error('Invalid cron: bad field');
      },
    });
    await openNewForm();
    fireEvent.change(screen.getByPlaceholderText('e.g. /opt/scripts/backup.sh'), { target: { value: 'ls' } });
    fireEvent.click(screen.getByText('Add'));
    expect(await screen.findByRole('alert')).toHaveTextContent('Invalid cron: bad field');
    expect(screen.getByText('New task')).toBeInTheDocument();
  });

  it('Cancel closes the form', async () => {
    await mockCommands({});
    await openNewForm();
    fireEvent.click(screen.getByText('Cancel'));
    expect(screen.queryByText('New task')).not.toBeInTheDocument();
  });

  it('deleting asks first and does nothing when declined', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false);
    try {
      const invoke = await mockCommands({});
      render(<ScheduledTasksView />);
      fireEvent.click(await screen.findByTitle('Delete'));
      expect(confirmSpy).toHaveBeenCalledWith('Remove this scheduled task?');
      expect(invoke).not.toHaveBeenCalledWith('remove_scheduled_task', expect.anything());
    } finally {
      confirmSpy.mockRestore();
    }
  });

  it('deleting the task being edited removes it and closes the form', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    try {
      const invoke = await mockCommands({});
      render(<ScheduledTasksView />);
      fireEvent.click(await screen.findByTitle('Edit'));
      expect(screen.getByText('Edit task')).toBeInTheDocument();
      fireEvent.click(screen.getByTitle('Delete'));
      await waitFor(() => expect(invoke).toHaveBeenCalledWith('remove_scheduled_task', { id: 'task-1' }));
      await waitFor(() => expect(screen.queryByText('Edit task')).not.toBeInTheDocument());
    } finally {
      confirmSpy.mockRestore();
    }
  });

  it('shows the backend error when deleting fails', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    try {
      await mockCommands({
        remove_scheduled_task: () => {
          throw new Error('locked');
        },
      });
      render(<ScheduledTasksView />);
      fireEvent.click(await screen.findByTitle('Delete'));
      expect(await screen.findByRole('alert')).toHaveTextContent('locked');
    } finally {
      confirmSpy.mockRestore();
    }
  });

  it('Run now shows a successful result with output, and Dismiss hides it', async () => {
    const invoke = await mockCommands({ run_scheduled_task_now: () => ({ success: true, output: 'up 3 days', error: null }) });
    render(<ScheduledTasksView />);
    fireEvent.click(await screen.findByTitle('Run now'));
    expect(await screen.findByText(/Run result: Daily Backup/)).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith('run_scheduled_task_now', { id: 'task-1' });
    expect(screen.getByText('Success')).toBeInTheDocument();
    expect(screen.getByText('up 3 days')).toBeInTheDocument();
    fireEvent.click(screen.getByLabelText('Dismiss'));
    expect(screen.queryByText(/Run result/)).not.toBeInTheDocument();
  });

  it('Run now shows a failed result with its error', async () => {
    await mockCommands({ run_scheduled_task_now: () => ({ success: false, output: '', error: 'exit 1' }) });
    render(<ScheduledTasksView />);
    fireEvent.click(await screen.findByTitle('Run now'));
    expect(await screen.findByText('Failed')).toBeInTheDocument();
    expect(screen.getByText('exit 1')).toBeInTheDocument();
  });

  it('Run now shows an error when the backend refuses', async () => {
    await mockCommands({
      run_scheduled_task_now: () => {
        throw new Error('already running');
      },
    });
    render(<ScheduledTasksView />);
    fireEvent.click(await screen.findByTitle('Run now'));
    expect(await screen.findByRole('alert')).toHaveTextContent('already running');
    expect(screen.getByTitle('Run now')).not.toBeDisabled();
  });

  it('shows the last run status, error and output of each task', async () => {
    await mockCommands({
      list_scheduled_tasks: () => [
        { ...mockTask, last_run_at: 1700000000, last_run_status: 'failure', last_run_error: 'timeout', last_run_output: 'partial' },
        { ...mockTask, id: 't2', name: 'Other', host_id: 'gone', credential_id: 'missing', last_run_at: 1700000000, last_run_status: 'success' },
      ],
    });
    render(<ScheduledTasksView />);
    expect(await screen.findByText('Failed: timeout')).toBeInTheDocument();
    expect(screen.getByText('Output: partial')).toBeInTheDocument();
    expect(screen.getAllByText(/Last run:/)).toHaveLength(2);
    // Unknown host and credential ids fall back to the raw id.
    expect(screen.getByText(/Host: gone · Credential: missing/)).toBeInTheDocument();
  });

  // FE-021: every form field has an accessible name from its visible label.
  it('labels every task form field', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue([]);
    render(<ScheduledTasksView />);
    await waitFor(() => screen.getByText('Add task'));
    fireEvent.click(screen.getByText('Add task'));
    for (const label of ['Name', /Cron schedule/, 'Host', /Credential/, 'Task type', 'Command']) {
      expect(screen.getByLabelText(label)).toBeInTheDocument();
    }
  });
});
