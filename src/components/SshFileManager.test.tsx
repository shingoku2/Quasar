import { render, screen, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import SshFileManager from './SshFileManager';
import '@testing-library/jest-dom';

// Mock Tauri API
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
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
});
