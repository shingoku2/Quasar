import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { useUpdater } from './useUpdater';

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: vi.fn(),
}));

describe('useUpdater', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('initializes in idle state without checking if autoCheck is false', () => {
    const { result } = renderHook(() => useUpdater(false));
    expect(result.current.status).toBe('idle');
    expect(result.current.version).toBe('');
    expect(result.current.error).toBe('');
    expect(check).not.toHaveBeenCalled();
  });

  it('automatically checks for updates on mount if autoCheck is true', async () => {
    vi.mocked(check).mockResolvedValueOnce(null);
    const { result } = renderHook(() => useUpdater(true));

    expect(result.current.status).toBe('checking');

    await waitFor(() => {
      expect(result.current.status).toBe('upToDate');
    });

    expect(check).toHaveBeenCalledTimes(1);
  });

  it('sets status to available when an update is found', async () => {
    const mockUpdate = {
      version: '1.2.3',
      close: vi.fn(),
      downloadAndInstall: vi.fn(),
    };
    vi.mocked(check).mockResolvedValueOnce(mockUpdate as any);

    const { result } = renderHook(() => useUpdater(false));

    act(() => {
      result.current.checkForUpdates();
    });

    expect(result.current.status).toBe('checking');

    await waitFor(() => {
      expect(result.current.status).toBe('available');
    });

    expect(result.current.version).toBe('1.2.3');
  });

  it('sets error state when check fails', async () => {
    vi.mocked(check).mockRejectedValueOnce(new Error('Network error'));

    const { result } = renderHook(() => useUpdater(false));

    act(() => {
      result.current.checkForUpdates();
    });

    await waitFor(() => {
      expect(result.current.status).toBe('error');
    });

    expect(result.current.error).toBe('Network error');
  });

  it('installs update and relaunches successfully', async () => {
    const mockUpdate = {
      version: '1.2.3',
      close: vi.fn(),
      downloadAndInstall: vi.fn().mockResolvedValueOnce(undefined),
    };
    vi.mocked(check).mockResolvedValueOnce(mockUpdate as any);

    const { result } = renderHook(() => useUpdater(false));

    await act(async () => {
      await result.current.checkForUpdates();
    });

    expect(result.current.status).toBe('available');

    let installPromise: Promise<void> = Promise.resolve();
    act(() => {
      installPromise = result.current.installUpdate();
    });

    expect(result.current.status).toBe('downloading');

    // Await the installUpdate() promise itself rather than polling for the
    // downloadAndInstall call: that call happens synchronously before the
    // first await, so waitFor could resolve before the continuation that
    // calls relaunch() has actually run.
    await act(async () => {
      await installPromise;
    });

    expect(mockUpdate.downloadAndInstall).toHaveBeenCalledTimes(1);
    expect(relaunch).toHaveBeenCalledTimes(1);
  });

  it('sets error state when installation fails', async () => {
    const mockUpdate = {
      version: '1.2.3',
      close: vi.fn(),
      downloadAndInstall: vi.fn().mockRejectedValueOnce(new Error('Install failed')),
    };
    vi.mocked(check).mockResolvedValueOnce(mockUpdate as any);

    const { result } = renderHook(() => useUpdater(false));

    await act(async () => {
      await result.current.checkForUpdates();
    });

    await act(async () => {
      await result.current.installUpdate();
    });

    expect(result.current.status).toBe('error');
    expect(result.current.error).toBe('Install failed');
  });

  it('cleans up pending update on unmount', async () => {
    const mockUpdate = {
      version: '1.2.3',
      close: vi.fn(),
      downloadAndInstall: vi.fn(),
    };
    vi.mocked(check).mockResolvedValueOnce(mockUpdate as any);

    const { result, unmount } = renderHook(() => useUpdater(false));

    await act(async () => {
      await result.current.checkForUpdates();
    });

    unmount();

    expect(mockUpdate.close).toHaveBeenCalledTimes(1);
  });

  it('handles rapid unmount during check without updating state', async () => {
    const mockUpdate = {
      version: '1.2.3',
      close: vi.fn(),
      downloadAndInstall: vi.fn(),
    };

    let resolveCheck: (value: any) => void;
    const checkPromise = new Promise((resolve) => {
      resolveCheck = resolve;
    });
    vi.mocked(check).mockReturnValueOnce(checkPromise as any);

    const { result, unmount } = renderHook(() => useUpdater(true));

    expect(result.current.status).toBe('checking');

    unmount();

    await act(async () => {
      resolveCheck(mockUpdate);
    });

    // The close should be called on the update since the component unmounted
    // before the check promise resolved
    expect(mockUpdate.close).toHaveBeenCalledTimes(1);
    // Because state updates after unmount shouldn't happen and shouldn't matter,
    // we're primarily verifying that the resource is closed.
  });

  it('closes old pending update when a new check is triggered', async () => {
    const mockUpdate1 = {
      version: '1.0.0',
      close: vi.fn(),
      downloadAndInstall: vi.fn(),
    };
    const mockUpdate2 = {
      version: '2.0.0',
      close: vi.fn(),
      downloadAndInstall: vi.fn(),
    };

    vi.mocked(check)
      .mockResolvedValueOnce(mockUpdate1 as any)
      .mockResolvedValueOnce(mockUpdate2 as any);

    const { result } = renderHook(() => useUpdater(false));

    // First check
    await act(async () => {
      await result.current.checkForUpdates();
    });
    expect(result.current.version).toBe('1.0.0');

    // Second check
    act(() => {
      result.current.checkForUpdates();
    });

    // Synchronously before the second check resolves, the first one should be closed
    expect(mockUpdate1.close).toHaveBeenCalledTimes(1);

    await waitFor(() => {
      expect(result.current.version).toBe('2.0.0');
    });
  });
});
