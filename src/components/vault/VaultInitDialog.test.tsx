import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import VaultInitDialog from './VaultInitDialog';
import '@testing-library/jest-dom';

const mockInvoke = vi.mocked(invoke);

describe('VaultInitDialog', () => {
  const onInitialized = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  const fillPassword = (password: string, confirm?: string) => {
    fireEvent.change(screen.getByLabelText(/Master Password/i), { target: { value: password } });
    fireEvent.change(screen.getByLabelText(/Confirm Password/i), { target: { value: confirm ?? password } });
  };

  it('renders the initialization form', () => {
    render(<VaultInitDialog onInitialized={onInitialized} />);

    // "Initialize Vault" appears in both the h2 and the submit button
    expect(screen.getAllByText('Initialize Vault').length).toBeGreaterThanOrEqual(2);
    expect(screen.getByLabelText(/Master Password/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/Confirm Password/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Initialize Vault/i })).toBeInTheDocument();
  });

  it('shows error when password is shorter than 12 characters', async () => {
    render(<VaultInitDialog onInitialized={onInitialized} />);
    fillPassword('Short1A');
    fireEvent.submit(screen.getByRole('button', { name: /Initialize Vault/i }).closest('form')!);

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent(/at least 12 characters/i);
    });
    expect(onInitialized).not.toHaveBeenCalled();
  });

  it('shows error when password has no uppercase letter', async () => {
    render(<VaultInitDialog onInitialized={onInitialized} />);
    fillPassword('alllowercase123');
    fireEvent.submit(screen.getByRole('button', { name: /Initialize Vault/i }).closest('form')!);

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent(/uppercase/i);
    });
  });

  it('shows error when password has no lowercase letter', async () => {
    render(<VaultInitDialog onInitialized={onInitialized} />);
    fillPassword('ALLUPPERCASE123');
    fireEvent.submit(screen.getByRole('button', { name: /Initialize Vault/i }).closest('form')!);

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent(/lowercase/i);
    });
  });

  it('shows error when password has no number', async () => {
    render(<VaultInitDialog onInitialized={onInitialized} />);
    fillPassword('NoNumbersHereAtAll');
    fireEvent.submit(screen.getByRole('button', { name: /Initialize Vault/i }).closest('form')!);

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent(/number/i);
    });
  });

  it('shows error when passwords do not match', async () => {
    render(<VaultInitDialog onInitialized={onInitialized} />);
    fillPassword('ValidPass123!', 'DifferentPass456!');
    fireEvent.submit(screen.getByRole('button', { name: /Initialize Vault/i }).closest('form')!);

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent(/do not match/i);
    });
  });

  it('calls initialize_vault and onInitialized on valid submission', async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    render(<VaultInitDialog onInitialized={onInitialized} />);
    fillPassword('ValidPassword123!');
    fireEvent.submit(screen.getByRole('button', { name: /Initialize Vault/i }).closest('form')!);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('initialize_vault', { masterPassword: 'ValidPassword123!' });
      expect(onInitialized).toHaveBeenCalledOnce();
    });
  });

  it('shows error message returned from invoke failure', async () => {
    mockInvoke.mockRejectedValueOnce('Vault already initialized');
    render(<VaultInitDialog onInitialized={onInitialized} />);
    fillPassword('ValidPassword123!');
    fireEvent.submit(screen.getByRole('button', { name: /Initialize Vault/i }).closest('form')!);

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent('Vault already initialized');
    });
    expect(onInitialized).not.toHaveBeenCalled();
  });

  it('shows password strength indicator when typing', () => {
    render(<VaultInitDialog onInitialized={onInitialized} />);
    fireEvent.change(screen.getByLabelText(/Master Password/i), { target: { value: 'Short' } });
    expect(screen.getByText('Password Strength')).toBeInTheDocument();
    expect(screen.getByText('Weak')).toBeInTheDocument();
  });

  it('shows Strong strength for a complex password', () => {
    render(<VaultInitDialog onInitialized={onInitialized} />);
    fireEvent.change(screen.getByLabelText(/Master Password/i), { target: { value: 'Very$ecurePass123!' } });
    expect(screen.getByText('Strong')).toBeInTheDocument();
  });

  it('shows initializing state while submitting', async () => {
    let resolveInvoke!: () => void;
    mockInvoke.mockReturnValueOnce(new Promise<void>((res) => { resolveInvoke = res; }));
    render(<VaultInitDialog onInitialized={onInitialized} />);
    fillPassword('ValidPassword123!');
    fireEvent.submit(screen.getByRole('button', { name: /Initialize Vault/i }).closest('form')!);

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /Initializing/i })).toBeDisabled();
    });

    resolveInvoke();
  });
});
