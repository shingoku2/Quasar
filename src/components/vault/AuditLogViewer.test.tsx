import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import AuditLogViewer from './AuditLogViewer';
import '@testing-library/jest-dom';

const mockInvoke = vi.mocked(invoke);

const mockLogs = [
  {
    id: '1',
    timestamp: 1700000000,
    event_type: 'vault_unlock',
    action: 'Vault unlocked',
    result: 'success',
    details: 'Master password accepted',
  },
  {
    id: '2',
    timestamp: 1700001000,
    event_type: 'credential_access',
    action: 'Credential retrieved',
    result: 'success',
    resource_type: 'credential',
    resource_id: 'abcdef1234567890',
  },
  {
    id: '3',
    timestamp: 1700002000,
    event_type: 'vault_unlock',
    action: 'Vault unlock failed',
    result: 'failure',
    details: 'Invalid master password',
  },
  {
    id: '4',
    timestamp: 1700003000,
    event_type: 'credential_delete',
    action: 'Credential deleted',
    result: 'denied',
  },
];

describe('AuditLogViewer', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue(mockLogs as any);
  });

  it('shows loading state initially', () => {
    mockInvoke.mockReturnValue(new Promise(() => {}));
    render(<AuditLogViewer />);
    expect(screen.getByText(/Loading audit logs/i)).toBeInTheDocument();
  });

  it('renders audit log entries after loading', async () => {
    render(<AuditLogViewer />);
    await waitFor(() => {
      expect(screen.getByText('Vault unlocked')).toBeInTheDocument();
      expect(screen.getByText('Credential retrieved')).toBeInTheDocument();
    });
  });

  it('shows empty state when no logs exist', async () => {
    mockInvoke.mockResolvedValueOnce([]);
    render(<AuditLogViewer />);
    await waitFor(() => {
      expect(screen.getByText(/No audit logs yet/i)).toBeInTheDocument();
    });
  });

  it('shows error when loading fails', async () => {
    mockInvoke.mockRejectedValueOnce('Permission denied');
    render(<AuditLogViewer />);
    await waitFor(() => {
      expect(screen.getByText('Permission denied')).toBeInTheDocument();
    });
  });

  it('calls get_audit_logs on mount with limit and offset', async () => {
    render(<AuditLogViewer />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_audit_logs', {
        filter: { limit: 100, offset: 0 },
      });
    });
  });

  it('shows entry count in header', async () => {
    render(<AuditLogViewer />);
    await waitFor(() => {
      expect(screen.getByText('4 entries')).toBeInTheDocument();
    });
  });

  it('filters logs by search query', async () => {
    render(<AuditLogViewer />);
    await waitFor(() => expect(screen.getByText('Vault unlocked')).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText(/Search audit logs/i), {
      target: { value: 'Credential retrieved' },
    });

    expect(screen.queryByText('Vault unlocked')).not.toBeInTheDocument();
    expect(screen.getByText('Credential retrieved')).toBeInTheDocument();
  });

  it('filters by event type dropdown', async () => {
    render(<AuditLogViewer />);
    await waitFor(() => expect(screen.getByText('Vault unlocked')).toBeInTheDocument());

    // The event type select is the first combobox on the page (no aria-label on the select itself)
    const selects = screen.getAllByRole('combobox');
    fireEvent.change(selects[0], { target: { value: 'credential_access' } });

    expect(screen.queryByText('Vault unlocked')).not.toBeInTheDocument();
    expect(screen.getByText('Credential retrieved')).toBeInTheDocument();
  });

  it('filters by result dropdown', async () => {
    render(<AuditLogViewer />);
    await waitFor(() => expect(screen.getByText('Vault unlocked')).toBeInTheDocument());

    // The result select is the second combobox on the page
    const selects = screen.getAllByRole('combobox');
    fireEvent.change(selects[1], { target: { value: 'failure' } });

    expect(screen.queryByText('Vault unlocked')).not.toBeInTheDocument();
    expect(screen.getByText('Vault unlock failed')).toBeInTheDocument();
  });

  it('shows "no matching logs" message when filters return nothing', async () => {
    render(<AuditLogViewer />);
    await waitFor(() => expect(screen.getByText('Vault unlocked')).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText(/Search audit logs/i), {
      target: { value: 'this-matches-nothing' },
    });

    expect(screen.getByText(/No matching audit logs found/i)).toBeInTheDocument();
  });

  it('shows details section for logs that have details', async () => {
    render(<AuditLogViewer />);
    await waitFor(() => {
      expect(screen.getByText('Master password accepted')).toBeInTheDocument();
    });
  });

  it('shows resource info for logs that have resource_type', async () => {
    render(<AuditLogViewer />);
    await waitFor(() => {
      // Log 2 has resource_type 'credential' — the "Resource:" label only appears for logs with a resource
      expect(screen.getByText('Resource:')).toBeInTheDocument();
    });
  });
});
