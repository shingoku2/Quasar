import React, { useState } from 'react';
import { ShieldPlus, AlertTriangle, Eye, EyeOff } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

interface VaultInitDialogProps {
  onInitialized: () => void;
}

const VaultInitDialog: React.FC<VaultInitDialogProps> = ({ onInitialized }) => {
  const [masterPassword, setMasterPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const [error, setError] = useState('');
  const [isInitializing, setIsInitializing] = useState(false);

  const validatePassword = (password: string): string | null => {
    if (password.length < 12) {
      return 'Master password must be at least 12 characters';
    }
    if (!/[A-Z]/.test(password)) {
      return 'Master password must contain at least one uppercase letter';
    }
    if (!/[a-z]/.test(password)) {
      return 'Master password must contain at least one lowercase letter';
    }
    if (!/[0-9]/.test(password)) {
      return 'Master password must contain at least one number';
    }
    return null;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    const validationError = validatePassword(masterPassword);
    if (validationError) {
      setError(validationError);
      return;
    }

    if (masterPassword !== confirmPassword) {
      setError('Passwords do not match');
      return;
    }

    setIsInitializing(true);
    setError('');

    try {
      await invoke('initialize_vault', { masterPassword });
      setMasterPassword('');
      setConfirmPassword('');
      onInitialized();
    } catch (err) {
      setError(err as string || 'Failed to initialize vault');
    } finally {
      setMasterPassword('');
      setConfirmPassword('');
      setIsInitializing(false);
    }
  };

  const passwordStrength = (password: string): { strength: number; label: string; color: string } => {
    let strength = 0;
    if (password.length >= 8) strength++;
    if (password.length >= 12) strength++;
    if (/[A-Z]/.test(password)) strength++;
    if (/[a-z]/.test(password)) strength++;
    if (/[0-9]/.test(password)) strength++;
    if (/[^A-Za-z0-9]/.test(password)) strength++;

    if (strength <= 2) return { strength: 33, label: 'Weak', color: 'bg-alert' };
    if (strength <= 4) return { strength: 66, label: 'Medium', color: 'bg-warning' };
    return { strength: 100, label: 'Strong', color: 'bg-success' };
  };

  const strength = passwordStrength(masterPassword);

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/90 backdrop-blur-md">
      <div className="bg-bg-sidebar border border-gray-700 rounded-xl shadow-2xl w-full max-w-md overflow-hidden animate-in fade-in zoom-in duration-200">
        <div className="px-6 py-4 border-b border-gray-800 flex justify-between items-center bg-bg-root/50">
          <div className="flex items-center space-x-2">
            <ShieldPlus className="h-5 w-5 text-accent" />
            <h2 className="text-base font-bold text-white uppercase tracking-wider">Initialize Vault</h2>
          </div>
        </div>
        
        <form onSubmit={handleSubmit} className="p-6 space-y-5">
          <div className="bg-accent/10 border border-accent/30 rounded-lg px-4 py-3 flex items-start space-x-3">
            <AlertTriangle className="h-5 w-5 text-accent flex-shrink-0 mt-0.5" />
            <div className="text-sm text-gray-300">
              <p className="font-bold text-white mb-1">Important</p>
              <p>Your master password encrypts all stored credentials. If you forget it, there is no way to recover your data.</p>
            </div>
          </div>

          {error && (
            <div className="bg-alert/10 border border-alert/30 rounded-lg px-4 py-3 text-alert text-sm">
              {error}
            </div>
          )}

          <div className="space-y-4">
            <div className="relative">
              <label className="block text-sm font-medium text-gray-400 mb-2">Master Password</label>
              <input 
                autoFocus
                type={showPassword ? 'text' : 'password'}
<<<<<<< HEAD
                autoComplete="new-password"
=======
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
                className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-3 pr-12 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all placeholder:text-gray-600"
                placeholder="Enter master password"
                value={masterPassword}
                onChange={(e) => {
                  setMasterPassword(e.target.value);
                  setError('');
                }}
                disabled={isInitializing}
                required
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                className="absolute right-3 top-[42px] text-gray-500 hover:text-gray-300 transition-colors"
                tabIndex={-1}
              >
                {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
              {masterPassword && (
                <div className="mt-2">
                  <div className="flex justify-between items-center mb-1">
                    <span className="text-xs text-gray-500">Password Strength</span>
                    <span className={`text-xs font-medium ${strength.color.replace('bg-', 'text-')}`}>
                      {strength.label}
                    </span>
                  </div>
                  <div className="h-1.5 bg-gray-800 rounded-full overflow-hidden">
                    <div 
                      className={`h-full ${strength.color} transition-all duration-300`}
                      style={{ width: `${strength.strength}%` }}
                    />
                  </div>
                </div>
              )}
            </div>

            <div className="relative">
              <label className="block text-sm font-medium text-gray-400 mb-2">Confirm Password</label>
              <input 
                type={showConfirm ? 'text' : 'password'}
<<<<<<< HEAD
                autoComplete="new-password"
=======
>>>>>>> 30e7e777944d676f8ea8e22a69c2d690697e9fa7
                className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-3 pr-12 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all placeholder:text-gray-600"
                placeholder="Confirm master password"
                value={confirmPassword}
                onChange={(e) => {
                  setConfirmPassword(e.target.value);
                  setError('');
                }}
                disabled={isInitializing}
                required
              />
              <button
                type="button"
                onClick={() => setShowConfirm(!showConfirm)}
                className="absolute right-3 top-[42px] text-gray-500 hover:text-gray-300 transition-colors"
                tabIndex={-1}
              >
                {showConfirm ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            </div>
          </div>

          <div className="text-xs text-gray-500 space-y-1">
            <p>Password requirements:</p>
            <ul className="list-disc list-inside space-y-0.5 ml-2">
              <li>At least 12 characters long</li>
              <li>Contains uppercase and lowercase letters</li>
              <li>Contains at least one number</li>
            </ul>
          </div>

          <button 
            type="submit"
            disabled={isInitializing}
            className="w-full bg-accent hover:bg-accent/80 disabled:bg-accent/50 disabled:cursor-not-allowed text-white py-3 rounded-lg text-sm font-bold transition-all shadow-lg shadow-accent/20"
          >
            {isInitializing ? 'Initializing...' : 'Initialize Vault'}
          </button>
        </form>
      </div>
    </div>
  );
};

export default VaultInitDialog;
