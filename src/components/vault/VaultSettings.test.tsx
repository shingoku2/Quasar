import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import VaultSettings from './VaultSettings';
import '@testing-library/jest-dom';

const mockInvoke = vi.mocked(invoke);

const mockSettings = {
  auto_lock_timeout_minutes: 15,
  require_password_on_credential_use: false,
  vault_initialized: true,
};

describe('VaultSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue(mockSettings as any);
  });

  it('shows loading state initially', () => {
    mockInvoke.mockReturnValue(new Promise(() => {}));
    render(<VaultSettings />);
    expect(screen.getByText(/Loading settings/i)).toBeInTheDocument();
  });

  it('renders settings after loading', async () => {
    render(<VaultSettings />);
    await waitFor(() => {
      expect(screen.getByText('Auto-lock Timeout')).toBeInTheDocument();
      expect(screen.getByText('Vault Settings')).toBeInTheDocument();
    });
  });

  it('shows error when loading settings fails', async () => {
    mockInvoke.mockRejectedValueOnce('Failed to load settings');
    render(<VaultSettings />);
    await waitFor(() => {
      expect(screen.getByText('Failed to load settings')).toBeInTheDocument();
    });
  });

  it('shows auto-lock timeout value from settings', async () => {
    render(<VaultSettings />);
    await waitFor(() => {
      const input = screen.getByRole('spinbutton') as HTMLInputElement;
      expect(input.value).toBe('15');
    });
  });

  it('shows vault initialized status', async () => {
    render(<VaultSettings />);
    await waitFor(() => {
      expect(screen.getByText(/Initialized/)).toBeInTheDocument();
    });
  });

  it('calls update_vault_settings on Save', async () => {
    mockInvoke
      .mockResolvedValueOnce(mockSettings as any)
      .mockResolvedValueOnce(undefined);

    render(<VaultSettings />);
    await waitFor(() => expect(screen.getByText('Auto-lock Timeout')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /Save Settings/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('update_vault_settings', { settings: mockSettings });
    });
  });

  it('shows success message after saving settings', async () => {
    mockInvoke
      .mockResolvedValueOnce(mockSettings as any)
      .mockResolvedValueOnce(undefined);

    render(<VaultSettings />);
    await waitFor(() => expect(screen.getByText('Auto-lock Timeout')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /Save Settings/i }));

    await waitFor(() => {
      expect(screen.getByText(/Settings saved successfully/i)).toBeInTheDocument();
    });
  });

  it('shows error when saving settings fails', async () => {
    mockInvoke
      .mockResolvedValueOnce(mockSettings as any)
      .mockRejectedValueOnce('Write error');

    render(<VaultSettings />);
    await waitFor(() => expect(screen.getByText('Auto-lock Timeout')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /Save Settings/i }));

    await waitFor(() => {
      expect(screen.getByText('Write error')).toBeInTheDocument();
    });
  });

  it('calls lock_vault when Lock Vault Now is clicked', async () => {
    mockInvoke
      .mockResolvedValueOnce(mockSettings as any)
      .mockResolvedValueOnce(undefined);

    render(<VaultSettings />);
    await waitFor(() => expect(screen.getByText('Auto-lock Timeout')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /Lock Vault Now/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('lock_vault');
    });
  });

  it('shows change password form when Change Master Password is clicked', async () => {
    render(<VaultSettings />);
    await waitFor(() => expect(screen.getByText('Auto-lock Timeout')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /Change Master Password/i }));

    expect(screen.getByPlaceholderText(/Enter current master password/i)).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/Enter new master password/i)).toBeInTheDocument();
  });

  it('shows validation error when change password fields are empty', async () => {
    render(<VaultSettings />);
    await waitFor(() => expect(screen.getByText('Auto-lock Timeout')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /Change Master Password/i }));
    fireEvent.click(screen.getByRole('button', { name: /^Change Password$/i }));

    await waitFor(() => {
      expect(screen.getByText(/All fields are required/i)).toBeInTheDocument();
    });
  });

  it('shows error when new passwords do not match', async () => {
    render(<VaultSettings />);
    await waitFor(() => expect(screen.getByText('Auto-lock Timeout')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /Change Master Password/i }));
    fireEvent.change(screen.getByPlaceholderText(/Enter current master password/i), { target: { value: 'OldPass123!' } });
    fireEvent.change(screen.getByPlaceholderText(/Enter new master password/i), { target: { value: 'NewPass123!' } });
    fireEvent.change(screen.getByPlaceholderText(/Confirm new master password/i), { target: { value: 'Different456!' } });
    fireEvent.click(screen.getByRole('button', { name: /^Change Password$/i }));

    await waitFor(() => {
      expect(screen.getByText(/New passwords do not match/i)).toBeInTheDocument();
    });
  });

  it('calls change_master_password with correct args on valid submission', async () => {
    mockInvoke
      .mockResolvedValueOnce(mockSettings as any)
      .mockResolvedValueOnce(undefined);

    render(<VaultSettings />);
    await waitFor(() => expect(screen.getByText('Auto-lock Timeout')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /Change Master Password/i }));
    fireEvent.change(screen.getByPlaceholderText(/Enter current master password/i), { target: { value: 'OldPass123!' } });
    fireEvent.change(screen.getByPlaceholderText(/Enter new master password/i), { target: { value: 'NewPass456!abcd' } });
    fireEvent.change(screen.getByPlaceholderText(/Confirm new master password/i), { target: { value: 'NewPass456!abcd' } });
    fireEvent.click(screen.getByRole('button', { name: /^Change Password$/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('change_master_password', {
        currentPassword: 'OldPass123!',
        newPassword: 'NewPass456!abcd',
      });
    });
  });
});
