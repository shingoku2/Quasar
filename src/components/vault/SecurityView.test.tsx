import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import SecurityView from './SecurityView';
import '@testing-library/jest-dom';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Mock child components to isolate SecurityView tab logic
vi.mock('./CredentialManager', () => ({
  default: () => <div data-testid="credential-manager">CredentialManager</div>,
}));

vi.mock('./KnownHostsManager', () => ({
  default: () => <div data-testid="known-hosts-manager">KnownHostsManager</div>,
}));

vi.mock('./AuditLogViewer', () => ({
  default: () => <div data-testid="audit-log-viewer">AuditLogViewer</div>,
}));

describe('SecurityView', () => {
  it('renders with credentials tab active by default', () => {
    render(<SecurityView />);

    expect(screen.getByText('Credentials')).toBeInTheDocument();
    expect(screen.getByText('Known Hosts')).toBeInTheDocument();
    expect(screen.getByText('Audit Log')).toBeInTheDocument();
    expect(screen.getByTestId('credential-manager')).toBeInTheDocument();
  });

  it('switches to Known Hosts tab', () => {
    render(<SecurityView />);

    fireEvent.click(screen.getByText('Known Hosts'));
    expect(screen.getByTestId('known-hosts-manager')).toBeInTheDocument();
  });

  it('switches to Audit Log tab', () => {
    render(<SecurityView />);

    fireEvent.click(screen.getByText('Audit Log'));
    expect(screen.getByTestId('audit-log-viewer')).toBeInTheDocument();
  });

  it('switches back to Credentials tab', () => {
    render(<SecurityView />);

    fireEvent.click(screen.getByText('Known Hosts'));
    expect(screen.getByTestId('known-hosts-manager')).toBeInTheDocument();

    fireEvent.click(screen.getByText('Credentials'));
    expect(screen.getByTestId('credential-manager')).toBeInTheDocument();
  });
});
