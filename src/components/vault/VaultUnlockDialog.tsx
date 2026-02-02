import React, { useState } from 'react';
import { Lock, ShieldAlert, X, Eye, EyeOff } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

interface VaultUnlockDialogProps {
  onUnlocked: () => void;
  onCancel?: () => void;
}

const VaultUnlockDialog: React.FC<VaultUnlockDialogProps> = ({ onUnlocked, onCancel }) => {
  const [masterPassword, setMasterPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState('');
  const [isUnlocking, setIsUnlocking] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!masterPassword) {
      setError('Master password is required');
      return;
    }

    setIsUnlocking(true);
    setError('');

    try {
      await invoke('unlock_vault', { masterPassword });
      setMasterPassword('');
      setIsUnlocking(false);
      onUnlocked();
    } catch (err) {
      setError(err as string || 'Failed to unlock vault');
      setMasterPassword('');
      setIsUnlocking(false);
    }
  };

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/90 backdrop-blur-md">
      <div className="bg-bg-sidebar border border-gray-700 rounded-xl shadow-2xl w-full max-w-md overflow-hidden animate-in fade-in zoom-in duration-200">
        <div className="px-6 py-4 border-b border-gray-800 flex justify-between items-center bg-bg-root/50">
          <div className="flex items-center space-x-2">
            <Lock className="h-5 w-5 text-accent" />
            <h2 className="text-base font-bold text-white uppercase tracking-wider">Unlock Vault</h2>
          </div>
          {onCancel && (
            <button onClick={onCancel} className="text-gray-500 hover:text-white transition-colors">
              <X className="h-4 w-4" />
            </button>
          )}
        </div>
        
        <form onSubmit={handleSubmit} className="p-6 space-y-5">
          <div className="text-center space-y-2">
            <div className="inline-flex p-4 bg-accent/10 rounded-full mb-2">
              <ShieldAlert className="h-8 w-8 text-accent" />
            </div>
            <p className="text-gray-300 text-sm">
              Enter your master password to unlock the credential vault
            </p>
          </div>

          {error && (
            <div className="bg-alert/10 border border-alert/30 rounded-lg px-4 py-3 text-alert text-sm">
              {error}
            </div>
          )}

          <div className="relative">
            <input 
              autoFocus
              type={showPassword ? 'text' : 'password'}
              className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-3 pr-12 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all placeholder:text-gray-600"
              placeholder="Master Password"
              value={masterPassword}
              onChange={(e) => {
                setMasterPassword(e.target.value);
                setError('');
              }}
              disabled={isUnlocking}
              required
            />
            <button
              type="button"
              onClick={() => setShowPassword(!showPassword)}
              className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-300 transition-colors"
              tabIndex={-1}
            >
              {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
            </button>
          </div>

          <div className="pt-2 flex flex-col space-y-2">
            <button 
              type="submit"
              disabled={isUnlocking}
              className="w-full bg-accent hover:bg-accent/80 disabled:bg-accent/50 disabled:cursor-not-allowed text-white py-3 rounded-lg text-sm font-bold transition-all shadow-lg shadow-accent/20"
            >
              {isUnlocking ? 'Unlocking...' : 'Unlock Vault'}
            </button>
            {onCancel && (
              <button 
                type="button"
                onClick={onCancel}
                disabled={isUnlocking}
                className="w-full py-2.5 text-xs font-medium text-gray-500 hover:text-gray-300 transition-colors disabled:opacity-50"
              >
                Cancel
              </button>
            )}
          </div>
        </form>
      </div>
    </div>
  );
};

export default VaultUnlockDialog;
