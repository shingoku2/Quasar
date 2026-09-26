import { SFTP_CREDENTIAL_TYPES, SSH_CREDENTIAL_TYPES } from '../../lib/utils';

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

export interface TaskHost {
  id: string;
  name: string;
  address: string;
  port: number;
  username?: string | null;
  protocol: string;
  credential_id?: string | null;
}

export interface TaskCredential {
  id: string;
  name: string;
  username: string;
  credential_type: string;
}

export type TaskType = 'ssh' | 'sftp_upload' | 'sftp_download';

export interface TaskForm {
  name: string;
  cron_expression: string;
  host_id: string;
  command: string;
  credential_id: string | null;
  enabled: boolean;
  task_type: TaskType;
  local_path: string;
  remote_path: string;
}

export const DEFAULT_CRON = '0 0 9 * * *'; // 9:00 daily (6-field: sec min hour day month dow)

export const isTransfer = (type: string): boolean => type === 'sftp_upload' || type === 'sftp_download';

/** Tasks run over SSH; database, API and other hosts are inventory-only (the backend checks too). */
export const taskHosts = (hosts: TaskHost[]): TaskHost[] =>
  hosts.filter((h) => h.protocol.trim().toLowerCase() === 'ssh');

/** The credential types a task of this type may use (the backend checks too). */
export const allowedCredentialTypes = (type: TaskType): readonly string[] =>
  isTransfer(type) ? SFTP_CREDENTIAL_TYPES : SSH_CREDENTIAL_TYPES;

/**
 * Shape check only: 6 fields (sec min hour day month dow) or 7 (plus year). The backend's
 * cron parser is the single source of truth for the grammar and its error message is shown
 * as-is. A hand-rolled field check here used to reject valid steps like `*\/5` and accept
 * values the backend refused (FE-005).
 */
export function hasCronShape(expr: string): boolean {
  const fields = expr.trim().split(/\s+/).length;
  return fields === 6 || fields === 7;
}

export const emptyTaskForm = (hostId = ''): TaskForm => ({
  name: '',
  cron_expression: DEFAULT_CRON,
  host_id: hostId,
  command: '',
  credential_id: null,
  enabled: true,
  task_type: 'ssh',
  local_path: '',
  remote_path: '',
});

export const formFromTask = (task: ScheduledTask): TaskForm => ({
  name: task.name,
  cron_expression: task.cron_expression,
  host_id: task.host_id,
  command: task.command,
  credential_id: task.credential_id || null,
  enabled: task.enabled,
  task_type: isTransfer(task.task_type) ? (task.task_type as TaskType) : 'ssh',
  local_path: task.local_path ?? '',
  remote_path: task.remote_path ?? '',
});

/** Switching type drops a selected credential the new type can't use. */
export function withTaskType(form: TaskForm, type: TaskType, credentials: TaskCredential[]): TaskForm {
  const selected = credentials.find((c) => c.id === form.credential_id);
  const keep = !selected || allowedCredentialTypes(type).includes(selected.credential_type);
  return { ...form, task_type: type, credential_id: keep ? form.credential_id : null };
}

/** The first problem with the form, or null when it can be saved. */
export function validateTaskForm(form: TaskForm): string | null {
  if (!form.name.trim() || !form.cron_expression.trim() || !form.host_id) {
    return 'Name, schedule, and host are required.';
  }
  if (!hasCronShape(form.cron_expression)) {
    return 'Invalid cron expression. Use 6 fields: sec min hour day month dow (e.g. 0 */5 * * * * for every 5 minutes)';
  }
  if (form.task_type === 'ssh') {
    if (!form.command.trim()) return 'Command is required for SSH tasks.';
  } else if (!form.local_path.trim() || !form.remote_path.trim()) {
    return 'Local path and remote path are required for file transfer tasks.';
  }
  return null;
}

/** `add_scheduled_task` / `update_scheduled_task` args (camelCase; see CLAUDE.md IPC). */
export const buildTaskPayload = (form: TaskForm) => {
  const transfer = form.task_type !== 'ssh';
  return {
    name: form.name.trim(),
    cronExpression: form.cron_expression.trim(),
    hostId: form.host_id,
    command: transfer ? '' : form.command.trim(),
    credentialId: form.credential_id || null,
    enabled: form.enabled,
    taskType: form.task_type,
    localPath: transfer ? form.local_path.trim() || null : null,
    remotePath: transfer ? form.remote_path.trim() || null : null,
  };
};
