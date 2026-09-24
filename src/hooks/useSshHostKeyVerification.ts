import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface HostKeyPromptData {
  host: string;
  port: number;
  fingerprint: string;
  keyType: string;
  isChanged: boolean;
  oldFingerprint?: string;
}

interface SshHostKeyEvent {
  requestId: string;
  host: string;
  port: number;
  fingerprint: string;
  keyType: string;
  keyBytes: number[];
  status: string;
  message: string;
  /** Previously stored fingerprint when the key changed (null otherwise). */
  oldFingerprint?: string | null;
}

interface PendingPromptEntry {
  requestId: string;
  prompt: HostKeyPromptData;
  onTrust: (permanent: boolean) => Promise<boolean>;
  onReject: () => Promise<boolean>;
}

const isDismissiblePromptError = (err: unknown) => {
  const message = err instanceof Error ? err.message : String(err);
  return message.includes('no longer pending')
    || message.includes('stopped waiting for host key approval');
};

export const useSshHostKeyVerification = () => {
  const [pendingPrompts, setPendingPrompts] = useState<PendingPromptEntry[]>([]);
  const actionInFlightRequestIdRef = useRef<string | null>(null);
  const promptData = pendingPrompts[0]?.prompt ?? null;

  // Listen for host key verification events from backend
  useEffect(() => {
    // listen() resolves asynchronously; if cleanup runs first, unlisten as soon as it
    // resolves instead of leaking a listener (FE-007: StrictMode used to register two,
    // so every host-key event queued two prompts in dev).
    const unlistenPromise = listen<SshHostKeyEvent>('ssh-host-key-verification', (event) => {
      const { requestId, host, port, fingerprint, keyType, status, oldFingerprint } = event.payload;
      const isChanged = status.toLowerCase().includes('changed');
      const prompt: HostKeyPromptData = {
        host,
        port,
        fingerprint,
        keyType,
        isChanged,
        oldFingerprint: isChanged ? oldFingerprint ?? undefined : undefined,
      };

      setPendingPrompts(prev => [
        ...prev,
        {
          requestId,
          prompt,
          onTrust: async (permanent: boolean) => {
            try {
              if (permanent) {
                // The backend persists the key this handshake presented; the webview
                // only names the pending request (IPC-002).
                await invoke('trust_ssh_host_key', { requestId });
              } else {
                await invoke('respond_ssh_host_key_verification', {
                  requestId,
                  accepted: true,
                });
              }
              return true;
            } catch (err) {
              console.error('Failed to approve host key:', err);
              return isDismissiblePromptError(err);
            }
          },
          onReject: async () => {
            try {
              await invoke('respond_ssh_host_key_verification', {
                requestId,
                accepted: false,
              });
              return true;
            } catch (err) {
              console.error('Failed to reject host key:', err);
              return isDismissiblePromptError(err);
            }
          },
        },
      ]);
    });

    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  const completeCurrentPrompt = async (
    action: (entry: PendingPromptEntry) => Promise<boolean>
  ) => {
    const current = pendingPrompts[0];
    if (!current || actionInFlightRequestIdRef.current === current.requestId) {
      return;
    }

    actionInFlightRequestIdRef.current = current.requestId;
    const handled = await action(current);
    if (handled) {
      setPendingPrompts(prev =>
        prev[0]?.requestId === current.requestId ? prev.slice(1) : prev
      );
    }
    if (actionInFlightRequestIdRef.current === current.requestId) {
      actionInFlightRequestIdRef.current = null;
    }
  };

  const handleTrust = (permanent: boolean) => {
    void completeCurrentPrompt((current) => current.onTrust(permanent));
  };

  const handleReject = () => {
    void completeCurrentPrompt((current) => current.onReject());
  };

  return {
    promptData,
    handleTrust,
    handleReject,
  };
};
