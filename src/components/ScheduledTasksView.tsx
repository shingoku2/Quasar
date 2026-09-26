import React, { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Clock, Plus } from 'lucide-react';
import { getErrorMessage } from '../lib/utils';
import { useOnViewShown } from '../hooks/useViewVisibility';
import RunResultBanner from './scheduled/RunResultBanner';
import type { RunResult } from './scheduled/RunResultBanner';
import ScheduledTaskForm from './scheduled/ScheduledTaskForm';
import ScheduledTaskList from './scheduled/ScheduledTaskList';
import { buildTaskPayload, emptyTaskForm, formFromTask, taskHosts, validateTaskForm } from './scheduled/taskForm';
import type { ScheduledTask, TaskCredential, TaskForm, TaskHost, TaskRunResult } from './scheduled/taskForm';

const ScheduledTasksView: React.FC = () => {
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [allHosts, setHosts] = useState<TaskHost[]>([]);
  const hosts = taskHosts(allHosts);
  const [credentials, setCredentials] = useState<TaskCredential[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [runningId, setRunningId] = useState<string | null>(null);
  const [lastRunResult, setLastRunResult] = useState<RunResult | null>(null);
  const [form, setForm] = useState<TaskForm>(emptyTaskForm);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [taskList, hostList, credList] = await Promise.all([
        invoke<ScheduledTask[]>('list_scheduled_tasks'),
        invoke<TaskHost[]>('get_saved_hosts'),
        invoke<TaskCredential[]>('list_credentials'),
      ]);
      setTasks(Array.isArray(taskList) ? taskList : []);
      setHosts(Array.isArray(hostList) ? hostList : []);
      setCredentials(Array.isArray(credList) ? credList : []);
    } catch (e) {
      setError(getErrorMessage(e));
      setTasks([]);
      setHosts([]);
      setCredentials([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useOnViewShown(load);

  const resetForm = () => {
    setForm(emptyTaskForm(hosts[0]?.id));
    setEditingId(null);
    setShowForm(false);
  };

  const handleSave = async () => {
    const problem = validateTaskForm(form);
    if (problem) {
      setError(problem);
      return;
    }
    setError(null);
    const payload = buildTaskPayload(form);
    try {
      if (editingId) {
        await invoke('update_scheduled_task', { id: editingId, ...payload });
      } else {
        await invoke('add_scheduled_task', payload);
      }
      await load();
      resetForm();
    } catch (e) {
      setError(getErrorMessage(e));
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
      setError(getErrorMessage(e));
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
      setError(getErrorMessage(e));
    } finally {
      setRunningId(null);
    }
  };

  const startEdit = (task: ScheduledTask) => {
    setForm(formFromTask(task));
    setEditingId(task.id);
    setShowForm(true);
  };

  const hostName = (hostId: string) => allHosts.find((h) => h.id === hostId)?.name ?? hostId;
  const credName = (credId: string | null) =>
    credId ? credentials.find((c) => c.id === credId)?.name ?? credId : '—';

  return (
    <div className="h-full overflow-auto p-6">
      <div className="max-w-4xl mx-auto">
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-xl font-semibold text-white">Scheduled Tasks</h2>
          <button
            type="button"
            onClick={() => {
              setForm(emptyTaskForm(hosts[0]?.id));
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
          <RunResultBanner result={lastRunResult} onDismiss={() => setLastRunResult(null)} />
        )}

        {showForm && (
          <ScheduledTaskForm
            form={form}
            setForm={setForm}
            editing={editingId !== null}
            hosts={hosts}
            credentials={credentials}
            onSave={handleSave}
            onCancel={resetForm}
            onError={setError}
          />
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
          <ScheduledTaskList
            tasks={tasks}
            hostName={hostName}
            credName={credName}
            runningId={runningId}
            onRun={handleRunNow}
            onEdit={startEdit}
            onDelete={handleDelete}
          />
        )}
      </div>
    </div>
  );
};

export default ScheduledTasksView;
