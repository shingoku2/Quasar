import React, { useState, useEffect, useCallback } from 'react';
import { Palette } from 'lucide-react';
import { applyAppearance, defaultSettings, loadStoredSettings, saveStoredSettings } from './appearanceStore';
import type { StoredSettings } from './appearanceStore';

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


export default AppearanceSettings;
