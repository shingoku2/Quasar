import { useCallback, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getErrorMessage } from '../../../lib/utils';
import { useOnViewShown } from '../../../hooks/useViewVisibility';
import type { CredentialSummary } from './types';

/**
 * The credential list: loaded each time the view is shown, searchable, and
 * reloaded after a delete. Only the latest request's result is applied, so a slow
 * list can't overwrite a newer search (or the reverse) (FE-014).
 */
export function useCredentials() {
  const [credentials, setCredentials] = useState<CredentialSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState('');
  const latest = useRef(0);

  const run = useCallback(async (request: () => Promise<CredentialSummary[]>, failure: string) => {
    const id = ++latest.current;
    setIsLoading(true);
    setError('');
    try {
      const result = await request();
      if (id === latest.current) setCredentials(result);
    } catch (err) {
      if (id === latest.current) setError(getErrorMessage(err, failure));
    } finally {
      if (id === latest.current) setIsLoading(false);
    }
  }, []);

  const load = useCallback(
    () => run(() => invoke<CredentialSummary[]>('list_credentials'), 'Failed to load credentials'),
    [run],
  );

  const search = useCallback(
    (query: string) =>
      query.trim()
        ? run(() => invoke<CredentialSummary[]>('search_credentials', { query }), 'Search failed')
        : load(),
    [run, load],
  );

  const remove = useCallback(async (id: string) => {
    try {
      await invoke('delete_credential', { credentialId: id });
      await load();
    } catch (err) {
      setError(getErrorMessage(err, 'Failed to delete credential'));
    }
  }, [load]);

  useOnViewShown(() => { void load(); });

  return { credentials, isLoading, error, setError, load, search, remove };
}
