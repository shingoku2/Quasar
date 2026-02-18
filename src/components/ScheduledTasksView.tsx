import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Clock, Plus, Pencil, Trash2 } from 'lucide-react';

export interface ScheduledTask {
  id: string;
  name: string;
  cron_expression: string;
  host_id: string;
  command: string;
  credential_id: string | null;
  enabled: boolean;
  last_run_at: number | null;
  created_at: number;
  updated_at: number;
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

const DEFAULT_CRON = '0 9 * * *'; // 9:00 daily (5-field: min hour day month dow)

const ScheduledTasksView: React.FC = () => {
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [hosts, setHosts] = useState<SavedHost[]>([]);
  const [credentials, setCredentials] = useState<CredentialSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState({
    name: '',
    cron_expression: DEFAULT_CRON,
    host_id: '',
    command: '',
    credential_id: '' as string | null,
    enabled: true,
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
    });
    setEditingId(null);
    setShowForm(false);
  };

  const handleSave = async () => {
    if (!form.name.trim() || !form.cron_expression.trim() || !form.host_id || !form.command.trim()) {
      setError('Name, schedule, host, and command are required.');
      return;
    }
    setError(null);
    try {
      if (editingId) {
        await invoke('update_scheduled_task', {
          id: editingId,
          name: form.name.trim(),
          cron_expression: form.cron_expression.trim(),
          host_id: form.host_id,
          command: form.command.trim(),
          credential_id: form.credential_id || null,
          enabled: form.enabled,
        });
      } else {
        await invoke('add_scheduled_task', {
          name: form.name.trim(),
          cron_expression: form.cron_expression.trim(),
          host_id: form.host_id,
          command: form.command.trim(),
          credential_id: form.credential_id || null,
          enabled: form.enabled,
        });
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

  const startEdit = (task: ScheduledTask) => {
    setForm({
      name: task.name,
      cron_expression: task.cron_expression,
      host_id: task.host_id,
      command: task.command,
      credential_id: task.credential_id || null,
      enabled: task.enabled,
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
          <div className="mb-4 p-3 rounded-lg bg-alert/10 text-alert text-sm">{error}</div>
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
                <label className="block text-xs text-gray-500 mb-1">Cron schedule (min hour day month dow)</label>
                <input
                  value={form.cron_expression}
                  onChange={(e) => setForm((f) => ({ ...f, cron_expression: e.target.value }))}
                  className="w-full bg-bg-sidebar border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-accent font-mono"
                  placeholder="0 9 * * *"
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
                  {credentials.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.name} ({c.username})
                    </option>
                  ))}
                </select>
              </div>
            </div>
            <div className="mt-4">
              <label className="block text-xs text-gray-500 mb-1">Command</label>
              <input
                value={form.command}
                onChange={(e) => setForm((f) => ({ ...f, command: e.target.value }))}
                className="w-full bg-bg-sidebar border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-accent font-mono"
                placeholder="e.g. /opt/scripts/backup.sh"
              />
            </div>
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
                  </div>
                  <div className="text-sm text-gray-500 mt-1 truncate font-mono" title={task.command}>
                    {task.command}
                  </div>
                  {task.last_run_at != null && (
                    <div className="text-xs text-gray-500 mt-1">Last run: {formatTs(task.last_run_at)}</div>
                  )}
                </div>
                <div className="flex items-center gap-2 shrink-0">
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
