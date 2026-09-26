import { describe, it, expect } from 'vitest';
import {
  buildTaskPayload, emptyTaskForm, formFromTask, hasCronShape, taskHosts, validateTaskForm, withTaskType,
} from './taskForm';
import type { ScheduledTask, TaskCredential, TaskHost } from './taskForm';

const ready = { ...emptyTaskForm('h1'), name: 'Backup', command: '/opt/backup.sh' };

describe('taskForm', () => {
  it('accepts 6- and 7-field cron shapes only, leaving the grammar to the backend', () => {
    expect(hasCronShape('0 */5 * * * *')).toBe(true);
    expect(hasCronShape(' 0 0 9 * * * 2027 ')).toBe(true);
    expect(hasCronShape('*/5 * * * *')).toBe(false);
  });

  it('reports the first problem, or null', () => {
    expect(validateTaskForm(ready)).toBeNull();
    expect(validateTaskForm({ ...ready, host_id: '' })).toBe('Name, schedule, and host are required.');
    expect(validateTaskForm({ ...ready, cron_expression: '* * *' })).toMatch(/Invalid cron expression/);
    expect(validateTaskForm({ ...ready, command: ' ' })).toBe('Command is required for SSH tasks.');
    expect(validateTaskForm({ ...ready, task_type: 'sftp_upload', local_path: '/a' })).toMatch(/Local path and remote path/);
  });

  it('sends only the fields of its type, trimmed, with camelCase keys', () => {
    const ssh = buildTaskPayload({ ...ready, name: ' Backup ', local_path: '/ignored' });
    expect(ssh).toMatchObject({ name: 'Backup', command: '/opt/backup.sh', localPath: null, remotePath: null, credentialId: null });
    const up = buildTaskPayload({ ...ready, task_type: 'sftp_upload', local_path: '/l', remote_path: ' /r ' });
    expect(up).toMatchObject({ command: '', localPath: '/l', remotePath: '/r', taskType: 'sftp_upload' });
    expect(Object.keys(up).filter((k) => k.includes('_'))).toEqual([]);
  });

  it('drops a credential the new task type cannot use', () => {
    const creds: TaskCredential[] = [
      { id: 'k', name: 'key', username: 'u', credential_type: 'ssh_key' },
      { id: 'p', name: 'pwd', username: 'u', credential_type: 'ssh' },
    ];
    expect(withTaskType({ ...ready, credential_id: 'k' }, 'sftp_upload', creds).credential_id).toBeNull();
    expect(withTaskType({ ...ready, credential_id: 'p' }, 'sftp_upload', creds).credential_id).toBe('p');
  });

  it('offers only SSH hosts', () => {
    const hosts = [
      { id: '1', protocol: ' SSH ' }, { id: '2', protocol: 'rdp' }, { id: '3', protocol: 'database' },
    ] as TaskHost[];
    expect(taskHosts(hosts).map((h) => h.id)).toEqual(['1']);
  });

  it('loads an unknown stored task type as an SSH task', () => {
    const task = { ...ready, id: 't', task_type: 'weird', local_path: null, remote_path: null } as unknown as ScheduledTask;
    expect(formFromTask(task)).toMatchObject({ task_type: 'ssh', local_path: '', remote_path: '' });
  });
});
