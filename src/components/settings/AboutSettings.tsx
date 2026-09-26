import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Shield, Info } from 'lucide-react';
import { useUpdater } from '../../hooks/useUpdater';
import type { AppInfo } from './appInfo';

const AboutSettings: React.FC = () => {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const { status: updateStatus, version: updateVersion, error: updateError, checkForUpdates, installUpdate } = useUpdater(false);

  useEffect(() => {
    invoke<AppInfo>('get_app_info')
      .then(setAppInfo)
      .catch(() => setAppInfo(null));
  }, []);

  const version = appInfo?.version ?? '—';
  const platform = appInfo?.platform ?? '—';
  const arch = appInfo?.arch ?? '—';

  return (
    <div className="flex flex-col h-full bg-bg-root">
      <div className="p-6 border-b border-border">
        <div className="flex items-center space-x-2">
          <Info className="h-5 w-5 text-accent" />
          <h2 className="text-xl font-bold text-white">About</h2>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-2xl space-y-6">
          <div className="bg-bg-card border border-border rounded-lg p-6 text-center">
            <div className="inline-flex items-center justify-center w-20 h-20 bg-accent/10 rounded-2xl mb-4">
              <Shield className="h-10 w-10 text-accent" />
            </div>
            <h2 className="text-2xl font-bold text-white mb-2">Quasar</h2>
            <p className="text-accent font-mono text-sm mb-4">v{version}</p>
            <p className="text-gray-400 text-sm">
              Remote Infrastructure Management Platform
            </p>
          </div>

          <div className="bg-bg-card border border-border rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">System Information</h3>
            <div className="space-y-3 text-sm">
              <div className="flex justify-between">
                <span className="text-gray-400">Platform</span>
                <span className="text-white capitalize">{platform}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-400">Architecture</span>
                <span className="text-white">{arch}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-400">Tauri</span>
                <span className="text-white">2.x</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-400">React</span>
                <span className="text-white">19.x</span>
              </div>
            </div>
          </div>

          <div className="bg-bg-card border border-border rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">Updates</h3>
            {updateStatus === 'available' ? (
              <div className="space-y-3">
                <p className="text-sm text-gray-300">Version {updateVersion} is available.</p>
                <button
                  type="button"
                  onClick={installUpdate}
                  className="w-full bg-accent hover:bg-accent/90 text-black py-2.5 px-4 rounded-lg text-sm font-medium transition-all"
                >
                  Install & Restart
                </button>
              </div>
            ) : (
              <button
                type="button"
                onClick={checkForUpdates}
                disabled={updateStatus === 'checking' || updateStatus === 'downloading'}
                className="w-full bg-bg-root border border-border hover:border-accent text-white py-2.5 px-4 rounded-lg text-sm font-medium transition-all text-left disabled:opacity-50"
              >
                {updateStatus === 'checking'
                  ? 'Checking…'
                  : updateStatus === 'downloading'
                    ? 'Downloading update…'
                    : 'Check for Updates'}
              </button>
            )}
            {updateStatus === 'upToDate' && (
              <p className="text-xs text-success mt-3">You're on the latest version.</p>
            )}
            {updateStatus === 'error' && (
              <p className="text-xs text-alert mt-3">{updateError}</p>
            )}
          </div>

          <div className="bg-bg-card border border-border rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">Features</h3>
            <div className="grid grid-cols-2 gap-3 text-sm">
              {['SSH Management', 'SFTP Transfer', 'System Monitoring', 'Security Vault', 'Alert System', 'AI Assistant'].map((feature) => (
                <div key={feature} className="flex items-center space-x-2">
                  <div className="w-2 h-2 bg-success rounded-full" />
                  <span className="text-gray-300">{feature}</span>
                </div>
              ))}
            </div>
          </div>

          <div className="bg-accent/10 border border-accent/30 rounded-lg p-4 text-center">
            <p className="text-sm text-gray-300">
              Built with ❤️ for infrastructure management
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};

export default AboutSettings;
