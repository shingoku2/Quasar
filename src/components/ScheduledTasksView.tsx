import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Clock, Plus, Pencil, Trash2, Play } from 'lucide-react';

export interface ScheduledTask {
  id: string;
  name: string;
  cron_expression: string;
  host_id: string;
  command: string;
  credential_id: string | null;
  enabled: boolean;
  task_type: string;
  local_path: string | null;
  remote_path: string | null;
  last_run_at: number | null;
  last_run_status: string | null;
  last_run_error: string | null;
  last_run_output: string | null;
  created_at: number;
  updated_at: number;
}

export interface TaskRunResult {
  success: boolean;
  output: string | null;
  error: string | null;
}

interface SavedHost {
  id: string;
  name: string;
  address: string;
  port: number;
  username?: string | null;
  protocol: string;
  credential_id?: string | null;
}

interface CredentialSummary {
  id: string;
  name: string;
  username: string;
  credential_type: string;
}


/** Validate a 6-field cron expression: sec min hour day month dow */
function isValidCronExpression(expr: string): boolean {
  const parts = expr.trim().split(/\s+/);
  if (parts.length !== 6) return false;
  // Very basic field range check (not exhaustive, full validation is done backend-side)
  const ranges = [
    [0, 59],  // seconds
    [0, 59],  // minutes
    [0, 23],  // hours
    [1, 31],  // day of month
    [1, 12],  // month
    [0, 7],   // day of week
  ];
  return parts.every((part, i) => {
    if (part === '*') return true;
    const n = parseInt(part, 10);
    return Number.isInteger(n) && n >= ranges[i][0] && n <= ranges[i][1];
  });
}

const DEFAULT_CRON = '0 0 9 * * *'; // 9:00 daily (6-field: sec min hour day month dow)

