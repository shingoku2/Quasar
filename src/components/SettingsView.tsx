import React, { useState } from 'react';
import { Settings, Shield, Bell, Palette, Database, Info } from 'lucide-react';
import VaultSettings from './vault/VaultSettings';

type SettingsCategory = 'security' | 'notifications' | 'appearance' | 'data' | 'about';

const SettingsView: React.FC = () => {
  const [activeCategory, setActiveCategory] = useState<SettingsCategory>('security');

  const categories = [
    { id: 'security', icon: Shield, label: 'Security & Vault', description: 'Vault settings, master password, and security options' },
    { id: 'notifications', icon: Bell, label: 'Notifications', description: 'Alert preferences and notification settings' },
    { id: 'appearance', icon: Palette, label: 'Appearance', description: 'Theme, colors, and display preferences' },
    { id: 'data', icon: Database, label: 'Data & Storage', description: 'Database, backups, and data management' },
    { id: 'about', icon: Info, label: 'About', description: 'Version info and system details' },
  ] as const;

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
      {/* Sidebar */}
      <div className="w-80 border-r border-gray-800 bg-bg-sidebar flex flex-col">
        <div className="p-6 border-b border-gray-800">
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
                onClick={() => setActiveCategory(category.id as SettingsCategory)}
                className={`w-full text-left p-4 rounded-lg transition-all ${
                  isActive
                    ? 'bg-accent/10 border border-accent/30'
                    : 'bg-bg-root border border-gray-800 hover:border-gray-700 hover:bg-bg-root/50'
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

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {renderContent()}
      </div>
    </div>
  );
};

// Notifications Settings Component
const NotificationsSettings: React.FC = () => {
  const [emailNotifications, setEmailNotifications] = useState(false);
  const [desktopNotifications, setDesktopNotifications] = useState(true);
  const [alertSounds, setAlertSounds] = useState(true);

  return (
    <div className="flex flex-col h-full bg-bg-root">
      <div className="p-6 border-b border-gray-800">
        <div className="flex items-center space-x-2">
          <Bell className="h-5 w-5 text-accent" />
          <h2 className="text-xl font-bold text-white">Notifications</h2>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-2xl space-y-6">
          <div className="bg-bg-sidebar border border-gray-700 rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">Alert Notifications</h3>
            <div className="space-y-4">
              <label className="flex items-start space-x-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={desktopNotifications}
                  onChange={(e) => setDesktopNotifications(e.target.checked)}
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
                  checked={emailNotifications}
                  onChange={(e) => setEmailNotifications(e.target.checked)}
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
                  checked={alertSounds}
                  onChange={(e) => setAlertSounds(e.target.checked)}
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

          <div className="bg-warning/10 border border-warning/30 rounded-lg p-4 text-sm text-gray-300">
            <p className="font-medium text-warning mb-1">🚧 Coming Soon</p>
            <p>Advanced notification features including email and webhook integrations are planned for a future release.</p>
          </div>
        </div>
      </div>
    </div>
  );
};

// Appearance Settings Component
const AppearanceSettings: React.FC = () => {
  const [theme, setTheme] = useState<'dark' | 'light'>('dark');
  const [accentColor, setAccentColor] = useState('#00d9ff');

  return (
    <div className="flex flex-col h-full bg-bg-root">
      <div className="p-6 border-b border-gray-800">
        <div className="flex items-center space-x-2">
          <Palette className="h-5 w-5 text-accent" />
          <h2 className="text-xl font-bold text-white">Appearance</h2>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-2xl space-y-6">
          <div className="bg-bg-sidebar border border-gray-700 rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">Theme</h3>
            <div className="space-y-3">
              <label className="flex items-center space-x-3 cursor-pointer">
                <input
                  type="radio"
                  name="theme"
                  checked={theme === 'dark'}
                  onChange={() => setTheme('dark')}
                  className="h-4 w-4 border-gray-600 bg-bg-root text-accent focus:ring-accent focus:ring-offset-0"
                />
                <span className="text-white text-sm">Dark Theme (Current)</span>
              </label>
              <label className="flex items-center space-x-3 cursor-pointer opacity-50">
                <input
                  type="radio"
                  name="theme"
                  checked={theme === 'light'}
                  onChange={() => setTheme('light')}
                  disabled
                  className="h-4 w-4 border-gray-600 bg-bg-root text-accent focus:ring-accent focus:ring-offset-0"
                />
                <span className="text-white text-sm">Light Theme (Coming Soon)</span>
              </label>
            </div>
          </div>

          <div className="bg-bg-sidebar border border-gray-700 rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">Accent Color</h3>
            <div className="flex items-center space-x-4">
              <input
                type="color"
                value={accentColor}
                onChange={(e) => setAccentColor(e.target.value)}
                className="h-12 w-24 rounded border border-gray-700 bg-bg-root cursor-pointer"
              />
              <div>
                <p className="text-white text-sm font-medium">{accentColor}</p>
                <p className="text-gray-400 text-xs mt-1">Current accent color</p>
              </div>
            </div>
            <p className="text-xs text-gray-500 mt-3">
              Note: Color changes will take effect after app restart
            </p>
          </div>

          <div className="bg-warning/10 border border-warning/30 rounded-lg p-4 text-sm text-gray-300">
            <p className="font-medium text-warning mb-1">🚧 Coming Soon</p>
            <p>Theme customization and additional appearance options are planned for a future release.</p>
          </div>
        </div>
      </div>
    </div>
  );
};

// Data Settings Component
const DataSettings: React.FC = () => {
  return (
    <div className="flex flex-col h-full bg-bg-root">
      <div className="p-6 border-b border-gray-800">
        <div className="flex items-center space-x-2">
          <Database className="h-5 w-5 text-accent" />
          <h2 className="text-xl font-bold text-white">Data & Storage</h2>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-2xl space-y-6">
          <div className="bg-bg-sidebar border border-gray-700 rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">Database</h3>
            <div className="space-y-3">
              <div className="flex justify-between items-center">
                <span className="text-gray-400 text-sm">Database Location</span>
                <span className="text-white text-sm font-mono">~/AppData/Roaming/com.tauri.dev/quasar.db</span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-gray-400 text-sm">Database Size</span>
                <span className="text-white text-sm">~5 MB</span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-gray-400 text-sm">Metrics Retention</span>
                <span className="text-white text-sm">30 days</span>
              </div>
            </div>
          </div>

          <div className="bg-bg-sidebar border border-gray-700 rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">Backup & Export</h3>
            <div className="space-y-3">
              <button className="w-full bg-bg-root border border-gray-700 hover:border-accent text-white py-2.5 px-4 rounded-lg text-sm font-medium transition-all text-left">
                Export Database Backup
              </button>
              <button className="w-full bg-bg-root border border-gray-700 hover:border-accent text-white py-2.5 px-4 rounded-lg text-sm font-medium transition-all text-left">
                Import Database Backup
              </button>
              <button className="w-full bg-bg-root border border-alert/50 hover:border-alert text-white py-2.5 px-4 rounded-lg text-sm font-medium transition-all text-left">
                Clear All Metrics Data
              </button>
            </div>
          </div>

          <div className="bg-warning/10 border border-warning/30 rounded-lg p-4 text-sm text-gray-300">
            <p className="font-medium text-warning mb-1">🚧 Coming Soon</p>
            <p>Database backup, export, and management features are planned for a future release.</p>
          </div>
        </div>
      </div>
    </div>
  );
};

// About Settings Component
const AboutSettings: React.FC = () => {
  return (
    <div className="flex flex-col h-full bg-bg-root">
      <div className="p-6 border-b border-gray-800">
        <div className="flex items-center space-x-2">
          <Info className="h-5 w-5 text-accent" />
          <h2 className="text-xl font-bold text-white">About</h2>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-2xl space-y-6">
          <div className="bg-bg-sidebar border border-gray-700 rounded-lg p-6 text-center">
            <div className="inline-flex items-center justify-center w-20 h-20 bg-accent/10 rounded-2xl mb-4">
              <Shield className="h-10 w-10 text-accent" />
            </div>
            <h2 className="text-2xl font-bold text-white mb-2">Quasar</h2>
            <p className="text-accent font-mono text-sm mb-4">v0.1.0-alpha</p>
            <p className="text-gray-400 text-sm">
              Remote Infrastructure Management Platform
            </p>
          </div>

          <div className="bg-bg-sidebar border border-gray-700 rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">System Information</h3>
            <div className="space-y-3 text-sm">
              <div className="flex justify-between">
                <span className="text-gray-400">Platform</span>
                <span className="text-white">Windows</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-400">Architecture</span>
                <span className="text-white">x86_64</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-400">Tauri Version</span>
                <span className="text-white">2.x</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-400">React Version</span>
                <span className="text-white">18.x</span>
              </div>
            </div>
          </div>

          <div className="bg-bg-sidebar border border-gray-700 rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">Features</h3>
            <div className="grid grid-cols-2 gap-3 text-sm">
              <div className="flex items-center space-x-2">
                <div className="w-2 h-2 bg-success rounded-full"></div>
                <span className="text-gray-300">SSH Management</span>
              </div>
              <div className="flex items-center space-x-2">
                <div className="w-2 h-2 bg-success rounded-full"></div>
                <span className="text-gray-300">SFTP Transfer</span>
              </div>
              <div className="flex items-center space-x-2">
                <div className="w-2 h-2 bg-success rounded-full"></div>
                <span className="text-gray-300">System Monitoring</span>
              </div>
              <div className="flex items-center space-x-2">
                <div className="w-2 h-2 bg-success rounded-full"></div>
                <span className="text-gray-300">Security Vault</span>
              </div>
              <div className="flex items-center space-x-2">
                <div className="w-2 h-2 bg-success rounded-full"></div>
                <span className="text-gray-300">Alert System</span>
              </div>
              <div className="flex items-center space-x-2">
                <div className="w-2 h-2 bg-success rounded-full"></div>
                <span className="text-gray-300">AI Assistant</span>
              </div>
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

export default SettingsView;
