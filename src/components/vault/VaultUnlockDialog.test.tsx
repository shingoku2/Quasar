import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import VaultUnlockDialog from './VaultUnlockDialog';
import '@testing-library/jest-dom';

const mockInvoke = vi.mocked(invoke);

describe('VaultUnlockDialog', () => {
  const onUnlocked = vi.fn();
  const onCancel = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the unlock form', () => {
    render(<VaultUnlockDialog onUnlocked={onUnlocked} />);

    // "Unlock Vault" appears in both the h2 and the submit button
    expect(screen.getAllByText('Unlock Vault').length).toBeGreaterThanOrEqual(2);
    expect(screen.getByLabelText(/Master Password/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Unlock Vault/i })).toBeInTheDocument();
  });

  it('shows error when submitting with empty password', async () => {
    render(<VaultUnlockDialog onUnlocked={onUnlocked} />);
    fireEvent.submit(screen.getByRole('button', { name: /Unlock Vault/i }).closest('form')!);

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent(/required/i);
    });
    expect(onUnlocked).not.toHaveBeenCalled();
  });

  it('calls unlock_vault and onUnlocked on successful submission', async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    render(<VaultUnlockDialog onUnlocked={onUnlocked} />);

    fireEvent.change(screen.getByLabelText(/Master Password/i), { target: { value: 'mypassword' } });
    fireEvent.submit(screen.getByRole('button', { name: /Unlock Vault/i }).closest('form')!);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('unlock_vault', { masterPassword: 'mypassword' });
      expect(onUnlocked).toHaveBeenCalledOnce();
    });
  });

  it('shows error from invoke failure', async () => {
    mockInvoke.mockRejectedValueOnce('Invalid master password');
    render(<VaultUnlockDialog onUnlocked={onUnlocked} />);

    fireEvent.change(screen.getByLabelText(/Master Password/i), { target: { value: 'wrongpassword' } });
    fireEvent.submit(screen.getByRole('button', { name: /Unlock Vault/i }).closest('form')!);

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent('Invalid master password');
    });
    expect(onUnlocked).not.toHaveBeenCalled();
  });

  it('hides cancel button when onCancel is not provided', () => {
    render(<VaultUnlockDialog onUnlocked={onUnlocked} />);
    expect(screen.queryByRole('button', { name: /Cancel/i })).not.toBeInTheDocument();
  });

  it('shows and calls cancel button when onCancel is provided', () => {
    render(<VaultUnlockDialog onUnlocked={onUnlocked} onCancel={onCancel} />);
    const cancelBtn = screen.getByRole('button', { name: /Cancel/i });
    expect(cancelBtn).toBeInTheDocument();
    fireEvent.click(cancelBtn);
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it('disables the form while unlocking', async () => {
    let resolveInvoke!: () => void;
    mockInvoke.mockReturnValueOnce(new Promise<void>((res) => { resolveInvoke = res; }));
    render(<VaultUnlockDialog onUnlocked={onUnlocked} />);

    fireEvent.change(screen.getByLabelText(/Master Password/i), { target: { value: 'somepassword' } });
    fireEvent.submit(screen.getByRole('button', { name: /Unlock Vault/i }).closest('form')!);

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /Unlocking/i })).toBeDisabled();
    });

    resolveInvoke();
  });

  it('toggles password visibility', () => {
    render(<VaultUnlockDialog onUnlocked={onUnlocked} />);
    const input = screen.getByLabelText(/Master Password/i);
    expect(input).toHaveAttribute('type', 'password');

    // The toggle button is the one that is NOT submit and NOT cancel
    const toggleBtn = screen.getAllByRole('button').find(
      (btn) => !btn.textContent?.match(/Unlock|Cancel/)
    )!;
    fireEvent.click(toggleBtn);
    expect(input).toHaveAttribute('type', 'text');

    fireEvent.click(toggleBtn);
    expect(input).toHaveAttribute('type', 'password');
  });
});
