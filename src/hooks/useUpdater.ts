import { useCallback, useEffect, useRef, useState } from 'react';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { getErrorMessage } from '../lib/utils';

export type UpdateStatus = 'idle' | 'checking' | 'upToDate' | 'available' | 'downloading' | 'error';

export interface UpdaterState {
  status: UpdateStatus;
  version: string;
  error: string;
  checkForUpdates: () => Promise<void>;
  installUpdate: () => Promise<void>;
}

/**
 * Wraps the Tauri updater plugin. `autoCheck` runs one check on mount (for a
 * startup banner); callers that only want a manual "Check for Updates"
 * button should leave it false and call `checkForUpdates()` themselves.
 */
export function useUpdater(autoCheck: boolean): UpdaterState {
  const [status, setStatus] = useState<UpdateStatus>('idle');
  const [version, setVersion] = useState('');
  const [error, setError] = useState('');
  const pendingUpdateRef = useRef<Update | null>(null);
  const cancelledRef = useRef(false);

  const checkForUpdates = useCallback(async () => {
    // A prior available-but-uninstalled update holds a Rust-side resource
    // that must be explicitly released before we discard the reference.
    pendingUpdateRef.current?.close();
    pendingUpdateRef.current = null;

    setStatus('checking');
    setError('');
    try {
      const update = await check();
      if (cancelledRef.current) {
        update?.close();
        return;
      }
      if (update) {
        pendingUpdateRef.current = update;
        setVersion(update.version);
        setStatus('available');
      } else {
        setStatus('upToDate');
      }
    } catch (err) {
      if (cancelledRef.current) return;
      setError(getErrorMessage(err));
      setStatus('error');
    }
  }, []);

  const installUpdate = useCallback(async () => {
    const update = pendingUpdateRef.current;
    if (!update) return;
    setStatus('downloading');
    setError('');
    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch (err) {
      if (cancelledRef.current) return;
      setError(getErrorMessage(err));
      setStatus('error');
    }
  }, []);

  useEffect(() => {
    cancelledRef.current = false;
    if (autoCheck) {
      checkForUpdates();
    }
    return () => {
      cancelledRef.current = true;
      pendingUpdateRef.current?.close();
      pendingUpdateRef.current = null;
    };
  }, [autoCheck, checkForUpdates]);

  return { status, version, error, checkForUpdates, installUpdate };
}
