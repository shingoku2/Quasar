import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Settings, Lock, Clock, Shield, Save, AlertTriangle } from 'lucide-react';

interface VaultSettings {
  auto_lock_timeout_minutes: number;
  require_password_on_credential_use: boolean;
  vault_initialized: boolean;
}

const VaultSettings: React.FC = () => {
  const [settings, setSettings] = useState<VaultSettings>({
    auto_lock_timeout_minutes: 15,
    require_password_on_credential_use: false,
    vault_initialized: false,
  });
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState('');
  const [successMessage, setSuccessMessage] = useState('');
  const [showChangeMasterPassword, setShowChangeMasterPassword] = useState(false);
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [isChangingPassword, setIsChangingPassword] = useState(false);

  useEffect(() => {
    loadSettings();
  }, []);

  const loadSettings = async () => {
    setIsLoading(true);
    setError('');
    try {
      const vaultSettings = await invoke<VaultSettings>('get_vault_settings');
      setSettings(vaultSettings);
    } catch (err) {
      setError(String(err) || 'Failed to load vault settings');
    } finally {
      setIsLoading(false);
    }
  };

  const handleSave = async () => {
    setIsSaving(true);
    setError('');
    setSuccessMessage('');

    try {
      await invoke('update_vault_settings', { settings });
      setSuccessMessage('Settings saved successfully');
      setTimeout(() => setSuccessMessage(''), 3000);
    } catch (err) {
      setError(String(err) || 'Failed to save settings');
    } finally {
      setIsSaving(false);
    }
  };

  const handleLockVault = async () => {
    try {
      await invoke('lock_vault');
      setSuccessMessage('Vault locked successfully');
      setTimeout(() => setSuccessMessage(''), 3000);
    } catch (err) {
      setError(String(err) || 'Failed to lock vault');
    }
  };

  return (
    <div className="flex flex-col h-full bg-bg-root">
      <div className="p-6 border-b border-gray-800">
        <div className="flex items-center space-x-2">
          <Settings className="h-5 w-5 text-accent" />
          <h1 className="text-xl font-bold text-white">Vault Settings</h1>
        </div>
      </div>

      {error && (
        <div className="mx-6 mt-4 bg-alert/10 border border-alert/30 rounded-lg px-4 py-3 text-alert text-sm">
          {error}
        </div>
      )}

      {successMessage && (
        <div className="mx-6 mt-4 bg-success/10 border border-success/30 rounded-lg px-4 py-3 text-success text-sm">
          {successMessage}
        </div>
      )}

      <div className="flex-1 overflow-auto p-6">
        {isLoading ? (
          <div className="text-center text-gray-500 py-12">Loading settings...</div>
        ) : (
          <div className="max-w-2xl space-y-6">
            {/* Auto-lock Settings */}
            <div className="bg-bg-sidebar border border-gray-700 rounded-lg p-6">
              <div className="flex items-start space-x-3 mb-4">
                <Clock className="h-5 w-5 text-accent shrink-0 mt-0.5" />
                <div className="flex-1">
                  <h3 className="text-white font-bold mb-1">Auto-lock Timeout</h3>
                  <p className="text-sm text-gray-400">
                    Automatically lock the vault after a period of inactivity
                  </p>
                </div>
              </div>

              <div className="space-y-3">
                <div>
                  <label className="block text-sm font-medium text-gray-400 mb-2">
                    Timeout (minutes)
                  </label>
                  <input
                    type="number"
                    min="1"
                    max="1440"
                    value={settings.auto_lock_timeout_minutes}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        auto_lock_timeout_minutes: parseInt(e.target.value) || 15,
                      })
                    }
                    className="w-full bg-bg-root border border-gray-700 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
                  />
                  <p className="text-xs text-gray-500 mt-1">
                    Recommended: 15 minutes. Range: 1-1440 minutes (24 hours)
                  </p>
                </div>

                <div className="flex items-center space-x-2 text-sm text-gray-400">
                  <AlertTriangle className="h-4 w-4 text-warning" />
                  <span>Shorter timeouts provide better security but require more frequent unlocking</span>
                </div>
              </div>
            </div>

            {/* Security Options */}
            <div className="bg-bg-sidebar border border-gray-700 rounded-lg p-6">
              <div className="flex items-start space-x-3 mb-4">
                <Shield className="h-5 w-5 text-accent shrink-0 mt-0.5" />
                <div className="flex-1">
                  <h3 className="text-white font-bold mb-1">Security Options</h3>
                  <p className="text-sm text-gray-400">
                    Additional security measures for credential access
                  </p>
                </div>
              </div>

              <div className="space-y-4">
                <label className="flex items-start space-x-3 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.require_password_on_credential_use}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        require_password_on_credential_use: e.target.checked,
                      })
                    }
                    className="mt-1 h-4 w-4 rounded border-gray-600 bg-bg-root text-accent focus:ring-accent focus:ring-offset-0"
                  />
                  <div className="flex-1">
                    <p className="text-white text-sm font-medium">
                      Require master password for credential access
                    </p>
                    <p className="text-gray-400 text-xs mt-1">
                      Prompt for master password each time a credential is accessed (not recommended for frequent use)
                    </p>
                  </div>
                </label>
              </div>
            </div>

            {/* Master Password Management */}
            <div className="bg-bg-sidebar border border-gray-700 rounded-lg p-6">
              <div className="flex items-start space-x-3 mb-4">
                <Lock className="h-5 w-5 text-accent shrink-0 mt-0.5" />
                <div className="flex-1">
                  <h3 className="text-white font-bold mb-1">Master Password</h3>
                  <p className="text-sm text-gray-400">
                    Manage your vault master password
                  </p>
                </div>
              </div>

              <div className="space-y-3">
                <button
                  onClick={() => setShowChangeMasterPassword(!showChangeMasterPassword)}
                  className="w-full bg-bg-root border border-gray-700 hover:border-accent text-white py-2.5 px-4 rounded-lg text-sm font-medium transition-all text-left"
                >
                  Change Master Password
                </button>

                {showChangeMasterPassword && (
                  <div className="bg-bg-root border border-gray-700 rounded-lg p-4 space-y-3">
                    <div>
                      <label className="block text-sm font-medium text-gray-400 mb-2">
                        Current Password
                      </label>
                      <input
                        type="password"
                        autoComplete="current-password"
                        value={currentPassword}
                        onChange={(e) => setCurrentPassword(e.target.value)}
                        className="w-full bg-bg-sidebar border border-gray-700 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
                        placeholder="Enter current master password"
                      />
                    </div>

                    <div>
                      <label className="block text-sm font-medium text-gray-400 mb-2">
                        New Password
                      </label>
                      <input
                        type="password"
                        autoComplete="new-password"
                        value={newPassword}
                        onChange={(e) => setNewPassword(e.target.value)}
                        className="w-full bg-bg-sidebar border border-gray-700 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
                        placeholder="Enter new master password (min 12 characters)"
                      />
                    </div>

                    <div>
                      <label className="block text-sm font-medium text-gray-400 mb-2">
                        Confirm New Password
                      </label>
                      <input
                        type="password"
                        autoComplete="new-password"
                        value={confirmPassword}
                        onChange={(e) => setConfirmPassword(e.target.value)}
                        className="w-full bg-bg-sidebar border border-gray-700 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent transition-all"
                        placeholder="Confirm new master password"
                      />
                    </div>

                    <div className="bg-warning/10 border border-warning/30 rounded-lg p-3 text-xs text-gray-300">
                      <p className="font-medium text-warning mb-1">⚠️ Important</p>
                      <p>
                        Changing your master password will re-encrypt all stored credentials.
                        This operation cannot be undone.
                      </p>
                    </div>

                    <div className="flex space-x-2">
                      <button
                        onClick={async () => {
                          if (!currentPassword || !newPassword || !confirmPassword) {
                            setError('All fields are required');
                            return;
                          }
                          if (newPassword !== confirmPassword) {
                            setError('New passwords do not match');
                            return;
                          }
                          if (newPassword.length < 12) {
                            setError('New password must be at least 12 characters');
                            return;
                          }

                          setIsChangingPassword(true);
                          setError('');
                          setSuccessMessage('');

                          try {
                            await invoke('change_master_password', {
                              currentPassword,
                              newPassword,
                            });
                            setSuccessMessage('Master password changed successfully');
                            setShowChangeMasterPassword(false);
                            setCurrentPassword('');
                            setNewPassword('');
                            setConfirmPassword('');
                            setTimeout(() => setSuccessMessage(''), 3000);
                          } catch (err) {
                            setError(String(err) || 'Failed to change master password');
                          } finally {
                            setIsChangingPassword(false);
                          }
                        }}
                        disabled={isChangingPassword}
                        className="flex-1 bg-accent hover:bg-accent/80 disabled:bg-accent/50 disabled:cursor-not-allowed text-white py-2 rounded-lg text-sm font-medium transition-all"
                      >
                        {isChangingPassword ? 'Changing...' : 'Change Password'}
                      </button>
                      <button
                        onClick={() => {
                          setShowChangeMasterPassword(false);
                          setCurrentPassword('');
                          setNewPassword('');
                          setConfirmPassword('');
                          setError('');
                        }}
                        className="flex-1 bg-bg-sidebar border border-gray-700 hover:border-gray-600 text-white py-2 rounded-lg text-sm font-medium transition-all"
                      >
                        Cancel
                      </button>
                    </div>
                  </div>
                )}

                <button
                  onClick={handleLockVault}
                  className="w-full bg-bg-root border border-gray-700 hover:border-alert text-white py-2.5 px-4 rounded-lg text-sm font-medium transition-all text-left"
                >
                  Lock Vault Now
                </button>
              </div>
            </div>

            {/* Vault Status */}
            <div className="bg-accent/10 border border-accent/30 rounded-lg p-4">
              <div className="flex items-center space-x-2 text-sm">
                <Shield className="h-4 w-4 text-accent" />
                <span className="text-gray-300">
                  Vault Status: <span className="font-bold text-white">
                    {settings.vault_initialized ? 'Initialized' : 'Not Initialized'}
                  </span>
                </span>
              </div>
            </div>

            {/* Save Button */}
            <div className="pt-4 border-t border-gray-800">
              <button
                onClick={handleSave}
                disabled={isSaving}
                className="w-full bg-accent hover:bg-accent/80 disabled:bg-accent/50 disabled:cursor-not-allowed text-white py-3 rounded-lg text-sm font-bold transition-all shadow-lg shadow-accent/20 flex items-center justify-center"
              >
                <Save className="h-4 w-4 mr-2" />
                {isSaving ? 'Saving...' : 'Save Settings'}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default VaultSettings;