const ScheduledTasksView: React.FC = () => {
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [hosts, setHosts] = useState<SavedHost[]>([]);
  const [credentials, setCredentials] = useState<CredentialSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [runningId, setRunningId] = useState<string | null>(null);
  const [lastRunResult, setLastRunResult] = useState<{ taskName: string; success: boolean; output: string | null; error: string | null } | null>(null);
  const [form, setForm] = useState({
    name: '',
    cron_expression: DEFAULT_CRON,
    host_id: '',
    command: '',
    credential_id: '' as string | null,
    enabled: true,
    task_type: 'ssh' as 'ssh' | 'sftp_upload' | 'sftp_download',
    local_path: '',
    remote_path: '',
  });

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [taskList, hostList, credList] = await Promise.all([
        invoke<ScheduledTask[]>('list_scheduled_tasks'),
        invoke<SavedHost[]>('get_saved_hosts'),
        invoke<CredentialSummary[]>('list_credentials'),
      ]);
      setTasks(Array.isArray(taskList) ? taskList : []);
      setHosts(Array.isArray(hostList) ? hostList : []);
      setCredentials(Array.isArray(credList) ? credList : []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setTasks([]);
      setHosts([]);
      setCredentials([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const resetForm = () => {
    setForm({
      name: '',
      cron_expression: DEFAULT_CRON,
      host_id: hosts[0]?.id ?? '',
      command: '',
      credential_id: null,
      enabled: true,
      task_type: 'ssh',
      local_path: '',
      remote_path: '',
    });
    setEditingId(null);
    setShowForm(false);
  };

  const handleSave = async () => {
    if (!form.name.trim() || !form.cron_expression.trim() || !form.host_id) {
      setError('Name, schedule, and host are required.');
      return;
    }
    if (!isValidCronExpression(form.cron_expression)) {
      setError('Invalid cron expression. Use 6-field format: sec min hour day month dow (e.g. 0 0 9 * * *)');
      return;
    }
    if (!form.host_id) {
      setError('Please select a host for this task.');
      return;
    }
    if (form.task_type === 'ssh') {
      if (!form.command.trim()) {
        setError('Command is required for SSH tasks.');
        return;
      }
    } else {
      if (!form.local_path.trim() || !form.remote_path.trim()) {
        setError('Local path and remote path are required for file transfer tasks.');
        return;
      }
    }
    setError(null);
    const payload = {
      name: form.name.trim(),
      cronExpression: form.cron_expression.trim(),
      hostId: form.host_id,
      command: form.task_type === 'ssh' ? form.command.trim() : '',
      credentialId: form.credential_id || null,
      enabled: form.enabled,
      taskType: form.task_type,
      localPath: form.task_type !== 'ssh' ? form.local_path.trim() || null : null,
      remotePath: form.task_type !== 'ssh' ? form.remote_path.trim() || null : null,
    };
    try {
      if (editingId) {
        await invoke('update_scheduled_task', { id: editingId, ...payload });
      } else {
        await invoke('add_scheduled_task', payload);
      }
      await load();
      resetForm();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm('Remove this scheduled task?')) return;
    setError(null);
    try {
      await invoke('remove_scheduled_task', { id });
      await load();
      if (editingId === id) resetForm();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleRunNow = async (task: ScheduledTask) => {
    setError(null);
    setLastRunResult(null);
    setRunningId(task.id);
    try {
      const result = await invoke<TaskRunResult>('run_scheduled_task_now', { id: task.id });
      setLastRunResult({
        taskName: task.name,
        success: result.success,
        output: result.output ?? null,
        error: result.error ?? null,
      });
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRunningId(null);
    }
  };

  const startEdit = (task: ScheduledTask) => {
    setForm({
      name: task.name,
      cron_expression: task.cron_expression,
      host_id: task.host_id,
      command: task.command,
      credential_id: task.credential_id || null,
      enabled: task.enabled,
      task_type: (task.task_type === 'sftp_upload' || task.task_type === 'sftp_download' ? task.task_type : 'ssh') as 'ssh' | 'sftp_upload' | 'sftp_download',
      local_path: task.local_path ?? '',
      remote_path: task.remote_path ?? '',
    });
    setEditingId(task.id);
    setShowForm(true);
  };

  const hostName = (hostId: string) => hosts.find((h) => h.id === hostId)?.name ?? hostId;
  const credName = (credId: string | null) =>
    credId ? credentials.find((c) => c.id === credId)?.name ?? credId : '—';

  const formatTs = (ts: number | null) =>
    ts ? new Date(ts * 1000).toLocaleString() : '—';

  return (
    <div className="h-full overflow-auto p-6">
      <div className="max-w-4xl mx-auto">
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-xl font-semibold text-white">Scheduled Tasks</h2>
          <button
            type="button"
            onClick={() => {
              setForm({
                name: '',
                cron_expression: DEFAULT_CRON,
                host_id: hosts[0]?.id ?? '',
                command: '',
                credential_id: null,
                enabled: true,
                task_type: 'ssh',
                local_path: '',
                remote_path: '',
              });
              setEditingId(null);
              setShowForm(true);
            }}
            className="flex items-center gap-2 px-4 py-2 bg-accent text-white rounded-lg hover:bg-accent/90 transition-colors text-sm font-medium"
          >
            <Plus className="h-4 w-4" />
            Add task
          </button>
        </div>

        {error && (
          <div className="mb-4 p-3 rounded-lg bg-alert/10 text-alert text-sm" role="alert" aria-live="assertive">{error}</div>
        )}

        {lastRunResult && (
          <div className="mb-4 p-4 rounded-xl bg-bg-card border border-border">
            <div className="flex items-start justify-between gap-2">
              <div className="min-w-0 flex-1">
                <div className="font-medium text-white">
                  Run result: {lastRunResult.taskName}
                  {lastRunResult.success ? (
                    <span className="ml-2 text-success">Success</span>
                  ) : (
                    <span className="ml-2 text-alert">Failed</span>
                  )}
                </div>
                {lastRunResult.error && (
                  <pre className="mt-2 text-sm text-alert whitespace-pre-wrap break-words font-mono">{lastRunResult.error}</pre>
                )}
                {lastRunResult.output != null && lastRunResult.output.length > 0 && (
                  <pre className="mt-2 text-sm text-gray-400 whitespace-pre-wrap break-words font-mono max-h-40 overflow-y-auto">{lastRunResult.output}</pre>
                )}
              </div>
              <button
                type="button"
                onClick={() => setLastRunResult(null)}
                className="shrink-0 text-gray-500 hover:text-white transition-colors"
                aria-label="Dismiss"
              >
                ×
              </button>
            </div>
          </div>
        )}

        {showForm && (
          <div className="mb-6 p-4 rounded-xl bg-bg-card border border-border">
            <h3 className="text-sm font-medium text-gray-300 mb-4">
              {editingId ? 'Edit task' : 'New task'}
            </h3>
            <div className="grid gap-4 sm:grid-cols-2">
              <div>
                <label className="block text-xs text-gray-500 mb-1">Name</label>
                <input
                  value={form.name}
                  onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
                  className="w-full bg-bg-sidebar border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-accent"
                  placeholder="e.g. Daily backup"
                />
              </div>
              <div>
                <label className="block text-xs text-gray-500 mb-1">Cron schedule (sec min hour day month dow)</label>
                <input
                  value={form.cron_expression}
                  onChange={(e) => setForm((f) => ({ ...f, cron_expression: e.target.value }))}
                  className="w-full bg-bg-sidebar border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-accent font-mono"
                  placeholder="0 0 9 * * *"
                />
              </div>
              <div>
                <label className="block text-xs text-gray-500 mb-1">Host</label>
                <select
                  value={form.host_id}
                  onChange={(e) => setForm((f) => ({ ...f, host_id: e.target.value }))}
                  className="w-full bg-bg-sidebar border border-border rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-accent"
                >
                  <option value="">Select host</option>
                  {hosts.map((h) => (
                    <option key={h.id} value={h.id}>
                      {h.name} ({h.address}:{h.port})
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label className="block text-xs text-gray-500 mb-1">Credential (optional)</label>
                <select
                  value={form.credential_id ?? ''}
                  onChange={(e) =>
                    setForm((f) => ({ ...f, credential_id: e.target.value || null }))
                  }
                  className="w-full bg-bg-sidebar border border-border rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-accent"
                >
                  <option value="">None</option>
                  {credentials
                    .filter((c) =>
                      form.task_type === 'sftp_upload' || form.task_type === 'sftp_download'
                        ? c.credential_type !== 'ssh_key'
                        : true
                    )
                    .map((c) => (
                      <option key={c.id} value={c.id}>
                        {c.name} ({c.username})
                      </option>
                    ))}
                </select>
              </div>
              <div>
                <label className="block text-xs text-gray-500 mb-1">Task type</label>
                <select
                  value={form.task_type}
                  onChange={(e) => {
                    const newType = e.target.value as 'ssh' | 'sftp_upload' | 'sftp_download';
                    setForm((f) => {
                      const isSftp = newType === 'sftp_upload' || newType === 'sftp_download';
                      const selectedCred = credentials.find((c) => c.id === f.credential_id);
                      const clearCred = isSftp && selectedCred?.credential_type === 'ssh_key';
                      return {
                        ...f,
                        task_type: newType,
                        credential_id: clearCred ? null : f.credential_id,
                      };
                    });
                  }}
                  className="w-full bg-bg-sidebar border border-border rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-accent"
                >
                  <option value="ssh">SSH command</option>
                  <option value="sftp_upload">Upload file (SFTP)</option>
                  <option value="sftp_download">Download file (SFTP)</option>
                </select>
              </div>
            </div>
            {form.task_type === 'ssh' && (
              <div className="mt-4">
                <label className="block text-xs text-gray-500 mb-1">Command</label>
                <input
                  value={form.command}
                  onChange={(e) => setForm((f) => ({ ...f, command: e.target.value }))}
                  className="w-full bg-bg-sidebar border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-accent font-mono"
                  placeholder="e.g. /opt/scripts/backup.sh"
                />
              </div>
            )}
            {(form.task_type === 'sftp_upload' || form.task_type === 'sftp_download') && (
              <div className="mt-4 grid gap-4 sm:grid-cols-2">
                <div>
                  <label className="block text-xs text-gray-500 mb-1">Local path</label>
                  <input
                    value={form.local_path}
                    onChange={(e) => setForm((f) => ({ ...f, local_path: e.target.value }))}
                    className="w-full bg-bg-sidebar border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-accent font-mono"
                    placeholder={form.task_type === 'sftp_upload' ? 'C:\\backup\\file.zip' : 'C:\\downloads\\file.zip'}
                  />
                </div>
                <div>
                  <label className="block text-xs text-gray-500 mb-1">Remote path</label>
                  <input
                    value={form.remote_path}
                    onChange={(e) => setForm((f) => ({ ...f, remote_path: e.target.value }))}
                    className="w-full bg-bg-sidebar border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-accent font-mono"
                    placeholder="/home/user/file.zip"
                  />
                </div>
              </div>
            )}
            <div className="mt-4 flex items-center gap-4">
              <label className="flex items-center gap-2 text-sm text-gray-400 cursor-pointer">
                <input
                  type="checkbox"
                  checked={form.enabled}
                  onChange={(e) => setForm((f) => ({ ...f, enabled: e.target.checked }))}
                  className="rounded border-border bg-bg-sidebar text-accent focus:ring-accent"
                />
                Enabled
              </label>
            </div>
            <div className="mt-4 flex gap-2">
              <button
                type="button"
                onClick={handleSave}
                className="px-4 py-2 bg-accent text-white rounded-lg hover:bg-accent/90 text-sm font-medium"
              >
                {editingId ? 'Update' : 'Add'}
              </button>
              <button
                type="button"
                onClick={resetForm}
                className="px-4 py-2 bg-bg-sidebar border border-border text-gray-300 rounded-lg hover:bg-white/5 text-sm"
              >
                Cancel
              </button>
            </div>
          </div>
        )}

        {loading ? (
          <p className="text-gray-500">Loading tasks…</p>
        ) : tasks.length === 0 ? (
          <div className="rounded-xl bg-bg-card border border-border p-8 text-center text-gray-500">
            <Clock className="h-12 w-12 mx-auto mb-3 opacity-50" />
            <p>No scheduled tasks yet.</p>
            <p className="text-sm mt-1">Add a task to run SSH commands on a schedule (cron).</p>
          </div>
        ) : (
          <ul className="space-y-3">
            {tasks.map((task) => (
              <li
                key={task.id}
                className="flex items-center gap-4 p-4 rounded-xl bg-bg-card border border-border"
              >
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="font-medium text-white">{task.name}</span>
                    {!task.enabled && (
                      <span className="text-xs px-2 py-0.5 rounded bg-gray-600 text-gray-300">
                        Paused
                      </span>
                    )}
                  </div>
                  <div className="text-sm text-gray-500 mt-1 font-mono">{task.cron_expression}</div>
                  <div className="text-sm text-gray-400 mt-1">
                    Host: {hostName(task.host_id)} · Credential: {credName(task.credential_id)}
                    {(task.task_type === 'sftp_upload' || task.task_type === 'sftp_download') && (
                      <span className="ml-2 text-accent">
                        {task.task_type === 'sftp_upload' ? 'Upload' : 'Download'}
                      </span>
                    )}
                  </div>
                  {(task.task_type === 'sftp_upload' || task.task_type === 'sftp_download') ? (
                    <div className="text-sm text-gray-500 mt-1 truncate font-mono" title={`${task.local_path ?? ''} → ${task.remote_path ?? ''}`}>
                      {task.local_path ?? '—'} → {task.remote_path ?? '—'}
                    </div>
                  ) : (
                    <div className="text-sm text-gray-500 mt-1 truncate font-mono" title={task.command}>
                      {task.command}
                    </div>
                  )}
                  {task.last_run_at != null && (
                    <div className="text-xs mt-1 flex flex-wrap items-center gap-x-3 gap-y-0.5">
                      <span className="text-gray-500">Last run: {formatTs(task.last_run_at)}</span>
                      {task.last_run_status === 'success' && (
                        <span className="text-success font-medium">Success</span>
                      )}
                      {task.last_run_status === 'failure' && (
                        <span className="text-alert font-medium" title={task.last_run_error ?? undefined}>
                          Failed{task.last_run_error ? `: ${task.last_run_error}` : ''}
                        </span>
                      )}
                      {task.last_run_output != null && task.last_run_output.length > 0 && (
                        <span className="text-gray-500 truncate max-w-xs" title={task.last_run_output}>
                          Output: {task.last_run_output}
                        </span>
                      )}
                    </div>
                  )}
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  <button
                    type="button"
                    onClick={() => handleRunNow(task)}
                    disabled={runningId !== null}
                    className="p-2 rounded-lg text-gray-400 hover:bg-white/10 hover:text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    title="Run now"
                  >
                    <Play className="h-4 w-4" />
                  </button>
                  <button
                    type="button"
                    onClick={() => startEdit(task)}
                    className="p-2 rounded-lg text-gray-400 hover:bg-white/10 hover:text-white transition-colors"
                    title="Edit"
                  >
                    <Pencil className="h-4 w-4" />
                  </button>
                  <button
                    type="button"
                    onClick={() => handleDelete(task.id)}
                    className="p-2 rounded-lg text-gray-400 hover:bg-alert/10 hover:text-alert transition-colors"
                    title="Delete"
                  >
                    <Trash2 className="h-4 w-4" />
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
};

export default ScheduledTasksView;
