import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { save, open } from '@tauri-apps/plugin-dialog';
import { Settings, Shield, Bell, Palette, Database, Info } from 'lucide-react';
import VaultSettings from './vault/VaultSettings';
import { getErrorMessage } from '../lib/utils';

const SETTINGS_STORAGE_KEY = 'quasar_settings';

export type SettingsCategory = 'security' | 'notifications' | 'appearance' | 'data' | 'about';

export interface StoredSettings {
  notifications: {
    desktopNotifications: boolean;
    emailNotifications: boolean;
    alertSounds: boolean;
  };
  appearance: {
    theme: 'dark' | 'light';
    accentColor: string;
  };
}

const defaultSettings: StoredSettings = {
  notifications: {
    desktopNotifications: true,
    emailNotifications: false,
    alertSounds: true,
  },
  appearance: {
    theme: 'dark',
    accentColor: '#00d4ff',
  },
};

function loadStoredSettings(): StoredSettings {
  try {
    const raw = localStorage.getItem(SETTINGS_STORAGE_KEY);
    if (!raw) return defaultSettings;
    const parsed = JSON.parse(raw) as Partial<StoredSettings>;
    return {
      notifications: { ...defaultSettings.notifications, ...parsed.notifications },
      appearance: { ...defaultSettings.appearance, ...parsed.appearance },
    };
  } catch {
    return defaultSettings;
  }
}

function saveStoredSettings(settings: StoredSettings) {
  try {
    localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // ignore
  }
}

function applyAppearance(theme: 'dark' | 'light', accentColor: string) {
  document.documentElement.setAttribute('data-theme', theme);
  document.documentElement.style.setProperty('--color-accent', accentColor);
}

