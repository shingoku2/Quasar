import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useOptionalVault } from '../VaultProvider';

/** How long a revealed password stays on screen, and a copied one on the clipboard. */
export const REVEAL_TIMEOUT_MS = 30_000;

/**
 * Wipes the clipboard, but only if it still holds `password`: text the user copied
 * since is left alone. If the clipboard can't be read, it is wiped anyway (a leaked
 * password is worse than a lost clipboard).
 */
async function clearClipboardIfUnchanged(password: string): Promise<void> {
  try {
    const current = await navigator.clipboard.readText();
    if (current !== password) return;
  } catch {
    // Unreadable: fall through and clear.
  }
  await navigator.clipboard.writeText('').catch(() => {});
}

/**
 * A credential's password, fetched only on an explicit reveal (native confirm, audited
 * in the backend). It hides itself after REVEAL_TIMEOUT_MS and as soon as the vault
 * locks; a copy is wiped from the clipboard after the same time, or at once on lock.
 */
export function useRevealedPassword(credentialId: string) {
  const isVaultLocked = useOptionalVault()?.isVaultLocked ?? false;
  const [password, setPassword] = useState<string | null>(null);
  const [isRevealing, setIsRevealing] = useState(false);
  const [copied, setCopied] = useState(false);
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const clipboardTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const copiedPassword = useRef<string | null>(null);

  const hide = useCallback(() => {
    if (hideTimer.current !== null) clearTimeout(hideTimer.current);
    hideTimer.current = null;
    setPassword(null);
  }, []);

  const flushClipboard = useCallback(() => {
    if (clipboardTimer.current !== null) clearTimeout(clipboardTimer.current);
    clipboardTimer.current = null;
    const pwd = copiedPassword.current;
    copiedPassword.current = null;
    if (pwd !== null) void clearClipboardIfUnchanged(pwd);
  }, []);

  useEffect(() => {
    if (!isVaultLocked) return;
    hide();
    flushClipboard();
  }, [isVaultLocked, hide, flushClipboard]);

  // The on-screen copy goes with the dialog; the clipboard wipe still runs on schedule.
  useEffect(() => () => {
    if (hideTimer.current !== null) clearTimeout(hideTimer.current);
  }, []);

  const toggle = async () => {
    if (password !== null) {
      hide();
      return;
    }
    setIsRevealing(true);
    try {
      const pwd = await invoke<string>('reveal_credential_password', { credentialId });
      setPassword(pwd);
      hideTimer.current = setTimeout(hide, REVEAL_TIMEOUT_MS);
    } catch (err) {
      console.error('Failed to reveal password', err);
    } finally {
      setIsRevealing(false);
    }
  };

  // Copy works only after an explicit reveal. Revealing needs a native confirmation, and
  // waiting on that dialog would outlive the click's user gesture, which WebKit requires
  // for clipboard writes, so a reveal-then-copy in one click failed silently.
  const copy = async () => {
    if (password === null) return;
    try {
      await navigator.clipboard.writeText(password);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
      if (clipboardTimer.current !== null) clearTimeout(clipboardTimer.current);
      copiedPassword.current = password;
      clipboardTimer.current = setTimeout(flushClipboard, REVEAL_TIMEOUT_MS);
    } catch (err) {
      console.error('Failed to copy password', err);
    }
  };

  return { password, isRevealing, copied, toggle, copy };
}
