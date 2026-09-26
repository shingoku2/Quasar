import React, { useState } from 'react';
import { Settings, Shield, Bell, Palette, Database, Info } from 'lucide-react';
import VaultSettings from './vault/VaultSettings';
import NotificationsSettings from './settings/NotificationsSettings';
import AppearanceSettings from './settings/AppearanceSettings';
import DataSettings from './settings/DataSettings';
import AboutSettings from './settings/AboutSettings';

export type SettingsCategory = 'security' | 'notifications' | 'appearance' | 'data' | 'about';

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

/**
 * Where alerts go. This panel used to offer desktop, email and sound toggles that nothing
 * read (FE-013); they were removed rather than left as switches that do nothing.
 */

export default SettingsView;
