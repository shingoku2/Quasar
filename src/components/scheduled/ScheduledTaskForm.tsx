import React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getErrorMessage } from '../../lib/utils';
import { allowedCredentialTypes, isTransfer, withTaskType } from './taskForm';
import type { TaskCredential, TaskForm, TaskHost, TaskType } from './taskForm';

const ScheduledTaskForm: React.FC<{
  form: TaskForm;
  setForm: React.Dispatch<React.SetStateAction<TaskForm>>;
  editing: boolean;
  /** Hosts tasks can run on (SSH only). */
  hosts: TaskHost[];
  credentials: TaskCredential[];
  onSave: () => void;
  onCancel: () => void;
  onError: (message: string) => void;
}> = ({ form, setForm, editing, hosts, credentials, onSave, onCancel, onError }) => (
    <div className="mb-6 p-4 rounded-xl bg-bg-card border border-border">
      <h3 className="text-sm font-medium text-gray-300 mb-4">
        {editing ? 'Edit task' : 'New task'}
      </h3>
      <div className="grid gap-4 sm:grid-cols-2">
        <div>
          <label htmlFor="task-name" className="block text-xs text-gray-500 mb-1">Name</label>
          <input id="task-name"
            value={form.name}
            onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
            className="w-full bg-bg-sidebar border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-accent"
            placeholder="e.g. Daily backup"
          />
        </div>
        <div>
          <label htmlFor="task-cron" className="block text-xs text-gray-500 mb-1">Cron schedule (sec min hour day month dow)</label>
          <input id="task-cron"
            value={form.cron_expression}
            onChange={(e) => setForm((f) => ({ ...f, cron_expression: e.target.value }))}
            className="w-full bg-bg-sidebar border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-accent font-mono"
            placeholder="0 0 9 * * *"
          />
        </div>
        <div>
          <label htmlFor="task-host" className="block text-xs text-gray-500 mb-1">Host</label>
          <select id="task-host"
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
          <label htmlFor="task-credential" className="block text-xs text-gray-500 mb-1">Credential (optional)</label>
          <select id="task-credential"
            value={form.credential_id ?? ''}
            onChange={(e) =>
              setForm((f) => ({ ...f, credential_id: e.target.value || null }))
            }
            className="w-full bg-bg-sidebar border border-border rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-accent"
          >
            <option value="">None</option>
            {credentials
              .filter((c) => allowedCredentialTypes(form.task_type).includes(c.credential_type))
              .map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name} ({c.username})
                </option>
              ))}
          </select>
        </div>
        <div>
          <label htmlFor="task-type" className="block text-xs text-gray-500 mb-1">Task type</label>
          <select id="task-type"
            value={form.task_type}
            onChange={(e) => {
              const newType = e.target.value as TaskType;
              setForm((f) => withTaskType(f, newType, credentials));
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
          <label htmlFor="task-command" className="block text-xs text-gray-500 mb-1">Command</label>
          <input id="task-command"
            value={form.command}
            onChange={(e) => setForm((f) => ({ ...f, command: e.target.value }))}
            className="w-full bg-bg-sidebar border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-accent font-mono"
            placeholder="e.g. /opt/scripts/backup.sh"
          />
        </div>
      )}
      {isTransfer(form.task_type) && (
        <div className="mt-4 grid gap-4 sm:grid-cols-2">
          <div>
            <label htmlFor="task-local-path" className="block text-xs text-gray-500 mb-1">Local path</label>
            {/* Local paths come from a backend-opened dialog; the backend rejects typed ones (IPC-001). */}
            <div className="flex gap-2">
              <input id="task-local-path"
                value={form.local_path}
                readOnly
                aria-label="Local path"
                className="w-full bg-bg-sidebar border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-accent font-mono"
                placeholder="Choose a file…"
              />
              <button
                type="button"
                onClick={async () => {
                  try {
                    const picked = form.task_type === 'sftp_upload'
                      ? await invoke<string | null>('pick_local_file', { title: 'File to upload' })
                      : await invoke<string | null>('pick_save_location', { defaultName: null });
                    if (picked) setForm((f) => ({ ...f, local_path: picked }));
                  } catch (err) {
                    onError(getErrorMessage(err, 'Could not open the file dialog'));
                  }
                }}
                className="shrink-0 bg-bg-sidebar border border-border hover:border-accent rounded-lg px-3 py-2 text-sm text-white"
              >
                Browse…
              </button>
            </div>
          </div>
          <div>
            <label htmlFor="task-remote-path" className="block text-xs text-gray-500 mb-1">Remote path</label>
            <input id="task-remote-path"
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
          onClick={onSave}
          className="px-4 py-2 bg-accent text-white rounded-lg hover:bg-accent/90 text-sm font-medium"
        >
          {editing ? 'Update' : 'Add'}
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="px-4 py-2 bg-bg-sidebar border border-border text-gray-300 rounded-lg hover:bg-white/5 text-sm"
        >
          Cancel
        </button>
      </div>
    </div>
);

export default ScheduledTaskForm;
