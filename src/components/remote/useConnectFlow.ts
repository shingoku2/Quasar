import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Host } from '../HostList';
import { findTailscalePeer, useTailscaleStatus } from '../../hooks/useTailscaleStatus';

export type ConnectMode = 'ssh' | 'sftp';

interface SelectedCredential {
  id: string;
  username: string;
}

interface ManualEntryOptions {
  saveCredential: boolean;
  credentialName?: string;
}

/** Opens a session tab (or tells the user why it can't). */
export type OpenSession = (
  mode: ConnectMode,
  host: Host,
  password?: string,
  username?: string,
  credentialId?: string,
) => void;

/**
 * The SSH/SFTP connect flow: an unlocked vault offers its credentials first (by id, so
 * the password never enters the webview); a locked vault, or "enter manually", asks for
 * a username and password, optionally saving them to the vault.
 */
export function useConnectFlow(openSession: OpenSession) {
  const [pendingHost, setPendingHost] = useState<Host | null>(null);
  const [pendingMode, setPendingMode] = useState<ConnectMode>('ssh');
  const [showCredentialSelector, setShowCredentialSelector] = useState(false);
  const [useManualEntry, setUseManualEntry] = useState(false);
  const [allowCredentialSave, setAllowCredentialSave] = useState(false);

  // Tailscale SSH peers can authenticate by tailnet identity alone, so the
  // manual credential prompt should not require a password for them.
  const { status: tailscaleStatus } = useTailscaleStatus();
  const pendingHostIsTailscaleSsh = pendingHost
    ? !!findTailscalePeer(tailscaleStatus, pendingHost.address)?.tailscale_ssh
    : false;

  const begin = async (host: Host, mode: ConnectMode) => {
    setPendingHost(host);
    setPendingMode(mode);
    setUseManualEntry(false);
    try {
      if (await invoke<boolean>('is_vault_locked')) {
        setAllowCredentialSave(false);
        setUseManualEntry(true);
      } else {
        setShowCredentialSelector(true);
      }
    } catch {
      setAllowCredentialSave(false);
      setUseManualEntry(true);
    }
  };

  const selectCredential = (credential: SelectedCredential) => {
    if (!pendingHost) return;
    const username = credential.username || pendingHost.username;
    if (!username) {
      setShowCredentialSelector(false);
      setUseManualEntry(true);
      return;
    }
    // SSH and SFTP both get the credential by id: the backend decrypts it, so the vault
    // password never crosses IPC (FE-001 / RSEC-013).
    openSession(pendingMode, pendingHost, undefined, username, credential.id);
    setShowCredentialSelector(false);
    setPendingHost(null);
  };

  const chooseManualEntry = () => {
    setShowCredentialSelector(false);
    setAllowCredentialSave(true);
    setUseManualEntry(true);
  };

  const cancelSelector = () => {
    setShowCredentialSelector(false);
    setPendingHost(null);
  };

  const finishManual = () => {
    setPendingHost(null);
    setUseManualEntry(false);
    setAllowCredentialSave(false);
  };

  const submitManual = async (username: string, password: string, options?: ManualEntryOptions) => {
    if (!pendingHost) return;
    openSession(pendingMode, pendingHost, password, username);

    if (options?.saveCredential) {
      const credentialName = options.credentialName || `${pendingHost.name} (${username})`;
      try {
        await invoke('add_credential', {
          name: credentialName,
          username,
          password,
          credentialType: 'ssh',
          host: pendingHost.address,
          port: pendingHost.port || 22,
          metadata: null,
        });
      } catch (error) {
        console.error('Failed to save credential to vault:', error);
        alert(`Connected, but failed to save credential: ${error}`);
      }
    }
    finishManual();
  };

  return {
    pendingHost,
    pendingMode,
    showCredentialSelector,
    useManualEntry,
    allowCredentialSave,
    pendingHostIsTailscaleSsh,
    begin,
    selectCredential,
    chooseManualEntry,
    cancelSelector,
    submitManual,
    cancelManual: finishManual,
  };
}
