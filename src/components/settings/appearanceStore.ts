const SETTINGS_STORAGE_KEY = 'quasar_settings';

export interface StoredSettings {
  appearance: {
    theme: 'dark' | 'light';
    accentColor: string;
  };
}

export const defaultSettings: StoredSettings = {
  appearance: {
    theme: 'dark',
    accentColor: '#00d4ff',
  },
};

export function loadStoredSettings(): StoredSettings {
  try {
    const raw = localStorage.getItem(SETTINGS_STORAGE_KEY);
    if (!raw) return defaultSettings;
    const parsed = JSON.parse(raw) as Partial<StoredSettings>;
    return {
      appearance: { ...defaultSettings.appearance, ...parsed.appearance },
    };
  } catch {
    return defaultSettings;
  }
}

export function saveStoredSettings(settings: StoredSettings) {
  try {
    localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // ignore
  }
}

export function applyAppearance(theme: 'dark' | 'light', accentColor: string) {
  document.documentElement.setAttribute('data-theme', theme);
  document.documentElement.style.setProperty('--color-accent', accentColor);
}

/**
 * Applies the saved theme and accent color. Called once at startup: it used to run only
 * when the Appearance panel was opened, so a saved theme was lost on every launch (FE-013).
 */
export function applyStoredAppearance(): void {
  const { theme, accentColor } = loadStoredSettings().appearance;
  applyAppearance(theme, accentColor);
}