const SettingsView: React.FC = () => {
  const [activeCategory, setActiveCategory] = useState<SettingsCategory>('security');

  const categories = [
    { id: 'security' as const, icon: Shield, label: 'Security & Vault', description: 'Vault settings, master password, and security options' },
    { id: 'notifications' as const, icon: Bell, label: 'Notifications', description: 'Alert preferences and notification settings' },
    { id: 'appearance' as const, icon: Palette, label: 'Appearance', description: 'Theme, colors, and display preferences' },
    { id: 'data' as const, icon: Database, label: 'Data & Storage', description: 'Database, backups, and data management' },
    { id: 'about' as const, icon: Info, label: 'About', description: 'Version info and system details' },
  ];

  const renderContent = () => {
    switch (activeCategory) {
      case 'security':
        return <VaultSettings />;
      case 'notifications':
        return <NotificationsSettings />;
      case 'appearance':
        return <AppearanceSettings />;
      case 'data':
        return <DataSettings />;
      case 'about':
        return <AboutSettings />;
      default:
        return <VaultSettings />;
    }
  };

  return (
    <div className="flex h-full bg-bg-root">
      <div className="w-80 border-r border-border bg-bg-sidebar flex flex-col">
        <div className="p-6 border-b border-border">
          <div className="flex items-center space-x-2">
            <Settings className="h-6 w-6 text-accent" />
            <h1 className="text-xl font-bold text-white">Settings</h1>
          </div>
          <p className="text-sm text-gray-400 mt-1">Configure Quasar</p>
        </div>

        <div className="flex-1 overflow-y-auto p-4 space-y-2">
          {categories.map((category) => {
            const isActive = activeCategory === category.id;
            const Icon = category.icon;
            return (
              <button
                key={category.id}
                onClick={() => setActiveCategory(category.id)}
                className={`w-full text-left p-4 rounded-lg transition-all ${
                  isActive
                    ? 'bg-accent/10 border border-accent/30'
                    : 'bg-bg-root border border-border hover:border-gray-700 hover:bg-bg-root/50'
                }`}
              >
                <div className="flex items-start space-x-3">
                  <Icon className={`h-5 w-5 mt-0.5 shrink-0 ${isActive ? 'text-accent' : 'text-gray-400'}`} />
                  <div className="flex-1 min-w-0">
                    <h3 className={`text-sm font-bold ${isActive ? 'text-white' : 'text-gray-300'}`}>
                      {category.label}
                    </h3>
                    <p className="text-xs text-gray-500 mt-1 line-clamp-2">
                      {category.description}
                    </p>
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      </div>

      <div className="flex-1 overflow-hidden">
        {renderContent()}
      </div>
    </div>
  );
};

const NotificationsSettings: React.FC = () => {
  const [settings, setSettings] = useState<StoredSettings>(defaultSettings);

  useEffect(() => {
    setSettings(loadStoredSettings());
  }, []);

  const updateNotifications = useCallback((patch: Partial<NonNullable<StoredSettings['notifications']>>) => {
    const next = {
      ...settings,
      notifications: { ...defaultSettings.notifications, ...settings.notifications, ...patch },
    };
    setSettings(next);
    saveStoredSettings(next);
  }, [settings]);

  const n = settings.notifications;

  return (
    <div className="flex flex-col h-full bg-bg-root">
      <div className="p-6 border-b border-border">
        <div className="flex items-center space-x-2">
          <Bell className="h-5 w-5 text-accent" />
          <h2 className="text-xl font-bold text-white">Notifications</h2>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-2xl space-y-6">
          <div className="bg-bg-card border border-border rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">Alert Notifications</h3>
            <div className="space-y-4">
              <label className="flex items-start space-x-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={n.desktopNotifications}
                  onChange={(e) => updateNotifications({ desktopNotifications: e.target.checked })}
                  className="mt-1 h-4 w-4 rounded border-gray-600 bg-bg-root text-accent focus:ring-accent focus:ring-offset-0"
                />
                <div className="flex-1">
                  <p className="text-white text-sm font-medium">Desktop Notifications</p>
                  <p className="text-gray-400 text-xs mt-1">
                    Show system notifications for critical alerts
                  </p>
                </div>
              </label>

              <label className="flex items-start space-x-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={n.emailNotifications}
                  onChange={(e) => updateNotifications({ emailNotifications: e.target.checked })}
                  className="mt-1 h-4 w-4 rounded border-gray-600 bg-bg-root text-accent focus:ring-accent focus:ring-offset-0"
                />
                <div className="flex-1">
                  <p className="text-white text-sm font-medium">Email Notifications</p>
                  <p className="text-gray-400 text-xs mt-1">
                    Send email alerts for critical system events (requires SMTP configuration)
                  </p>
                </div>
              </label>

              <label className="flex items-start space-x-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={n.alertSounds}
                  onChange={(e) => updateNotifications({ alertSounds: e.target.checked })}
                  className="mt-1 h-4 w-4 rounded border-gray-600 bg-bg-root text-accent focus:ring-accent focus:ring-offset-0"
                />
                <div className="flex-1">
                  <p className="text-white text-sm font-medium">Alert Sounds</p>
                  <p className="text-gray-400 text-xs mt-1">
                    Play sound when alerts are triggered
                  </p>
                </div>
              </label>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

const AppearanceSettings: React.FC = () => {
  const [settings, setSettings] = useState<StoredSettings>(defaultSettings);

  useEffect(() => {
    const s = loadStoredSettings();
    setSettings(s);
    const app = s.appearance ?? defaultSettings.appearance!;
    applyAppearance(app.theme, app.accentColor);
  }, []);

  const updateAppearance = useCallback((patch: Partial<StoredSettings['appearance']>) => {
    const next = {
      ...settings,
      appearance: { ...defaultSettings.appearance, ...settings.appearance, ...patch },
    };
    setSettings(next);
    saveStoredSettings(next);
    applyAppearance(next.appearance.theme, next.appearance.accentColor);
  }, [settings]);

  const app = settings.appearance;

  return (
    <div className="flex flex-col h-full bg-bg-root">
      <div className="p-6 border-b border-border">
        <div className="flex items-center space-x-2">
          <Palette className="h-5 w-5 text-accent" />
          <h2 className="text-xl font-bold text-white">Appearance</h2>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-2xl space-y-6">
          <div className="bg-bg-card border border-border rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">Theme</h3>
            <div className="space-y-3">
              <label className="flex items-center space-x-3 cursor-pointer">
                <input
                  type="radio"
                  name="theme"
                  checked={app.theme === 'dark'}
                  onChange={() => updateAppearance({ theme: 'dark' })}
                  className="h-4 w-4 border-gray-600 bg-bg-root text-accent focus:ring-accent focus:ring-offset-0"
                />
                <span className="text-white text-sm">Dark Theme</span>
              </label>
              <label className="flex items-center space-x-3 cursor-pointer">
                <input
                  type="radio"
                  name="theme"
                  checked={app.theme === 'light'}
                  onChange={() => updateAppearance({ theme: 'light' })}
                  className="h-4 w-4 border-gray-600 bg-bg-root text-accent focus:ring-accent focus:ring-offset-0"
                />
                <span className="text-white text-sm">Light Theme</span>
              </label>
            </div>
          </div>

          <div className="bg-bg-card border border-border rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">Accent Color</h3>
            <div className="flex items-center space-x-4">
              <input
                type="color"
                value={app.accentColor}
                onChange={(e) => updateAppearance({ accentColor: e.target.value })}
                className="h-12 w-24 rounded border border-border bg-bg-root cursor-pointer"
              />
              <div>
                <p className="text-white text-sm font-medium">{app.accentColor}</p>
                <p className="text-gray-400 text-xs mt-1">Current accent color</p>
              </div>
            </div>
            <p className="text-xs text-gray-500 mt-3">
              Changes apply immediately
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};

interface AppInfo {
  app_data_dir: string;
  db_path: string;
  db_size_bytes: number | null;
  version: string;
  platform: string;
  arch: string;
}

const METRICS_RETENTION_DAYS = 30;

const DataSettings: React.FC = () => {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);
  const [clearing, setClearing] = useState(false);

  const loadInfo = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      const info = await invoke<AppInfo>('get_app_info');
      setAppInfo(info);
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadInfo();
  }, [loadInfo]);

  const handleExport = async () => {
    const path = await save({
      defaultPath: `quasar-backup-${new Date().toISOString().slice(0, 10)}.db`,
      filters: [{ name: 'Database', extensions: ['db'] }],
    });
    if (!path) return;
    setExporting(true);
    setError('');
    setSuccess('');
    try {
      await invoke('export_database', { destPath: path });
      setSuccess('Database exported successfully.');
      setTimeout(() => setSuccess(''), 3000);
      await loadInfo();
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setExporting(false);
    }
  };

  const handleImport = async () => {
    const path = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Database', extensions: ['db'] }],
    });
    if (!path || typeof path !== 'string') return;
    setImporting(true);
    setError('');
    setSuccess('');
    try {
      await invoke('import_database', { sourcePath: path });
      setSuccess('Database imported. Restart the app for changes to take effect.');
      setTimeout(() => setSuccess(''), 5000);
      await loadInfo();
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setImporting(false);
    }
  };

  const handleClearMetrics = async () => {
    if (!window.confirm('Clear all metrics and alert history? This cannot be undone.')) return;
    setClearing(true);
    setError('');
    setSuccess('');
    try {
      await invoke('clear_metrics_data');
      setSuccess('Metrics and alert history cleared.');
      setTimeout(() => setSuccess(''), 3000);
      await loadInfo();
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setClearing(false);
    }
  };

  const dbSizeStr = appInfo?.db_size_bytes != null
    ? (appInfo.db_size_bytes < 1024
        ? `${appInfo.db_size_bytes} B`
        : appInfo.db_size_bytes < 1024 * 1024
          ? `${(appInfo.db_size_bytes / 1024).toFixed(1)} KB`
          : `${(appInfo.db_size_bytes / (1024 * 1024)).toFixed(2)} MB`)
    : '—';

  return (
    <div className="flex flex-col h-full bg-bg-root">
      <div className="p-6 border-b border-border">
        <div className="flex items-center space-x-2">
          <Database className="h-5 w-5 text-accent" />
          <h2 className="text-xl font-bold text-white">Data & Storage</h2>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-2xl space-y-6">
          {error && (
            <div className="bg-alert/10 border border-alert/30 rounded-lg p-4 text-alert text-sm">
              {error}
            </div>
          )}
          {success && (
            <div className="bg-success/10 border border-success/30 rounded-lg p-4 text-success text-sm">
              {success}
            </div>
          )}

          <div className="bg-bg-card border border-border rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">Database</h3>
            {loading ? (
              <p className="text-gray-400 text-sm">Loading…</p>
            ) : appInfo ? (
              <div className="space-y-3">
                <div className="flex justify-between items-center">
                  <span className="text-gray-400 text-sm">Database Location</span>
                  <span className="text-white text-sm font-mono truncate max-w-[60%]" title={appInfo.db_path}>
                    {appInfo.db_path}
                  </span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-gray-400 text-sm">Database Size</span>
                  <span className="text-white text-sm">{dbSizeStr}</span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-gray-400 text-sm">Metrics Retention</span>
                  <span className="text-white text-sm">{METRICS_RETENTION_DAYS} days</span>
                </div>
              </div>
            ) : (
              <p className="text-gray-400 text-sm">Unable to load database info.</p>
            )}
          </div>

          <div className="bg-bg-card border border-border rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">Backup & Export</h3>
            <div className="space-y-3">
              <button
                type="button"
                onClick={handleExport}
                disabled={exporting}
                className="w-full bg-bg-root border border-border hover:border-accent text-white py-2.5 px-4 rounded-lg text-sm font-medium transition-all text-left disabled:opacity-50"
              >
                {exporting ? 'Exporting…' : 'Export Database Backup'}
              </button>
              <button
                type="button"
                onClick={handleImport}
                disabled={importing}
                className="w-full bg-bg-root border border-border hover:border-accent text-white py-2.5 px-4 rounded-lg text-sm font-medium transition-all text-left disabled:opacity-50"
              >
                {importing ? 'Importing…' : 'Import Database Backup'}
              </button>
              <button
                type="button"
                onClick={handleClearMetrics}
                disabled={clearing}
                className="w-full bg-bg-root border border-alert/50 hover:border-alert text-white py-2.5 px-4 rounded-lg text-sm font-medium transition-all text-left disabled:opacity-50"
              >
                {clearing ? 'Clearing…' : 'Clear All Metrics Data'}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

const AboutSettings: React.FC = () => {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);

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

// Apply stored appearance on app load (so theme persists across navigation)
const stored = loadStoredSettings();
const appearance = stored.appearance ?? defaultSettings.appearance;
if (appearance) {
  applyAppearance(appearance.theme, appearance.accentColor);
}

export default SettingsView;
