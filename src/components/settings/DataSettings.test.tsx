import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import DataSettings from './DataSettings';
import type { AppInfo } from './appInfo';
import '@testing-library/jest-dom';

const mockInvoke = vi.mocked(invoke);

const appInfo = (db_size_bytes: number | null): AppInfo => ({
  app_data_dir: '/data',
  db_path: '/data/quasar.db',
  db_size_bytes,
  version: '0.1.0',
  platform: 'linux',
  arch: 'x86_64',
});

describe('DataSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue(appInfo(512));
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('loads and shows the database location, size, and retention policy', async () => {
    render(<DataSettings />);
    await waitFor(() => expect(screen.getByText('/data/quasar.db')).toBeInTheDocument());
    expect(screen.getByText('512 B')).toBeInTheDocument();
    expect(screen.getByText('30 days')).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith('get_app_info');
  });

  // The size is the only live formatting logic; cover every branch of it.
  it.each([
    [512, '512 B'],
    [2048, '2.0 KB'],
    [1536, '1.5 KB'],
    [3_145_728, '3.00 MB'],
  ])('formats a %i-byte database as %s', async (bytes, expected) => {
    mockInvoke.mockResolvedValue(appInfo(bytes));
    render(<DataSettings />);
    await waitFor(() => expect(screen.getByText(expected)).toBeInTheDocument());
  });

  it('shows a dash when the database size is unknown', async () => {
    mockInvoke.mockResolvedValue(appInfo(null));
    render(<DataSettings />);
    await waitFor(() => expect(screen.getByText('—')).toBeInTheDocument());
  });

  it('shows the error when get_app_info fails', async () => {
    mockInvoke.mockRejectedValue(new Error('db is locked'));
    render(<DataSettings />);
    expect(await screen.findByText('db is locked')).toBeInTheDocument();
    expect(screen.getByText('Unable to load database info.')).toBeInTheDocument();
  });

  it('exports the backup to the location picked in the native dialog', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'pick_save_location') return '/backups/quasar-backup.db';
      return appInfo(512);
    });
    render(<DataSettings />);
    await screen.findByText('/data/quasar.db');

    fireEvent.click(screen.getByRole('button', { name: /Export Database Backup/ }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('export_database', { destPath: '/backups/quasar-backup.db' }),
    );
    // The payload keys are lowerCamelCase; a snake_case key would silently arrive as None.
    const pickCall = mockInvoke.mock.calls.find(([cmd]) => cmd === 'pick_save_location');
    expect(pickCall?.[1]).toMatchObject({ defaultName: expect.stringMatching(/quasar-backup-\d{4}-\d{2}-\d{2}\.db/), extensions: ['db'] });
    for (const key of Object.keys(pickCall?.[1] as object)) expect(key).not.toContain('_');
    // Success is shown and the info panel refreshed.
    expect(await screen.findByText('Database exported successfully.')).toBeInTheDocument();
    await waitFor(() =>
      expect(mockInvoke.mock.calls.filter(([cmd]) => cmd === 'get_app_info').length).toBeGreaterThan(1),
    );
  });

  it('does not export when the save dialog is cancelled', async () => {
    mockInvoke.mockImplementation(async (cmd: string) =>
      cmd === 'pick_save_location' ? null : appInfo(512),
    );
    render(<DataSettings />);
    await screen.findByText('/data/quasar.db');

    fireEvent.click(screen.getByRole('button', { name: /Export Database Backup/ }));

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('pick_save_location', expect.anything()));
    expect(mockInvoke).not.toHaveBeenCalledWith('export_database', expect.anything());
  });

  it('shows the error when the export fails', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'pick_save_location') return '/backups/quasar-backup.db';
      if (cmd === 'export_database') throw new Error('disk full');
      return appInfo(512);
    });
    render(<DataSettings />);
    await screen.findByText('/data/quasar.db');

    fireEvent.click(screen.getByRole('button', { name: /Export Database Backup/ }));

    expect(await screen.findByText('disk full')).toBeInTheDocument();
  });

  it('imports the picked backup and warns that the vault is locked', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'pick_local_file') return '/downloads/old-quasar.db';
      return appInfo(512);
    });
    render(<DataSettings />);
    await screen.findByText('/data/quasar.db');

    fireEvent.click(screen.getByRole('button', { name: /Import Database Backup/ }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('import_database', { sourcePath: '/downloads/old-quasar.db' }),
    );
    const pickCall = mockInvoke.mock.calls.find(([cmd]) => cmd === 'pick_local_file');
    expect(pickCall?.[1]).toMatchObject({ title: expect.any(String), extensions: ['db'] });
    expect(await screen.findByText(/Database imported\./)).toBeInTheDocument();
  });

  it('does not import when the open dialog is cancelled', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => (cmd === 'pick_local_file' ? null : appInfo(512)));
    render(<DataSettings />);
    await screen.findByText('/data/quasar.db');

    fireEvent.click(screen.getByRole('button', { name: /Import Database Backup/ }));

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('pick_local_file', expect.anything()));
    expect(mockInvoke).not.toHaveBeenCalledWith('import_database', expect.anything());
  });

  it('shows the error when an import fails', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'pick_local_file') return '/downloads/old-quasar.db';
      if (cmd === 'import_database') throw new Error('backup is from a newer version');
      return appInfo(512);
    });
    render(<DataSettings />);
    await screen.findByText('/data/quasar.db');

    fireEvent.click(screen.getByRole('button', { name: /Import Database Backup/ }));

    expect(await screen.findByText('backup is from a newer version')).toBeInTheDocument();
  });

  it('clears metrics only after the destructive-action confirmation', async () => {
    mockInvoke.mockResolvedValue(appInfo(512));
    const confirmSpy = vi.spyOn(window, 'confirm');
    render(<DataSettings />);
    await screen.findByText('/data/quasar.db');

    // Declined: nothing happens.
    confirmSpy.mockReturnValue(false);
    fireEvent.click(screen.getByRole('button', { name: /Clear All Metrics Data/ }));
    expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('cannot be undone'));
    expect(mockInvoke).not.toHaveBeenCalledWith('clear_metrics_data');

    // Confirmed: the data is cleared and the success note appears.
    confirmSpy.mockReturnValue(true);
    fireEvent.click(screen.getByRole('button', { name: /Clear All Metrics Data/ }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('clear_metrics_data'));
    expect(await screen.findByText('Metrics and alert history cleared.')).toBeInTheDocument();
  });

  it('shows the error when clearing metrics fails', async () => {
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'clear_metrics_data') throw 'permission denied';
      return appInfo(512);
    });
    render(<DataSettings />);
    await screen.findByText('/data/quasar.db');

    fireEvent.click(screen.getByRole('button', { name: /Clear All Metrics Data/ }));

    expect(await screen.findByText('permission denied')).toBeInTheDocument();
  });
});
