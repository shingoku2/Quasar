import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import SshHostKeyPrompt from './SshHostKeyPrompt';
import '@testing-library/jest-dom';

const defaultProps = {
  host: 'server.example.com',
  port: 22,
  fingerprint: 'SHA256:abcdefghijklmnop1234567890',
  keyType: 'ed25519',
  onTrust: vi.fn(),
  onReject: vi.fn(),
};

describe('SshHostKeyPrompt', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('new host (isChanged = false)', () => {
    it('renders unknown host title', () => {
      render(<SshHostKeyPrompt {...defaultProps} />);
      expect(screen.getByText('Unknown SSH Host')).toBeInTheDocument();
    });

    it('shows first-time connection message', () => {
      render(<SshHostKeyPrompt {...defaultProps} />);
      expect(screen.getByText(/first time connecting to/i)).toBeInTheDocument();
    });

    it('displays the host and port', () => {
      render(<SshHostKeyPrompt {...defaultProps} />);
      // "server.example.com:22" appears in both the description text and the Host field
      expect(screen.getAllByText('server.example.com:22').length).toBeGreaterThanOrEqual(1);
    });

    it('displays the key type', () => {
      render(<SshHostKeyPrompt {...defaultProps} />);
      expect(screen.getByText(/ed25519/i)).toBeInTheDocument();
    });

    it('displays the fingerprint', () => {
      render(<SshHostKeyPrompt {...defaultProps} />);
      expect(screen.getByText('SHA256:abcdefghijklmnop1234567890')).toBeInTheDocument();
    });

    it('shows Trust & Connect button (not "Accept New Key")', () => {
      render(<SshHostKeyPrompt {...defaultProps} />);
      expect(screen.getByRole('button', { name: /Trust & Connect/i })).toBeInTheDocument();
      expect(screen.queryByRole('button', { name: /Accept New Key/i })).not.toBeInTheDocument();
    });

    it('calls onTrust with true when checkbox is checked (default) and Trust is clicked', () => {
      render(<SshHostKeyPrompt {...defaultProps} />);
      fireEvent.click(screen.getByRole('button', { name: /Trust & Connect/i }));
      expect(defaultProps.onTrust).toHaveBeenCalledWith(true);
    });

    it('calls onTrust with false when checkbox is unchecked before clicking Trust', () => {
      render(<SshHostKeyPrompt {...defaultProps} />);
      const checkbox = screen.getByRole('checkbox');
      fireEvent.click(checkbox); // uncheck
      fireEvent.click(screen.getByRole('button', { name: /Trust & Connect/i }));
      expect(defaultProps.onTrust).toHaveBeenCalledWith(false);
    });

    it('calls onReject when Cancel Connection is clicked', () => {
      render(<SshHostKeyPrompt {...defaultProps} />);
      fireEvent.click(screen.getByRole('button', { name: /Cancel Connection/i }));
      expect(defaultProps.onReject).toHaveBeenCalledOnce();
    });

    it('copies fingerprint to clipboard when copy button is clicked', async () => {
      const writeText = vi.fn().mockResolvedValue(undefined);
      Object.defineProperty(navigator, 'clipboard', {
        value: { writeText },
        writable: true,
        configurable: true,
      });

      render(<SshHostKeyPrompt {...defaultProps} />);
      fireEvent.click(screen.getByTitle('Copy fingerprint'));

      await waitFor(() => {
        expect(writeText).toHaveBeenCalledWith('SHA256:abcdefghijklmnop1234567890');
      });
    });
  });

  describe('changed host key (isChanged = true)', () => {
    const changedProps = {
      ...defaultProps,
      isChanged: true,
      oldFingerprint: 'SHA256:oldfingerprint0987654321',
    };

    it('renders security warning title', () => {
      render(<SshHostKeyPrompt {...changedProps} />);
      expect(screen.getByText(/Host Key Changed - Security Warning/i)).toBeInTheDocument();
    });

    it('shows man-in-the-middle attack warning', () => {
      render(<SshHostKeyPrompt {...changedProps} />);
      expect(screen.getByText(/man-in-the-middle/i)).toBeInTheDocument();
    });

    it('shows "Accept New Key & Connect" button instead of "Trust & Connect"', () => {
      render(<SshHostKeyPrompt {...changedProps} />);
      expect(screen.getByRole('button', { name: /Accept New Key & Connect/i })).toBeInTheDocument();
      expect(screen.queryByRole('button', { name: /^Trust & Connect$/i })).not.toBeInTheDocument();
    });

    it('displays both new and old fingerprints', () => {
      render(<SshHostKeyPrompt {...changedProps} />);
      expect(screen.getByText('SHA256:abcdefghijklmnop1234567890')).toBeInTheDocument();
      expect(screen.getByText('SHA256:oldfingerprint0987654321')).toBeInTheDocument();
    });

    it('shows New Fingerprint label', () => {
      render(<SshHostKeyPrompt {...changedProps} />);
      expect(screen.getByText('New Fingerprint')).toBeInTheDocument();
    });

    it('shows Previous Fingerprint label', () => {
      render(<SshHostKeyPrompt {...changedProps} />);
      expect(screen.getByText('Previous Fingerprint')).toBeInTheDocument();
    });

    it('calls onTrust when Accept New Key is clicked', () => {
      render(<SshHostKeyPrompt {...changedProps} />);
      fireEvent.click(screen.getByRole('button', { name: /Accept New Key & Connect/i }));
      expect(changedProps.onTrust).toHaveBeenCalledWith(true);
    });
  });
});
