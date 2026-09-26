import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { REVEAL_TIMEOUT_MS, useRevealedPassword } from './useRevealedPassword';
import { flushCopiedSecret } from '../../../lib/copiedSecret';

const vaultState = { isVaultLocked: false };
vi.mock('../VaultProvider', () => ({ useOptionalVault: () => vaultState }));

describe('useRevealedPassword (FE-009)', () => {
  const originalClipboard = navigator.clipboard;
  let clipboard: string;

  beforeEach(() => {
    vi.useFakeTimers();
    vaultState.isVaultLocked = false;
    clipboard = '';
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: vi.fn(async (t: string) => { clipboard = t; }), readText: vi.fn(async () => clipboard) },
      configurable: true,
    });
    vi.mocked(invoke).mockResolvedValue('secret123');
  });
  afterEach(() => {
    flushCopiedSecret();
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

  it('copies the revealed password and wipes it later, even after the dialog closes', async () => {
    const hook = await reveal();
    await act(() => hook.result.current.copy());
    expect(clipboard).toBe('secret123');
    hook.unmount();
    await vi.advanceTimersByTimeAsync(REVEAL_TIMEOUT_MS);
    expect(clipboard).toBe('');
  });
});
