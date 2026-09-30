import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import AppearanceSettings from './AppearanceSettings';
import {
  applyAppearance,
  applyStoredAppearance,
  defaultSettings,
  loadStoredSettings,
  saveStoredSettings,
} from './appearanceStore';
import '@testing-library/jest-dom';

describe('AppearanceSettings and appearanceStore', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute('data-theme');
    document.documentElement.style.removeProperty('--color-accent');
  });

  afterEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute('data-theme');
    document.documentElement.style.removeProperty('--color-accent');
    vi.restoreAllMocks();
  });

  describe('appearanceStore', () => {
    it('loadStoredSettings returns defaults when nothing is stored', () => {
      expect(loadStoredSettings()).toEqual(defaultSettings);
    });

    it('loadStoredSettings parses stored settings and merges partials', () => {
      localStorage.setItem(
        'quasar_settings',
        JSON.stringify({ appearance: { theme: 'light' } }),
      );
      const loaded = loadStoredSettings();
      expect(loaded.appearance.theme).toBe('light');
      expect(loaded.appearance.accentColor).toBe(defaultSettings.appearance.accentColor);
    });

    it('loadStoredSettings returns defaults when JSON is malformed', () => {
      localStorage.setItem('quasar_settings', '{malformed-json');
      expect(loadStoredSettings()).toEqual(defaultSettings);
    });

    it('saveStoredSettings persists settings to localStorage', () => {
      saveStoredSettings({
        appearance: { theme: 'light', accentColor: '#ff0055' },
      });
      const raw = localStorage.getItem('quasar_settings');
      expect(raw).toBeDefined();
      expect(JSON.parse(raw!)).toEqual({
        appearance: { theme: 'light', accentColor: '#ff0055' },
      });
    });

    it('saveStoredSettings handles storage write errors without throwing', () => {
      vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
        throw new Error('QuotaExceededError');
      });
      expect(() => {
        saveStoredSettings(defaultSettings);
      }).not.toThrow();
    });

    it('applyAppearance sets data-theme and CSS variable on documentElement', () => {
      applyAppearance('light', '#123456');
      expect(document.documentElement.getAttribute('data-theme')).toBe('light');
      expect(
        document.documentElement.style.getPropertyValue('--color-accent'),
      ).toBe('#123456');
    });

    it('applyStoredAppearance loads and applies saved theme and color', () => {
      saveStoredSettings({
        appearance: { theme: 'light', accentColor: '#abcdef' },
      });
      applyStoredAppearance();
      expect(document.documentElement.getAttribute('data-theme')).toBe('light');
      expect(
        document.documentElement.style.getPropertyValue('--color-accent'),
      ).toBe('#abcdef');
    });
  });

  describe('AppearanceSettings component', () => {
    it('renders with dark theme by default and applies it to the document', () => {
      render(<AppearanceSettings />);
      expect(screen.getByText('Appearance')).toBeInTheDocument();
      const darkRadio = screen.getByLabelText('Dark Theme') as HTMLInputElement;
      const lightRadio = screen.getByLabelText('Light Theme') as HTMLInputElement;

      expect(darkRadio.checked).toBe(true);
      expect(lightRadio.checked).toBe(false);
      expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
      expect(
        document.documentElement.style.getPropertyValue('--color-accent'),
      ).toBe('#00d4ff');
    });

    it('switches to light theme and persists to localStorage and document attributes', () => {
      render(<AppearanceSettings />);
      const lightRadio = screen.getByLabelText('Light Theme') as HTMLInputElement;

      fireEvent.click(lightRadio);

      expect(lightRadio.checked).toBe(true);
      expect(document.documentElement.getAttribute('data-theme')).toBe('light');
      const stored = loadStoredSettings();
      expect(stored.appearance.theme).toBe('light');
    });

    it('switches accent color and updates document style and store', () => {
      render(<AppearanceSettings />);
      const colorInput = screen.getByDisplayValue('#00d4ff');

      fireEvent.change(colorInput, { target: { value: '#ff5500' } });

      expect(screen.getByText('#ff5500')).toBeInTheDocument();
      expect(
        document.documentElement.style.getPropertyValue('--color-accent'),
      ).toBe('#ff5500');
      const stored = loadStoredSettings();
      expect(stored.appearance.accentColor).toBe('#ff5500');
    });
  });
});
