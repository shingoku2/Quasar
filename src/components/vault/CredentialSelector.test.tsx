import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import CredentialSelector from './CredentialSelector';
import '@testing-library/jest-dom';

const mockInvoke = vi.mocked(invoke);

const mockCredentials = [
  { id: '1', name: 'SSH Key', username: 'admin', credential_type: 'ssh_key', host: '192.168.1.10' },
  { id: '2', name: 'Dev SSH', username: 'dev', credential_type: 'ssh', host: '192.168.1.10' },
  { id: '3', name: 'Prod SSH', username: 'root', credential_type: 'ssh', host: '10.0.0.1' },
  { id: '4', name: 'API Token', username: 'bot', credential_type: 'api' },
];

describe('CredentialSelector', () => {
  const onSelect = vi.fn();
  const onCancel = vi.fn();
  const onManualEntry = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue(mockCredentials as any);
  });

  it('shows loading state initially', () => {
    mockInvoke.mockReturnValue(new Promise(() => {}));
    render(
      <CredentialSelector onSelect={onSelect} onCancel={onCancel} onManualEntry={onManualEntry} />
    );
    expect(screen.getByText(/Loading credentials/i)).toBeInTheDocument();
  });

  it('shows credentials after loading', async () => {
    render(
      <CredentialSelector onSelect={onSelect} onCancel={onCancel} onManualEntry={onManualEntry} />
    );
    await waitFor(() => {
      expect(screen.getByText('SSH Key')).toBeInTheDocument();
      expect(screen.getByText('Dev SSH')).toBeInTheDocument();
    });
  });

  it('shows empty state when no credentials are available', async () => {
    mockInvoke.mockResolvedValueOnce([]);
    render(
      <CredentialSelector onSelect={onSelect} onCancel={onCancel} onManualEntry={onManualEntry} />
    );
    await waitFor(() => {
      expect(screen.getByText(/No credentials found/i)).toBeInTheDocument();
    });
  });

  it('filters credentials by allowedTypes — excludes ssh_key for SFTP contexts', async () => {
    render(
      <CredentialSelector
        hostAddress="192.168.1.10"
        allowedTypes={['ssh']}
        onSelect={onSelect}
        onCancel={onCancel}
        onManualEntry={onManualEntry}
      />
    );
    await waitFor(() => {
      expect(screen.getByText('Dev SSH')).toBeInTheDocument();
      expect(screen.queryByText('SSH Key')).not.toBeInTheDocument();
    });
  });

  it('filters credentials by hostAddress when no allowedTypes given', async () => {
    render(
      <CredentialSelector
        hostAddress="192.168.1.10"
        onSelect={onSelect}
        onCancel={onCancel}
        onManualEntry={onManualEntry}
      />
    );
    await waitFor(() => {
      // Defaults to ['ssh', 'ssh_key'] and filters by host
      expect(screen.getByText('SSH Key')).toBeInTheDocument();
      expect(screen.getByText('Dev SSH')).toBeInTheDocument();
      // Different host — excluded
      expect(screen.queryByText('Prod SSH')).not.toBeInTheDocument();
    });
  });

  it('filters credentials by allowedTypes when no hostAddress given', async () => {
    render(
      <CredentialSelector
        allowedTypes={['api']}
        onSelect={onSelect}
        onCancel={onCancel}
        onManualEntry={onManualEntry}
      />
    );
    await waitFor(() => {
      expect(screen.getByText('API Token')).toBeInTheDocument();
      expect(screen.queryByText('Dev SSH')).not.toBeInTheDocument();
    });
  });

  it('filters visible credentials by search query', async () => {
    render(
      <CredentialSelector onSelect={onSelect} onCancel={onCancel} onManualEntry={onManualEntry} />
    );
    await waitFor(() => expect(screen.getByText('Dev SSH')).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText(/Search credentials/i), { target: { value: 'prod' } });
    expect(screen.queryByText('Dev SSH')).not.toBeInTheDocument();
    expect(screen.getByText('Prod SSH')).toBeInTheDocument();
  });

  it('calls onSelect with full credential data when a credential is clicked', async () => {
    const fullCred = { id: '2', name: 'Dev SSH', username: 'dev', credential_type: 'ssh', password: 'secret' };
    mockInvoke
      .mockResolvedValueOnce(mockCredentials as any)
      .mockResolvedValueOnce(fullCred as any);

    render(
      <CredentialSelector onSelect={onSelect} onCancel={onCancel} onManualEntry={onManualEntry} />
    );
    await waitFor(() => expect(screen.getByText('Dev SSH')).toBeInTheDocument());
    fireEvent.click(screen.getByText('Dev SSH'));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_credential', { credentialId: '2' });
      expect(onSelect).toHaveBeenCalledWith(fullCred);
    });
  });

  it('calls onManualEntry when Enter Manually is clicked', async () => {
    render(
      <CredentialSelector onSelect={onSelect} onCancel={onCancel} onManualEntry={onManualEntry} />
    );
    await waitFor(() => expect(screen.getByText('Dev SSH')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: /Enter Manually/i }));
    expect(onManualEntry).toHaveBeenCalledOnce();
  });

  it('calls onCancel when Cancel is clicked', async () => {
    render(
      <CredentialSelector onSelect={onSelect} onCancel={onCancel} onManualEntry={onManualEntry} />
    );
    await waitFor(() => expect(screen.getByText('Dev SSH')).toBeInTheDocument());
    // There are two Cancel buttons (header X and footer Cancel); click the text one
    const cancelBtns = screen.getAllByRole('button', { name: /Cancel/i });
    fireEvent.click(cancelBtns[cancelBtns.length - 1]);
    expect(onCancel).toHaveBeenCalled();
  });

  it('shows error when list_credentials fails', async () => {
    mockInvoke.mockRejectedValueOnce('Failed to decrypt vault');
    render(
      <CredentialSelector onSelect={onSelect} onCancel={onCancel} onManualEntry={onManualEntry} />
    );
    await waitFor(() => {
      expect(screen.getByText('Failed to decrypt vault')).toBeInTheDocument();
    });
  });

  it('shows the target host address when hostAddress prop is given', async () => {
    render(
      <CredentialSelector
        hostAddress="my-server.local"
        onSelect={onSelect}
        onCancel={onCancel}
        onManualEntry={onManualEntry}
      />
    );
    await waitFor(() => {
      expect(screen.getByText('my-server.local')).toBeInTheDocument();
    });
  });
});
