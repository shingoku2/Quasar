import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import CredentialManager from './CredentialManager';
import '@testing-library/jest-dom';

const mockInvoke = vi.mocked(invoke);

const mockCredentials = [
  {
    id: '1',
    name: 'Prod SSH',
    username: 'root',
    credential_type: 'ssh',
    host: '10.0.0.1',
    port: 22,
    created_at: '2024-01-01T00:00:00Z',
  },
  {
    id: '2',
    name: 'Dev API',
    username: 'bot',
    credential_type: 'api',
    created_at: '2024-01-02T00:00:00Z',
  },
];

describe('CredentialManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue(mockCredentials as any);
  });

  it('shows loading state initially', () => {
    mockInvoke.mockReturnValue(new Promise(() => {}));
    render(<CredentialManager />);
    expect(screen.getByText(/Loading credentials/i)).toBeInTheDocument();
  });

  it('renders credential cards after loading', async () => {
    render(<CredentialManager />);
    await waitFor(() => {
      expect(screen.getByText('Prod SSH')).toBeInTheDocument();
      expect(screen.getByText('Dev API')).toBeInTheDocument();
    });
  });

  it('shows empty state when no credentials exist', async () => {
    mockInvoke.mockResolvedValueOnce([]);
    render(<CredentialManager />);
    await waitFor(() => {
      expect(screen.getByText(/No credentials stored yet/i)).toBeInTheDocument();
    });
  });

  it('shows error when loading fails', async () => {
    mockInvoke.mockRejectedValueOnce('Vault is locked');
    render(<CredentialManager />);
    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent('Vault is locked');
    });
  });

  it('opens add dialog when Add Credential is clicked', async () => {
    render(<CredentialManager />);
    await waitFor(() => expect(screen.getByText('Prod SSH')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /Add Credential/i }));
    expect(screen.getByRole('dialog', { name: /Add Credential/i })).toBeInTheDocument();
  });

  it('closes add dialog when Cancel is clicked inside it', async () => {
    render(<CredentialManager />);
    await waitFor(() => expect(screen.getByText('Prod SSH')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /Add Credential/i }));
    const dialog = screen.getByRole('dialog', { name: /Add Credential/i });
    fireEvent.click(screen.getByRole('button', { name: /^Cancel$/i }));
    expect(dialog).not.toBeInTheDocument();
  });

  it('opens view dialog when View button is clicked', async () => {
    const fullCred = { ...mockCredentials[0], password: 'secret123' };
    mockInvoke
      .mockResolvedValueOnce(mockCredentials as any)
      .mockResolvedValueOnce(fullCred as any);

    render(<CredentialManager />);
    await waitFor(() => expect(screen.getByText('Prod SSH')).toBeInTheDocument());

    const viewButtons = screen.getAllByTitle('View');
    fireEvent.click(viewButtons[0]);

    await waitFor(() => {
      expect(screen.getByRole('dialog', { name: /View Credential/i })).toBeInTheDocument();
    });
  });

  it('opens edit dialog when Edit button is clicked', async () => {
    const fullCred = { ...mockCredentials[0], password: 'secret123' };
    mockInvoke
      .mockResolvedValueOnce(mockCredentials as any)
      .mockResolvedValueOnce(fullCred as any);

    render(<CredentialManager />);
    await waitFor(() => expect(screen.getByText('Prod SSH')).toBeInTheDocument());

    const editButtons = screen.getAllByTitle('Edit');
    fireEvent.click(editButtons[0]);

    await waitFor(() => {
      expect(screen.getByRole('dialog', { name: /Edit Credential/i })).toBeInTheDocument();
    });
  });

  it('calls delete_credential after confirmation', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    mockInvoke
      .mockResolvedValueOnce(mockCredentials as any)
      .mockResolvedValueOnce(undefined) // delete_credential
      .mockResolvedValueOnce([mockCredentials[1]] as any); // reload

    render(<CredentialManager />);
    await waitFor(() => expect(screen.getByText('Prod SSH')).toBeInTheDocument());

    const deleteButtons = screen.getAllByTitle('Delete');
    fireEvent.click(deleteButtons[0]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('delete_credential', { credentialId: '1' });
    });
    confirmSpy.mockRestore();
  });

  it('does not delete credential when user cancels confirmation', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false);
    render(<CredentialManager />);
    await waitFor(() => expect(screen.getByText('Prod SSH')).toBeInTheDocument());

    const deleteButtons = screen.getAllByTitle('Delete');
    fireEvent.click(deleteButtons[0]);

    expect(mockInvoke).not.toHaveBeenCalledWith('delete_credential', expect.anything());
    confirmSpy.mockRestore();
  });

  it('calls search_credentials when Search button is clicked with a query', async () => {
    const searchResults = [mockCredentials[0]];
    mockInvoke
      .mockResolvedValueOnce(mockCredentials as any)
      .mockResolvedValueOnce(searchResults as any);

    render(<CredentialManager />);
    await waitFor(() => expect(screen.getByText('Prod SSH')).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText(/Search credentials/i), { target: { value: 'Prod' } });
    fireEvent.click(screen.getByRole('button', { name: /^Search$/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('search_credentials', { query: 'Prod' });
    });
  });

  it('adds a new credential successfully', async () => {
    mockInvoke
      .mockResolvedValueOnce(mockCredentials as any)
      .mockResolvedValueOnce(undefined) // add_credential
      .mockResolvedValueOnce(mockCredentials as any); // reload

    render(<CredentialManager />);
    await waitFor(() => expect(screen.getByText('Prod SSH')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /Add Credential/i }));

    fireEvent.change(screen.getByPlaceholderText('My Server'), { target: { value: 'New Server' } });
    fireEvent.change(screen.getByPlaceholderText('root'), { target: { value: 'admin' } });
    fireEvent.change(screen.getByPlaceholderText('••••••••'), { target: { value: 'password123' } });

    fireEvent.click(screen.getByRole('button', { name: /^Save$/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('add_credential', expect.objectContaining({
        name: 'New Server',
        username: 'admin',
      }));
    });
  });

  it('sends camelCase argument keys when updating a credential', async () => {
    const fullCred = { ...mockCredentials[0], password: 'secret123' };
    mockInvoke
      .mockResolvedValueOnce(mockCredentials as any) // initial list
      .mockResolvedValueOnce(fullCred as any) // get_credential for edit dialog
      .mockResolvedValueOnce(undefined) // update_credential
      .mockResolvedValueOnce(mockCredentials as any); // reload

    render(<CredentialManager />);
    await waitFor(() => expect(screen.getByText('Prod SSH')).toBeInTheDocument());

    fireEvent.click(screen.getAllByTitle('Edit')[0]);
    await waitFor(() => {
      expect(screen.getByRole('dialog', { name: /Edit Credential/i })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: /^Save$/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('update_credential', expect.objectContaining({
        credentialId: '1',
        credentialType: 'ssh',
      }));
    });

    // Tauri matches invoke args against camelCased Rust parameter names and
    // silently drops unmatched keys, so a snake_case key here means the field
    // never reaches the backend. Guard the whole payload.
    const updateCall = mockInvoke.mock.calls.find(([cmd]) => cmd === 'update_credential');
    expect(updateCall).toBeDefined();
    for (const key of Object.keys(updateCall![1] as Record<string, unknown>)) {
      expect(key).not.toMatch(/_/);
    }
  });
});
