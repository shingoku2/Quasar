import React from 'react';
import SshHostKeyPrompt from './SshHostKeyPrompt';
import { useSshHostKeyVerification } from '../../hooks/useSshHostKeyVerification';

/**
 * Shows SSH host-key prompts above every view. It used to live inside RemoteManager, so a
 * prompt raised while another view was active (e.g. starting a tunnel) was invisible until
 * the handshake timed out (FE-011). Mount exactly once, in the app layout.
 */
export const HostKeyPromptHost: React.FC = () => {
  const { promptData, promptRequestId, handleTrust, handleReject } = useSshHostKeyVerification();
  if (!promptData) return null;
  return (
    // Keyed by request: each queued prompt gets fresh "I verified" / permanent-trust state,
    // so acknowledging one changed key can't pre-acknowledge the next (PR #68 review).
    <SshHostKeyPrompt
      key={promptRequestId ?? undefined}
      host={promptData.host}
      port={promptData.port}
      fingerprint={promptData.fingerprint}
      keyType={promptData.keyType}
      isChanged={promptData.isChanged}
      oldFingerprint={promptData.oldFingerprint}
      onTrust={handleTrust}
      onReject={handleReject}
    />
  );
};
