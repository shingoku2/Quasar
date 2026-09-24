import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import SshFileManager from './SshFileManager';
import '@testing-library/jest-dom';

// Mock Tauri API
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

const defaultProps = {
  host: '192.168.1.100',
  port: 22,
  username: 'admin',
  password: 'secret',
};

describe('SshFileManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders file manager with path bar', async () => {
    mockInvoke.mockResolvedValueOnce([]); // sftp_list_directory returns empty

    render(<SshFileManager {...defaultProps} />);

    await waitFor(() => {
      expect(screen.getByText('/')).toBeInTheDocument();
    });
  });

  it('shows loading state while fetching files', () => {
    mockInvoke.mockReturnValue(new Promise(() => {})); // never resolves

    render(<SshFileManager {...defaultProps} />);

    // Should show some loading indicator
    expect(screen.getByText('/')).toBeInTheDocument();
  });

  it('does not fetch files when password is missing', () => {
    render(<SshFileManager host="192.168.1.100" port={22} username="admin" />);

    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it('displays error when file listing fails', async () => {
    mockInvoke.mockRejectedValueOnce('Connection refused');

    render(<SshFileManager {...defaultProps} />);

    await waitFor(() => {
      expect(screen.getAllByText(/failed|error|refused/i).length).toBeGreaterThan(0);
    });
  });

  it('displays files when listing succeeds', async () => {
    mockInvoke.mockResolvedValueOnce([
      { name: 'documents', is_dir: true, size: 4096, permissions: 755, modified: 1700000000 },
      { name: 'readme.txt', is_dir: false, size: 1024, permissions: 644, modified: 1700000000 },
    ]);

    render(<SshFileManager {...defaultProps} />);

    await waitFor(() => {
      expect(screen.getByText('documents')).toBeInTheDocument();
      expect(screen.getByText('readme.txt')).toBeInTheDocument();
    });
  });

  // FE-001: a vault credential is passed by id, never as a password.
  it('lists files with the credential id when given one', async () => {
    mockInvoke.mockResolvedValueOnce([]);
    render(<SshFileManager host="10.0.0.5" port={22} username="root" credentialId="c1" />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('sftp_list_directory', {
        host: '10.0.0.5',
        port: 22,
        username: 'root',
        password: null,
        credentialId: 'c1',
        remotePath: '/',
      });
    });
  });

  // IPC-001: uploads use a path the backend's own dialog returned.
  it('asks the backend to pick the upload source', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'sftp_list_directory') return [];
      if (cmd === 'pick_local_file') return '/home/u/report.txt';
      return undefined;
    });
    render(<SshFileManager {...defaultProps} />);
    const upload = await screen.findByRole('button', { name: /upload/i });
    await waitFor(() => expect(upload).not.toBeDisabled());
    fireEvent.click(upload);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('sftp_upload_file', expect.objectContaining({
        localPath: '/home/u/report.txt',
        remotePath: '/report.txt',
      }));
    });
    expect(mockInvoke).toHaveBeenCalledWith('pick_local_file', expect.anything());
  });
});
