import React from 'react';
import { Pencil, Play, Trash2 } from 'lucide-react';
import { isTransfer } from './taskForm';
import type { ScheduledTask } from './taskForm';

const formatTs = (ts: number | null) => (ts ? new Date(ts * 1000).toLocaleString() : '—');

const ScheduledTaskList: React.FC<{
  tasks: ScheduledTask[];
  hostName: (hostId: string) => string;
  credName: (credId: string | null) => string;
  runningId: string | null;
  onRun: (task: ScheduledTask) => void;
  onEdit: (task: ScheduledTask) => void;
  onDelete: (id: string) => void;
}> = ({ tasks, hostName, credName, runningId, onRun, onEdit, onDelete }) => (
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
            {isTransfer(task.task_type) && (
              <span className="ml-2 text-accent">
                {task.task_type === 'sftp_upload' ? 'Upload' : 'Download'}
              </span>
            )}
          </div>
          {isTransfer(task.task_type) ? (
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
            onClick={() => onRun(task)}
            disabled={runningId !== null}
            className="p-2 rounded-lg text-gray-400 hover:bg-white/10 hover:text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            title="Run now"
          >
            <Play className="h-4 w-4" />
          </button>
          <button
            type="button"
            onClick={() => onEdit(task)}
            className="p-2 rounded-lg text-gray-400 hover:bg-white/10 hover:text-white transition-colors"
            title="Edit"
          >
            <Pencil className="h-4 w-4" />
          </button>
          <button
            type="button"
            onClick={() => onDelete(task.id)}
            className="p-2 rounded-lg text-gray-400 hover:bg-alert/10 hover:text-alert transition-colors"
            title="Delete"
          >
            <Trash2 className="h-4 w-4" />
          </button>
        </div>
      </li>
    ))}
  </ul>
);

export default ScheduledTaskList;
