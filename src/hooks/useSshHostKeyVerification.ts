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
  host: string;
  port: number;
  fingerprint: string;
  keyType: string;
  keyBytes: number[];
  status: string;
  message: string;
}

export const useSshHostKeyVerification = () => {
  const [promptData, setPromptData] = useState<HostKeyPromptData | null>(null);
  const [onTrustCallback, setOnTrustCallback] = useState<((permanent: boolean) => void) | null>(null);
  const [onRejectCallback, setOnRejectCallback] = useState<(() => void) | null>(null);

  // Listen for host key verification events from backend
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      unlisten = await listen<SshHostKeyEvent>('ssh-host-key-verification', (event) => {
        const { host, port, fingerprint, keyType, keyBytes, status, message } = event.payload;
        const isChanged = status.toLowerCase().includes('changed');

        setPromptData({
          host,
          port,
          fingerprint,
          keyType,
          isChanged,
          oldFingerprint: isChanged ? message : undefined,
        });

        // Set up trust callback
        setOnTrustCallback(() => async (permanent: boolean) => {
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
          setPromptData(null);
        });

        // Set up reject callback
        setOnRejectCallback(() => () => {
          setPromptData(null);
        });
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
            
            setPromptData({
              host,
              port,
              fingerprint,
              keyType,
              isChanged,
              oldFingerprint: isChanged ? result.message : undefined,
            });

            setOnTrustCallback(() => async (permanent: boolean) => {
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
              setPromptData(null);
              resolve(true);
            });

            setOnRejectCallback(() => () => {
              setPromptData(null);
              resolve(false);
            });
          }
        })
        .catch((err) => {
          console.error('Host key verification failed:', err);
          resolve(false);
        });
    });
  };

  const handleTrust = (permanent: boolean) => {
    if (onTrustCallback) {
      onTrustCallback(permanent);
    }
  };

  const handleReject = () => {
    if (onRejectCallback) {
      onRejectCallback();
    }
  };

  return {
    promptData,
    verifyHostKey,
    handleTrust,
    handleReject,
  };
};
