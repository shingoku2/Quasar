import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useOptionalVault } from '../VaultProvider';
import { copySecret } from '../../../lib/copiedSecret';

/** How long a revealed password stays on screen, and a copied one on the clipboard. */
export const REVEAL_TIMEOUT_MS = 30_000;

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

  const hide = useCallback(() => {
    if (hideTimer.current !== null) clearTimeout(hideTimer.current);
    hideTimer.current = null;
    setPassword(null);
  }, []);

  // The clipboard wipe on lock is VaultProvider's (flushCopiedSecret): it must run even
  // after this dialog has closed.
  useEffect(() => {
    if (isVaultLocked) hide();
  }, [isVaultLocked, hide]);

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
      await copySecret(password);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy password', err);
    }
  };

  return { password, isRevealing, copied, toggle, copy };
}
