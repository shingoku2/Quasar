import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface HostKeyVerificationResult {
  allowed: boolean;
  status: string;
  fingerprint: string;
  message: string;
}

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
}

interface PendingPromptEntry {
  prompt: HostKeyPromptData;
  onTrust: (permanent: boolean) => Promise<void>;
  onReject: () => Promise<void>;
}

export const useSshHostKeyVerification = () => {
  const [pendingPrompts, setPendingPrompts] = useState<PendingPromptEntry[]>([]);
  const promptData = pendingPrompts[0]?.prompt ?? null;

  // Listen for host key verification events from backend
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      unlisten = await listen<SshHostKeyEvent>('ssh-host-key-verification', (event) => {
        const { requestId, host, port, fingerprint, keyType, keyBytes, status, message } = event.payload;
        const isChanged = status.toLowerCase().includes('changed');
        const prompt: HostKeyPromptData = {
          host,
          port,
          fingerprint,
          keyType,
          isChanged,
          oldFingerprint: isChanged ? message : undefined,
        };

        setPendingPrompts(prev => [
          ...prev,
          {
            prompt,
            onTrust: async (permanent: boolean) => {
              try {
                if (permanent) {
                  await invoke('trust_ssh_host_key', {
                    host,
                    port,
                    fingerprint,
                    keyType,
                    keyBytes,
                    trustStatus: 'trusted',
                    requestId,
                  });
                } else {
                  await invoke('respond_ssh_host_key_verification', {
                    requestId,
                    accepted: true,
                  });
                }
              } catch (err) {
                console.error('Failed to approve host key:', err);
              }
            },
            onReject: async () => {
              try {
                await invoke('respond_ssh_host_key_verification', {
                  requestId,
                  accepted: false,
                });
              } catch (err) {
                console.error('Failed to reject host key:', err);
              }
            },
          },
        ]);
      });
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const verifyHostKey = async (
    host: string,
    port: number,
    fingerprint: string,
    keyType: string,
    keyBytes: number[]
  ): Promise<boolean> => {
    return new Promise((resolve) => {
      invoke<HostKeyVerificationResult>('verify_ssh_host_key', {
        host,
        port,
        fingerprint,
        keyType,
      })
        .then((result) => {
          if (result.allowed) {
            resolve(true);
          } else {
            const isChanged = result.status.toLowerCase() === 'changed';
            const prompt: HostKeyPromptData = {
              host,
              port,
              fingerprint,
              keyType,
              isChanged,
              oldFingerprint: isChanged ? result.message : undefined,
            };
            setPendingPrompts(prev => [
              ...prev,
              {
                prompt,
                onTrust: async (permanent: boolean) => {
                  if (permanent) {
                    try {
                      await invoke('trust_ssh_host_key', {
                        host,
                        port,
                        fingerprint,
                        keyType,
                        keyBytes,
                        trustStatus: 'trusted',
                      });
                    } catch (err) {
                      console.error('Failed to trust host key:', err);
                    }
                  }
                  resolve(true);
                },
                onReject: async () => {
                  resolve(false);
                },
              },
            ]);
          }
        })
        .catch((err) => {
          console.error('Host key verification failed:', err);
          resolve(false);
        });
    });
  };

  const handleTrust = (permanent: boolean) => {
    const current = pendingPrompts[0];
    if (!current) return;
    void current.onTrust(permanent).finally(() => {
      setPendingPrompts(prev => prev.slice(1));
    });
  };

  const handleReject = () => {
    const current = pendingPrompts[0];
    if (!current) return;
    void current.onReject().finally(() => {
      setPendingPrompts(prev => prev.slice(1));
    });
  };

  return {
    promptData,
    verifyHostKey,
    handleTrust,
    handleReject,
  };
};
