import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { useCredentials } from './useCredentials';
import type { CredentialSummary } from './types';

const cred = (id: string): CredentialSummary => ({
  id, name: id, username: 'root', credential_type: 'ssh', created_at: '2026-01-01T00:00:00Z',
});

describe('useCredentials (FE-014)', () => {
  it('ignores a slow list that finishes after a newer search', async () => {
    let finishList: (v: CredentialSummary[]) => void = () => {};
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === 'list_credentials') return new Promise((resolve) => { finishList = resolve; });
      if (cmd === 'search_credentials') return [cred('match')];
      return undefined;
    });

    const { result } = renderHook(() => useCredentials());
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('list_credentials'));
    await act(() => result.current.search('match'));
    expect(result.current.credentials.map((c) => c.id)).toEqual(['match']);

    await act(async () => { finishList([cred('stale')]); });
    expect(result.current.credentials.map((c) => c.id)).toEqual(['match']);
    expect(result.current.isLoading).toBe(false);
  });
});
