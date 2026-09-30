import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import CredentialViewDialog from './CredentialViewDialog';
import type { Credential } from './types';
import { flushCopiedSecret } from '../../../lib/copiedSecret';
import '@testing-library/jest-dom';

const mockInvoke = vi.mocked(invoke);

const baseCred: Credential = {
  id: 'cred-1',
  name: 'Production DB',
  credential_type: 'ssh',
  username: 'postgres',
  host: 'db.internal',
  port: 5432,
  has_password: true,
  has_private_key: false,
  has_key_passphrase: false,
  created_at: '2026-01-01T00:00:00Z',
};

describe('CredentialViewDialog', () => {
  const originalClipboard = navigator.clipboard;
  let clipboardText = '';
  const writeText = vi.fn(async (text: string) => {
    clipboardText = text;
  });
  const readText = vi.fn(async () => clipboardText);

  beforeEach(() => {
    vi.clearAllMocks();
    clipboardText = '';
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText, readText },
      configurable: true,
    });
  });

  afterEach(() => {
    flushCopiedSecret();
    Object.defineProperty(navigator, 'clipboard', {
      value: originalClipboard,
      configurable: true,
    });
    vi.restoreAllMocks();
  });

  it('renders credential name, type, host:port, and username', () => {
    render(<CredentialViewDialog credential={baseCred} onClose={vi.fn()} />);

    expect(screen.getByText('Production DB')).toBeInTheDocument();
    expect(screen.getByText('ssh')).toBeInTheDocument();
    expect(screen.getByText('db.internal:5432')).toBeInTheDocument();
    expect(screen.getByText('postgres')).toBeInTheDocument();
  });

  it('omits host section when host is not configured', () => {
    const noHostCred: Credential = {
      ...baseCred,
      host: undefined,
      port: undefined,
    };
    render(<CredentialViewDialog credential={noHostCred} onClose={vi.fn()} />);

    expect(screen.queryByText('Host')).not.toBeInTheDocument();
    expect(screen.queryByText('db.internal:5432')).not.toBeInTheDocument();
  });

  it('omits password section when has_password is false', () => {
    const keyOnlyCred: Credential = {
      ...baseCred,
      has_password: false,
    };
    render(<CredentialViewDialog credential={keyOnlyCred} onClose={vi.fn()} />);

    expect(screen.queryByText('Password')).not.toBeInTheDocument();
    expect(screen.queryByText('••••••••••••')).not.toBeInTheDocument();
  });

  it('copies username to clipboard and shows "Copied!" feedback', async () => {
    render(<CredentialViewDialog credential={baseCred} onClose={vi.fn()} />);

    const copyUsernameBtn = screen.getByRole('button', { name: 'Copy' });
    fireEvent.click(copyUsernameBtn);

    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith('postgres');
      expect(screen.getByText('Copied!')).toBeInTheDocument();
    });
  });

  it('reveals password on toggle, enables copy password, and hides on second toggle', async () => {
    mockInvoke.mockImplementation(async (cmd: string, args: unknown) => {
      if (cmd === 'reveal_credential_password') {
        const payload = args as { credentialId: string };
        if (payload.credentialId === 'cred-1') return 'secret-master-pw';
      }
      return undefined;
    });

    render(<CredentialViewDialog credential={baseCred} onClose={vi.fn()} />);

    // Initially masked
    expect(screen.getByText('••••••••••••')).toBeInTheDocument();
    const copyPwBtn = screen.getByLabelText('Copy password');
    expect(copyPwBtn).toBeDisabled();

    // Click toggle to reveal
    const toggleBtn = screen.getByLabelText('Toggle password visibility');
    fireEvent.click(toggleBtn);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('reveal_credential_password', {
        credentialId: 'cred-1',
      });
      expect(screen.getByText('secret-master-pw')).toBeInTheDocument();
    });

    // Copy password button is now enabled
    expect(copyPwBtn).not.toBeDisabled();
    fireEvent.click(copyPwBtn);

    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith('secret-master-pw');
    });

    // Toggle again to hide
    fireEvent.click(toggleBtn);
    expect(screen.getByText('••••••••••••')).toBeInTheDocument();
    expect(screen.queryByText('secret-master-pw')).not.toBeInTheDocument();
  });

  it('calls onClose when close button or X button is clicked', () => {
    const onClose = vi.fn();
    render(<CredentialViewDialog credential={baseCred} onClose={onClose} />);

    fireEvent.click(screen.getByLabelText('Close dialog'));
    expect(onClose).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
