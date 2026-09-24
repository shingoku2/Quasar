import React from 'react';
import SshHostKeyPrompt from './SshHostKeyPrompt';
import { useSshHostKeyVerification } from '../../hooks/useSshHostKeyVerification';

/**
 * Shows SSH host-key prompts above every view. It used to live inside RemoteManager, so a
 * prompt raised while another view was active (e.g. starting a tunnel) was invisible until
 * the handshake timed out (FE-011). Mount exactly once, in the app layout.
 */
export const HostKeyPromptHost: React.FC = () => {
  const { promptData, handleTrust, handleReject } = useSshHostKeyVerification();
  if (!promptData) return null;
  return (
    <SshHostKeyPrompt
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
