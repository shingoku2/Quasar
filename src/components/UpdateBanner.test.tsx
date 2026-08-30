import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import UpdateBanner from './UpdateBanner';
import '@testing-library/jest-dom';

const { checkMock, relaunchMock, closeMock } = vi.hoisted(() => ({
  checkMock: vi.fn(),
  relaunchMock: vi.fn(() => Promise.resolve()),
  closeMock: vi.fn(() => Promise.resolve()),
}));

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: checkMock,
}));

vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: relaunchMock,
}));

function makeUpdate(version: string, overrides: Partial<{ downloadAndInstall: () => Promise<void> }> = {}) {
  return {
    version,
    close: closeMock,
    downloadAndInstall: overrides.downloadAndInstall ?? vi.fn(() => Promise.resolve()),
  };
}

describe('UpdateBanner', () => {
  beforeEach(() => {
    checkMock.mockReset();
    relaunchMock.mockClear();
    closeMock.mockClear();
  });

  it('renders nothing when no update is available', async () => {
    checkMock.mockResolvedValue(null);
    const { container } = render(<UpdateBanner autoCheck />);

    await waitFor(() => expect(checkMock).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });

  it('shows an install prompt when an update is available', async () => {
    checkMock.mockResolvedValue(makeUpdate('1.2.0'));
    render(<UpdateBanner autoCheck />);

    await waitFor(() => {
      expect(screen.getByText(/1\.2\.0 is available/)).toBeInTheDocument();
    });
    expect(screen.getByRole('button', { name: 'Install & Restart' })).toBeInTheDocument();
  });

  it('downloads, installs, and relaunches when Install & Restart is clicked', async () => {
    const downloadAndInstall = vi.fn(() => Promise.resolve());
    checkMock.mockResolvedValue(makeUpdate('1.2.0', { downloadAndInstall }));
    render(<UpdateBanner autoCheck />);

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Install & Restart' })).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button', { name: 'Install & Restart' }));

    await waitFor(() => {
      expect(downloadAndInstall).toHaveBeenCalled();
      expect(relaunchMock).toHaveBeenCalled();
    });
  });

  it('can be dismissed', async () => {
    checkMock.mockResolvedValue(makeUpdate('1.2.0'));
    const { container } = render(<UpdateBanner autoCheck />);

    await waitFor(() => {
      expect(screen.getByText(/1\.2\.0 is available/)).toBeInTheDocument();
    });
    fireEvent.click(screen.getByLabelText('Dismiss update notification'));
    expect(container).toBeEmptyDOMElement();
  });

  it('stays hidden when the background update check fails', async () => {
    checkMock.mockRejectedValue(new Error('network unreachable'));
    const { container } = render(<UpdateBanner autoCheck />);

    await waitFor(() => expect(checkMock).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });
});
