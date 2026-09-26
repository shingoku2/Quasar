import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { REVEAL_TIMEOUT_MS, useRevealedPassword } from './useRevealedPassword';

const vaultState = { isVaultLocked: false };
vi.mock('../VaultProvider', () => ({ useOptionalVault: () => vaultState }));

describe('useRevealedPassword (FE-009)', () => {
  const originalClipboard = navigator.clipboard;
  let clipboard: string;
  const writeText = vi.fn(async (t: string) => { clipboard = t; });
  const readText = vi.fn(async () => clipboard);

  beforeEach(() => {
    vi.useFakeTimers();
    vaultState.isVaultLocked = false;
    clipboard = '';
    writeText.mockClear();
    readText.mockClear();
    Object.defineProperty(navigator, 'clipboard', { value: { writeText, readText }, configurable: true });
    vi.mocked(invoke).mockResolvedValue('secret123');
  });
  afterEach(() => {
    vi.useRealTimers();
    Object.defineProperty(navigator, 'clipboard', { value: originalClipboard, configurable: true });
  });

  const reveal = async () => {
    const hook = renderHook(() => useRevealedPassword('c1'));
    await act(() => hook.result.current.toggle());
    expect(hook.result.current.password).toBe('secret123');
    return hook;
  };

  it('hides the password as soon as the vault locks', async () => {
    const hook = await reveal();
    vaultState.isVaultLocked = true;
    hook.rerender();
    expect(hook.result.current.password).toBeNull();
  });

  it('hides the password after the timeout', async () => {
    const hook = await reveal();
    act(() => { vi.advanceTimersByTime(REVEAL_TIMEOUT_MS); });
    expect(hook.result.current.password).toBeNull();
  });

  it('wipes a copied password from the clipboard after the timeout', async () => {
    const hook = await reveal();
    await act(() => hook.result.current.copy());
    expect(clipboard).toBe('secret123');
    await act(async () => { vi.advanceTimersByTime(REVEAL_TIMEOUT_MS); });
    expect(clipboard).toBe('');
  });

  it('leaves the clipboard alone if the user copied something else since', async () => {
    const hook = await reveal();
    await act(() => hook.result.current.copy());
    clipboard = 'something else';
    await act(async () => { vi.advanceTimersByTime(REVEAL_TIMEOUT_MS); });
    expect(clipboard).toBe('something else');
  });

  it('wipes the clipboard at once when the vault locks', async () => {
    const hook = await reveal();
    await act(() => hook.result.current.copy());
    vaultState.isVaultLocked = true;
    await act(async () => { hook.rerender(); });
    expect(clipboard).toBe('');
  });

  it('wipes the clipboard when it cannot be read', async () => {
    readText.mockRejectedValueOnce(new Error('denied'));
    const hook = await reveal();
    await act(() => hook.result.current.copy());
    await act(async () => { vi.advanceTimersByTime(REVEAL_TIMEOUT_MS); });
    expect(clipboard).toBe('');
  });
});
