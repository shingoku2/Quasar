import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Edit2, Eye, EyeOff, Lock, Plus, Save, User, X } from 'lucide-react';
import { useModalDialog } from '../../../hooks/useModalDialog';
import { getErrorMessage, isUserCancelled } from '../../../lib/utils';
import { buildCredentialPayload, initialCredentialForm } from './credentialPayload';
import type { Credential, CredentialFormData } from './types';

const CredentialFormDialog: React.FC<{
  credential?: Credential;
  onClose: () => void;
  onSaved: () => void;
}> = ({ credential, onClose, onSaved }) => {
  const dialogRef = useModalDialog(onClose);
  const [formData, setFormData] = useState<CredentialFormData>(() => initialCredentialForm(credential));
  const [showPassword, setShowPassword] = useState(false);
  const [showPassphrase, setShowPassphrase] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState('');

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    const payload = buildCredentialPayload(formData, credential);
    if (!payload.ok) {
      setError(payload.error);
      return;
    }
    setIsSaving(true);
    try {
      await invoke(payload.command, payload.args);
      onSaved();
    } catch (err) {
      // Declining the native host-change confirmation keeps the form open unchanged.
      if (!isUserCancelled(err)) setError(getErrorMessage(err, 'Failed to save credential'));
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-100 flex items-center justify-center bg-black/80 backdrop-blur-md" ref={dialogRef} tabIndex={-1} role="dialog" aria-modal="true" aria-label={credential ? 'Edit Credential' : 'Add Credential'}>
      <div className="bg-bg-sidebar border border-gray-700 rounded-xl shadow-2xl w-full max-w-lg overflow-hidden animate-in fade-in zoom-in duration-200">
        <div className="px-6 py-4 border-b border-gray-800 flex justify-between items-center bg-bg-root/50">
          <div className="flex items-center space-x-2">
            {credential ? <Edit2 className="h-4 w-4 text-accent" /> : <Plus className="h-4 w-4 text-accent" />}
            <h2 className="text-sm font-bold text-white uppercase tracking-wider">
              {credential ? 'Edit Credential' : 'Add Credential'}
            </h2>
          </div>
          <button onClick={onClose} aria-label="Close dialog" className="text-gray-500 hover:text-white transition-colors">
            <X className="h-4 w-4" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          {error && (
            <div className="bg-alert/10 border border-alert/30 rounded-lg px-4 py-3 text-alert text-sm" role="alert" aria-live="assertive">
              {error}
            </div>
          )}

          <div>
            <label htmlFor="cred-name" className="block text-sm font-medium text-gray-400 mb-2">Name</label>
            <input id="cred-name"
              type="text"
              value={formData.name}
              onChange={(e) => setFormData({ ...formData, name: e.target.value })}
              className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
              placeholder="My Server"
              required
            />
          </div>

          <div>
            <label htmlFor="cred-type" className="block text-sm font-medium text-gray-400 mb-2">Type</label>
            <select id="cred-type"
              value={formData.credential_type}
              onChange={(e) => setFormData({ ...formData, credential_type: e.target.value })}
              className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
            >
              <option value="ssh">SSH (password)</option>
              <option value="ssh_key">SSH Key</option>
              <option value="rdp">RDP</option>
              <option value="database">Database</option>
              <option value="api">API</option>
              <option value="other">Other</option>
            </select>
          </div>

          {formData.credential_type === 'ssh_key' && (
            <>
              <div>
                <label htmlFor="cred-key-path" className="block text-sm font-medium text-gray-400 mb-2">Key path (optional)</label>
                <input id="cred-key-path"
                  type="text"
                  value={formData.key_path}
                  onChange={(e) => setFormData({ ...formData, key_path: e.target.value })}
                  className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-accent"
                  placeholder="~/.ssh/id_ed25519"
                />
              </div>
              <div>
                <label htmlFor="cred-private-key" className="block text-sm font-medium text-gray-400 mb-2">Private key PEM (or use path above)</label>
                <textarea id="cred-private-key"
                  value={formData.private_key}
                  onChange={(e) => setFormData({ ...formData, private_key: e.target.value })}
                  className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-accent font-mono text-xs min-h-[120px]"
                  placeholder="-----BEGIN OPENSSH PRIVATE KEY-----..."
                  rows={5}
                />
              </div>
              <div>
                <label htmlFor="cred-key-passphrase" className="block text-sm font-medium text-gray-400 mb-2">Key passphrase (optional)</label>
                <div className="relative">
                  <input id="cred-key-passphrase"
                    type={showPassphrase ? 'text' : 'password'}
                    autoComplete="off"
                    value={formData.key_passphrase}
                    onChange={(e) => setFormData({ ...formData, key_passphrase: e.target.value })}
                    className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 pr-12 py-2 text-white focus:outline-none focus:border-accent"
                    placeholder="Passphrase for encrypted key"
                  />
                  <button
                    type="button"
                    onClick={() => setShowPassphrase(!showPassphrase)}
                    aria-label="Toggle passphrase visibility"
                    aria-pressed={showPassphrase}
                    className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-300 transition-colors"
                  >
                    {showPassphrase ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                  </button>
                </div>
              </div>
            </>
          )}

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label htmlFor="cred-host" className="block text-sm font-medium text-gray-400 mb-2">Host (optional)</label>
              <input id="cred-host"
                type="text"
                value={formData.host}
                onChange={(e) => setFormData({ ...formData, host: e.target.value })}
                className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
                placeholder="192.168.1.100"
              />
            </div>
            <div>
              <label htmlFor="cred-port" className="block text-sm font-medium text-gray-400 mb-2">Port (optional)</label>
              <input id="cred-port"
                type="number"
                min="1"
                max="65535"
                value={formData.port}
                onChange={(e) => setFormData({ ...formData, port: e.target.value })}
                className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
                placeholder="22"
              />
            </div>
          </div>

          <div>
            <label htmlFor="cred-username" className="block text-sm font-medium text-gray-400 mb-2">Username</label>
            <div className="relative">
              <User className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-gray-500" />
              <input id="cred-username"
                type="text"
                value={formData.username}
                onChange={(e) => setFormData({ ...formData, username: e.target.value })}
                className="w-full bg-bg-root border border-gray-700 rounded-lg pl-10 pr-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
                placeholder="root"
                required
              />
            </div>
          </div>

          {formData.credential_type !== 'ssh_key' && (
          <div>
            <label htmlFor="cred-password" className="block text-sm font-medium text-gray-400 mb-2">Password</label>
            <div className="relative">
              <Lock className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-gray-500" />
              <input id="cred-password"
                type={showPassword ? 'text' : 'password'}
                autoComplete="new-password"
                value={formData.password}
                onChange={(e) => setFormData({ ...formData, password: e.target.value })}
                className="w-full bg-bg-root border border-gray-700 rounded-lg pl-10 pr-12 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
                placeholder={credential?.has_password ? 'Leave blank to keep current password' : '••••••••'}
                required={!credential?.has_password}
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                aria-label="Toggle password visibility"
                aria-pressed={showPassword}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-300 transition-colors"
              >
                {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            </div>
          </div>
          )}

          <div className="pt-2 flex space-x-2">
            <button
              type="submit"
              disabled={isSaving}
              className="flex-1 bg-accent hover:bg-accent/80 disabled:bg-accent/50 disabled:cursor-not-allowed text-white py-2.5 rounded-lg text-sm font-bold transition-all shadow-lg shadow-accent/20 flex items-center justify-center"
            >
              <Save className="h-4 w-4 mr-2" />
              {isSaving ? 'Saving...' : 'Save'}
            </button>
            <button
              type="button"
              onClick={onClose}
              disabled={isSaving}
              className="px-6 py-2.5 text-sm font-medium text-gray-400 hover:text-white transition-colors disabled:opacity-50"
            >
              Cancel
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};

export default CredentialFormDialog;
