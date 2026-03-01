import React, { useState } from 'react';
import { ShieldAlert, ShieldCheck, AlertTriangle, Copy, Check } from 'lucide-react';

interface SshHostKeyPromptProps {
  host: string;
  port: number;
  fingerprint: string;
  keyType: string;
  isChanged?: boolean;
  oldFingerprint?: string;
  onTrust: (permanent: boolean) => void;
  onReject: () => void;
}

const SshHostKeyPrompt: React.FC<SshHostKeyPromptProps> = ({
  host,
  port,
  fingerprint,
  keyType,
  isChanged = false,
  oldFingerprint,
  onTrust,
  onReject,
}) => {
  const [trustPermanently, setTrustPermanently] = useState(true);
  const [copied, setCopied] = useState(false);

  const copyFingerprint = async () => {
    await navigator.clipboard.writeText(fingerprint);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div role="dialog"
      aria-modal="true"
      className="fixed inset-0 z-100 flex items-center justify-center bg-black/90 backdrop-blur-md">
      <div className="bg-bg-sidebar border border-gray-700 rounded-xl shadow-2xl w-full max-w-2xl overflow-hidden animate-in fade-in zoom-in duration-200">
        <div className={`px-6 py-4 border-b border-gray-800 flex justify-between items-center ${isChanged ? 'bg-alert/10' : 'bg-bg-root/50'}`}>
          <div className="flex items-center space-x-2">
            {isChanged ? (
              <AlertTriangle className="h-5 w-5 text-alert" />
            ) : (
              <ShieldAlert className="h-5 w-5 text-warning" />
            )}
            <h2 className="text-base font-bold text-white uppercase tracking-wider">
              {isChanged ? 'Host Key Changed - Security Warning' : 'Unknown SSH Host'}
            </h2>
          </div>
        </div>

        <div className="p-6 space-y-5">
          {isChanged ? (
            <div className="bg-alert/10 border-2 border-alert rounded-lg p-4 space-y-3">
              <div className="flex items-start space-x-3">
                <AlertTriangle className="h-6 w-6 text-alert shrink-0 mt-0.5" />
                <div className="flex-1">
                  <p className="font-bold text-alert text-lg mb-2">Potential Security Risk Detected</p>
                  <p className="text-gray-300 text-sm leading-relaxed">
                    The host key for <span className="font-mono text-white">{host}:{port}</span> has changed since your last connection.
                    This could indicate:
                  </p>
                  <ul className="list-disc list-inside text-sm text-gray-300 mt-2 space-y-1 ml-4">
                    <li>The server was reinstalled or reconfigured</li>
                    <li>You are connecting to a different server</li>
                    <li className="text-alert font-medium">Someone is attempting a man-in-the-middle attack</li>
                  </ul>
                  <p className="text-gray-300 text-sm mt-3 font-medium">
                    Only proceed if you are certain this change is legitimate.
                  </p>
                </div>
              </div>
            </div>
          ) : (
            <div className="bg-warning/10 border border-warning/30 rounded-lg p-4">
              <div className="flex items-start space-x-3">
                <ShieldAlert className="h-5 w-5 text-warning shrink-0 mt-0.5" />
                <div>
                  <p className="text-gray-300 text-sm">
                    This is the first time connecting to <span className="font-mono text-white">{host}:{port}</span>.
                    Verify the host key fingerprint matches what you expect before proceeding.
                  </p>
                </div>
              </div>
            </div>
          )}

          <div className="space-y-4">
            <div>
              <label className="block text-xs font-medium text-gray-500 mb-2">Host</label>
              <p className="text-white font-mono text-sm bg-bg-root border border-gray-700 rounded-lg px-4 py-2">
                {host}:{port}
              </p>
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-500 mb-2">Key Type</label>
              <p className="text-white font-mono text-sm bg-bg-root border border-gray-700 rounded-lg px-4 py-2 uppercase">
                {keyType}
              </p>
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-500 mb-2">
                {isChanged ? 'New Fingerprint' : 'Fingerprint'}
              </label>
              <div className="flex items-center space-x-2">
                <div className="flex-1 bg-bg-root border border-gray-700 rounded-lg px-4 py-2">
                  <p className="text-white font-mono text-xs break-all">{fingerprint}</p>
                </div>
                <button
                  onClick={copyFingerprint}
                  className="bg-bg-root border border-gray-700 hover:border-accent text-gray-400 hover:text-accent px-3 py-2 rounded-lg transition-all"
                  title="Copy fingerprint"
                >
                  {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
                </button>
              </div>
            </div>

            {isChanged && oldFingerprint && (
              <div>
                <label className="block text-xs font-medium text-gray-500 mb-2">Previous Fingerprint</label>
                <div className="bg-bg-root border border-alert/30 rounded-lg px-4 py-2">
                  <p className="text-gray-400 font-mono text-xs break-all line-through">{oldFingerprint}</p>
                </div>
              </div>
            )}
          </div>

          <div className="bg-accent/10 border border-accent/30 rounded-lg p-4">
            <label className="flex items-start space-x-3 cursor-pointer">
              <input
                type="checkbox"
                checked={trustPermanently}
                onChange={(e) => setTrustPermanently(e.target.checked)}
                className="mt-1 h-4 w-4 rounded border-gray-600 bg-bg-root text-accent focus:ring-accent focus:ring-offset-0"
              />
              <div className="flex-1">
                <p className="text-white text-sm font-medium">Trust this host key permanently</p>
                <p className="text-gray-400 text-xs mt-1">
                  {trustPermanently 
                    ? 'The host key will be saved and verified on future connections'
                    : 'You will be prompted again on the next connection'}
                </p>
              </div>
            </label>
          </div>

          <div className="pt-2 flex space-x-3">
            <button
              onClick={() => onTrust(trustPermanently)}
              className={`flex-1 ${isChanged ? 'bg-alert hover:bg-alert/80' : 'bg-accent hover:bg-accent/80'} text-white py-3 rounded-lg text-sm font-bold transition-all shadow-lg flex items-center justify-center`}
            >
              <ShieldCheck className="h-4 w-4 mr-2" />
              {isChanged ? 'Accept New Key & Connect' : 'Trust & Connect'}
            </button>
            <button
              onClick={onReject}
              className="flex-1 bg-bg-root border border-gray-700 hover:border-alert text-white py-3 rounded-lg text-sm font-bold transition-all"
            >
              Cancel Connection
            </button>
          </div>

          <p className="text-xs text-gray-500 text-center">
            Never trust a host key unless you can verify it through a secure channel
          </p>
        </div>
      </div>
    </div>
  );
};

export default SshHostKeyPrompt;
