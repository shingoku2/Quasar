import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import SettingsView from './SettingsView';
import '@testing-library/jest-dom';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn((cmd: string) => {
    if (cmd === 'get_app_info') {
      return Promise.resolve({ version: '0.1.0', platform: 'windows', arch: 'x86_64', app_data_dir: '', db_path: '', db_size_bytes: 0 });
    }
    return Promise.resolve();
  }),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: vi.fn(() => Promise.resolve(null)),
  open: vi.fn(() => Promise.resolve(null)),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Mock VaultSettings since it has complex dependencies
vi.mock('./vault/VaultSettings', () => ({
  default: () => <div data-testid="vault-settings">VaultSettings</div>,
}));

describe('SettingsView', () => {
  it('renders settings sidebar with all categories', () => {
    render(<SettingsView />);

    expect(screen.getByText('Settings')).toBeInTheDocument();
    expect(screen.getByText('Security & Vault')).toBeInTheDocument();
    expect(screen.getByText('Notifications')).toBeInTheDocument();
    expect(screen.getByText('Appearance')).toBeInTheDocument();
    expect(screen.getByText('Data & Storage')).toBeInTheDocument();
    expect(screen.getByText('About')).toBeInTheDocument();
  });

  it('shows Security & Vault content by default', () => {
    render(<SettingsView />);

    expect(screen.getByTestId('vault-settings')).toBeInTheDocument();
  });

  it('switches to About category', () => {
    render(<SettingsView />);

    fireEvent.click(screen.getByText('About'));
    expect(screen.getByText('Quasar')).toBeInTheDocument();
    expect(screen.getByText(/v0\.1\.0/)).toBeInTheDocument();
  });

  it('switches to Notifications category', () => {
    render(<SettingsView />);

    fireEvent.click(screen.getByText('Notifications'));
    expect(screen.getByText('Desktop Notifications')).toBeInTheDocument();
  });

  it('switches to Appearance category', () => {
    render(<SettingsView />);

    fireEvent.click(screen.getByText('Appearance'));
    expect(screen.getByText('Dark Theme')).toBeInTheDocument();
  });

  it('switches to Data & Storage category', () => {
    render(<SettingsView />);

    fireEvent.click(screen.getByText('Data & Storage'));
    expect(screen.getByText('Export Database Backup')).toBeInTheDocument();
  });
});
