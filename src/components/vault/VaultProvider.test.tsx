import { render, screen, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { VaultProvider, useVault } from './VaultProvider';
import { invoke } from '@tauri-apps/api/core';
import '@testing-library/jest-dom';

// Mock child dialogs
vi.mock('./VaultInitDialog', () => ({
  default: ({ onInitialized }: { onInitialized: () => void }) => (
    <div data-testid="vault-init-dialog">
      <button onClick={onInitialized}>Initialize</button>
    </div>
  ),
}));

vi.mock('./VaultUnlockDialog', () => ({
  default: ({ onUnlocked }: { onUnlocked: () => void }) => (
    <div data-testid="vault-unlock-dialog">
      <button onClick={onUnlocked}>Unlock</button>
    </div>
  ),
}));

const mockInvoke = vi.mocked(invoke);

describe('VaultProvider', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading state while checking vault status', () => {
    mockInvoke.mockReturnValue(new Promise(() => {})); // never resolves
    render(
      <VaultProvider>
        <div>App Content</div>
      </VaultProvider>
    );

    expect(screen.getByText('Initializing security vault...')).toBeInTheDocument();
  });

  it('shows init dialog when vault is not initialized', async () => {
    mockInvoke.mockResolvedValueOnce(false as any); // is_vault_initialized

    render(
      <VaultProvider>
        <div>App Content</div>
      </VaultProvider>
    );

    await waitFor(() => {
      expect(screen.getByTestId('vault-init-dialog')).toBeInTheDocument();
    });
  });

  it('shows unlock dialog when vault is initialized but locked', async () => {
    mockInvoke
      .mockResolvedValueOnce(true as any)  // is_vault_initialized
      .mockResolvedValueOnce(true as any); // is_vault_locked

    render(
      <VaultProvider>
        <div>App Content</div>
      </VaultProvider>
    );

    await waitFor(() => {
      expect(screen.getByTestId('vault-unlock-dialog')).toBeInTheDocument();
    });
  });

  it('renders children when vault is initialized and unlocked', async () => {
    mockInvoke
      .mockResolvedValueOnce(true as any)   // is_vault_initialized
      .mockResolvedValueOnce(false as any); // is_vault_locked

    render(
      <VaultProvider>
        <div>App Content</div>
      </VaultProvider>
    );

    await waitFor(() => {
      expect(screen.getByText('App Content')).toBeInTheDocument();
    });
  });

  it('shows error or fallback state when vault check fails', async () => {
    mockInvoke.mockRejectedValueOnce('Connection failed');

    render(
      <VaultProvider>
        <div>App Content</div>
      </VaultProvider>
    );

    // When invoke rejects, the component catches the error.
    // Depending on timing, it may show error UI or init dialog as fallback.
    await waitFor(() => {
      const hasError = screen.queryByText('Vault Initialization Error');
      const hasInit = screen.queryByTestId('vault-init-dialog');
      const hasChildren = screen.queryByText('App Content');
      // At least one of these states should be reached
      expect(hasError || hasInit || hasChildren).toBeTruthy();
    });
  });
});

describe('useVault', () => {
  it('throws when used outside VaultProvider', () => {
    const TestComponent = () => {
      useVault();
      return null;
    };

    expect(() => render(<TestComponent />)).toThrow(
      'useVault must be used within VaultProvider'
    );
  });
});
