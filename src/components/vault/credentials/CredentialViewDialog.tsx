import React, { useState } from 'react';
import { Eye, EyeOff, Key, X } from 'lucide-react';
import { useModalDialog } from '../../../hooks/useModalDialog';
import type { Credential } from './types';
import { useRevealedPassword } from './useRevealedPassword';

const CredentialViewDialog: React.FC<{
  credential: Credential;
  onClose: () => void;
}> = ({ credential, onClose }) => {
  const dialogRef = useModalDialog(onClose);
  const [copied, setCopied] = useState(false);
  const revealed = useRevealedPassword(credential.id);

  const copyToClipboard = async (text: string) => {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="fixed inset-0 z-100 flex items-center justify-center bg-black/80 backdrop-blur-md" ref={dialogRef} tabIndex={-1} role="dialog" aria-modal="true" aria-label="View Credential">
      <div className="bg-bg-sidebar border border-gray-700 rounded-xl shadow-2xl w-full max-w-md overflow-hidden animate-in fade-in zoom-in duration-200">
        <div className="px-6 py-4 border-b border-gray-800 flex justify-between items-center bg-bg-root/50">
          <div className="flex items-center space-x-2">
            <Key className="h-4 w-4 text-accent" />
            <h2 className="text-sm font-bold text-white uppercase tracking-wider">View Credential</h2>
          </div>
          <button onClick={onClose} aria-label="Close dialog" className="text-gray-500 hover:text-white transition-colors">
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="p-6 space-y-4">
          <div>
            <div className="block text-xs font-medium text-gray-500 mb-1">Name</div>
            <p className="text-white font-medium">{credential.name}</p>
          </div>

          <div>
            <div className="block text-xs font-medium text-gray-500 mb-1">Type</div>
            <p className="text-white font-mono uppercase text-sm">{credential.credential_type}</p>
          </div>

          {credential.host && (
            <div>
              <div className="block text-xs font-medium text-gray-500 mb-1">Host</div>
              <p className="text-white font-mono text-sm">
                {credential.host}{credential.port ? `:${credential.port}` : ''}
              </p>
            </div>
          )}

          <div>
            <div className="block text-xs font-medium text-gray-500 mb-1">Username</div>
            <div className="flex items-center justify-between bg-bg-root border border-gray-700 rounded-lg px-4 py-2">
              <p className="text-white font-mono text-sm">{credential.username}</p>
              <button
                onClick={() => copyToClipboard(credential.username)}
                className="text-gray-400 hover:text-accent transition-colors text-xs"
              >
                {copied ? 'Copied!' : 'Copy'}
              </button>
            </div>
          </div>

          {credential.has_password && (
            <div>
              <div className="block text-xs font-medium text-gray-500 mb-1">Password</div>
              <div className="flex items-center justify-between bg-bg-root border border-gray-700 rounded-lg px-4 py-2">
                <p className="text-white font-mono text-sm flex-1 truncate">
                  {revealed.password ?? '••••••••••••'}
                </p>
                <div className="flex items-center space-x-2">
                  <button
                    onClick={revealed.toggle}
                    disabled={revealed.isRevealing}
                    aria-label="Toggle password visibility"
                    className="text-gray-400 hover:text-accent transition-colors disabled:opacity-50"
                  >
                    {revealed.password !== null ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                  </button>
                  <button
                    onClick={revealed.copy}
                    disabled={revealed.password === null}
                    aria-label="Copy password"
                    title={revealed.password === null ? 'Reveal the password first' : undefined}
                    className="text-gray-400 hover:text-accent transition-colors text-xs disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    {revealed.copied ? 'Copied!' : 'Copy'}
                  </button>
                </div>
              </div>
            </div>
          )}

          <button
            onClick={onClose}
            className="w-full bg-bg-root border border-gray-700 hover:border-accent text-white py-2.5 rounded-lg text-sm font-medium transition-all"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
};

export default CredentialViewDialog;
