import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useSshHostKeyVerification } from './useSshHostKeyVerification';

describe('useSshHostKeyVerification', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('queues concurrent host key prompts and processes them FIFO', async () => {
    let verificationListener: ((event: { payload: {
      requestId: string;
      host: string;
      port: number;
      fingerprint: string;
      keyType: string;
      keyBytes: number[];
      status: string;
      message: string;
    } }) => void) | undefined;

    vi.mocked(listen).mockImplementation(async (_eventName, callback) => {
      verificationListener = callback as typeof verificationListener;
      return () => {};
    });

    const { result } = renderHook(() => useSshHostKeyVerification());

    expect(verificationListener).toBeDefined();

    act(() => {
      verificationListener?.({
        payload: {
          requestId: 'req-1',
          host: 'host-1',
          port: 22,
          fingerprint: 'fp-1',
          keyType: 'ssh-ed25519',
          keyBytes: [1, 2, 3],
          status: 'Unknown',
          message: 'first',
        },
      });
      verificationListener?.({
        payload: {
          requestId: 'req-2',
          host: 'host-2',
          port: 22,
          fingerprint: 'fp-2',
          keyType: 'ssh-ed25519',
          keyBytes: [4, 5, 6],
          status: 'Unknown',
          message: 'second',
        },
      });
    });

    expect(result.current.promptData?.host).toBe('host-1');

    await act(async () => {
      result.current.handleReject();
    });

    expect(vi.mocked(invoke)).toHaveBeenCalledWith('respond_ssh_host_key_verification', {
      requestId: 'req-1',
      accepted: false,
    });

    await waitFor(() => {
      expect(result.current.promptData?.host).toBe('host-2');
    });
  });

  it('does not dequeue the current prompt when handleTrust fails with a non-dismissible error', async () => {
    let verificationListener: ((event: { payload: {
      requestId: string;
      host: string;
      port: number;
      fingerprint: string;
      keyType: string;
      keyBytes: number[];
      status: string;
      message: string;
    } }) => void) | undefined;

    vi.mocked(listen).mockImplementation(async (_eventName, callback) => {
      verificationListener = callback as typeof verificationListener;
      return () => {};
    });

    const approvalError = new Error('network error');
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});

    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === 'trust_ssh_host_key') {
        throw approvalError;
      }
      return undefined;
    });

    const { result } = renderHook(() => useSshHostKeyVerification());

    act(() => {
      verificationListener?.({
        payload: {
          requestId: 'req-approval',
          host: 'host-1',
          port: 22,
          fingerprint: 'fp-1',
          keyType: 'ssh-ed25519',
          keyBytes: [1, 2, 3],
          status: 'Unknown',
          message: 'first',
        },
      });
      verificationListener?.({
        payload: {
          requestId: 'req-approval-2',
          host: 'host-2',
          port: 22,
          fingerprint: 'fp-2',
          keyType: 'ssh-ed25519',
          keyBytes: [4, 5, 6],
          status: 'Unknown',
          message: 'second',
        },
      });
    });

    await act(async () => {
      await result.current.handleTrust(true);
    });

    await waitFor(() => {
      expect(consoleError).toHaveBeenCalledWith(
        'Failed to approve host key:',
        approvalError
      );
    });

    expect(invoke).toHaveBeenCalledWith(
      'trust_ssh_host_key',
      expect.objectContaining({ requestId: 'req-approval' })
    );

    // Prompt is NOT dequeued
    expect(result.current.promptData?.host).toBe('host-1');

    consoleError.mockRestore();
  });

  // The backend rejects the handshake when the user declines its native changed-key dialog.
  it('dequeues the prompt when the native changed-key confirmation is declined', async () => {
    let verificationListener: ((event: { payload: Record<string, unknown> }) => void) | undefined;
    vi.mocked(listen).mockImplementation(async (_eventName, callback) => {
      verificationListener = callback as typeof verificationListener;
      return () => {};
    });
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === 'respond_ssh_host_key_verification') throw 'Cancelled';
      return undefined;
    });

    const { result } = renderHook(() => useSshHostKeyVerification());
    act(() => {
      verificationListener?.({
        payload: {
          requestId: 'req-changed',
          host: 'host-1',
          port: 22,
          fingerprint: 'fp-new',
          keyType: 'ssh-ed25519',
          keyBytes: [1],
          status: 'Changed',
          message: 'changed',
          oldFingerprint: 'fp-old',
        },
      });
    });
    await waitFor(() => expect(result.current.promptData?.host).toBe('host-1'));

    await act(async () => {
      await result.current.handleTrust(false);
    });

    await waitFor(() => expect(result.current.promptData).toBeNull());
    consoleError.mockRestore();
  });

  it('dequeues the current prompt when handleTrust fails with a dismissible error', async () => {
    let verificationListener: ((event: { payload: {
      requestId: string;
      host: string;
      port: number;
      fingerprint: string;
      keyType: string;
      keyBytes: number[];
      status: string;
      message: string;
    } }) => void) | undefined;

    vi.mocked(listen).mockImplementation(async (_eventName, callback) => {
      verificationListener = callback as typeof verificationListener;
      return () => {};
    });

    const dismissibleError = new Error('Host key approval request is no longer pending');
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});

    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === 'trust_ssh_host_key') {
        throw dismissibleError;
      }
      return undefined;
    });

    const { result } = renderHook(() => useSshHostKeyVerification());

    act(() => {
      verificationListener?.({
        payload: {
          requestId: 'req-approval',
          host: 'host-1',
          port: 22,
          fingerprint: 'fp-1',
          keyType: 'ssh-ed25519',
          keyBytes: [1, 2, 3],
          status: 'Unknown',
          message: 'first',
        },
      });
      verificationListener?.({
        payload: {
          requestId: 'req-approval-2',
          host: 'host-2',
          port: 22,
          fingerprint: 'fp-2',
          keyType: 'ssh-ed25519',
          keyBytes: [4, 5, 6],
          status: 'Unknown',
          message: 'second',
        },
      });
    });

    await act(async () => {
      await result.current.handleTrust(true);
    });

    await waitFor(() => {
      expect(consoleError).toHaveBeenCalledWith(
        'Failed to approve host key:',
        dismissibleError
      );
    });

    expect(invoke).toHaveBeenCalledWith(
      'trust_ssh_host_key',
      expect.objectContaining({ requestId: 'req-approval' })
    );

    await waitFor(() => {
      expect(result.current.promptData?.host).toBe('host-2');
    });

    consoleError.mockRestore();
  });

  it('does not dequeue the current prompt when the backend response fails', async () => {
    let verificationListener: ((event: { payload: {
      requestId: string;
      host: string;
      port: number;
      fingerprint: string;
      keyType: string;
      keyBytes: number[];
      status: string;
      message: string;
    } }) => void) | undefined;

    vi.mocked(listen).mockImplementation(async (_eventName, callback) => {
      verificationListener = callback as typeof verificationListener;
      return () => {};
    });

    vi.mocked(invoke)
      .mockRejectedValueOnce(new Error('still waiting'))
      .mockResolvedValueOnce(undefined);

    const { result } = renderHook(() => useSshHostKeyVerification());

    act(() => {
      verificationListener?.({
        payload: {
          requestId: 'req-1',
          host: 'host-1',
          port: 22,
          fingerprint: 'fp-1',
          keyType: 'ssh-ed25519',
          keyBytes: [1, 2, 3],
          status: 'Unknown',
          message: 'first',
        },
      });
      verificationListener?.({
        payload: {
          requestId: 'req-2',
          host: 'host-2',
          port: 22,
          fingerprint: 'fp-2',
          keyType: 'ssh-ed25519',
          keyBytes: [4, 5, 6],
          status: 'Unknown',
          message: 'second',
        },
      });
    });

    await act(async () => {
      result.current.handleReject();
    });

    expect(result.current.promptData?.host).toBe('host-1');

    await act(async () => {
      result.current.handleReject();
    });

    await waitFor(() => {
      expect(result.current.promptData?.host).toBe('host-2');
    });
  });

  it('dequeues expired backend prompts so later requests are not blocked', async () => {
    let verificationListener: ((event: { payload: {
      requestId: string;
      host: string;
      port: number;
      fingerprint: string;
      keyType: string;
      keyBytes: number[];
      status: string;
      message: string;
    } }) => void) | undefined;

    vi.mocked(listen).mockImplementation(async (_eventName, callback) => {
      verificationListener = callback as typeof verificationListener;
      return () => {};
    });

    vi.mocked(invoke)
      .mockRejectedValueOnce(new Error('Host key approval request is no longer pending'))
      .mockResolvedValueOnce(undefined);

    const { result } = renderHook(() => useSshHostKeyVerification());

    act(() => {
      verificationListener?.({
        payload: {
          requestId: 'req-1',
          host: 'host-1',
          port: 22,
          fingerprint: 'fp-1',
          keyType: 'ssh-ed25519',
          keyBytes: [1, 2, 3],
          status: 'Unknown',
          message: 'first',
        },
      });
      verificationListener?.({
        payload: {
          requestId: 'req-2',
          host: 'host-2',
          port: 22,
          fingerprint: 'fp-2',
          keyType: 'ssh-ed25519',
          keyBytes: [4, 5, 6],
          status: 'Unknown',
          message: 'second',
        },
      });
    });

    await act(async () => {
      result.current.handleReject();
    });

    await waitFor(() => {
      expect(result.current.promptData?.host).toBe('host-2');
    });
  });

  it('ignores duplicate actions while the current prompt response is still in flight', async () => {
    let verificationListener: ((event: { payload: {
      requestId: string;
      host: string;
      port: number;
      fingerprint: string;
      keyType: string;
      keyBytes: number[];
      status: string;
      message: string;
    } }) => void) | undefined;
    let resolveInvoke: (() => void) | undefined;

    vi.mocked(listen).mockImplementation(async (_eventName, callback) => {
      verificationListener = callback as typeof verificationListener;
      return () => {};
    });

    vi.mocked(invoke).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveInvoke = () => resolve(undefined);
        })
    );

    const { result } = renderHook(() => useSshHostKeyVerification());

    act(() => {
      verificationListener?.({
        payload: {
          requestId: 'req-1',
          host: 'host-1',
          port: 22,
          fingerprint: 'fp-1',
          keyType: 'ssh-ed25519',
          keyBytes: [1, 2, 3],
          status: 'Unknown',
          message: 'first',
        },
      });
    });

    act(() => {
      result.current.handleReject();
      result.current.handleReject();
    });

    expect(vi.mocked(invoke)).toHaveBeenCalledTimes(1);
    expect(result.current.promptData?.host).toBe('host-1');

    await act(async () => {
      resolveInvoke?.();
    });

    await waitFor(() => {
      expect(result.current.promptData).toBeNull();
    });
  });

  type HostKeyPayload = {
    requestId: string;
    host: string;
    port: number;
    fingerprint: string;
    keyType: string;
    keyBytes: number[];
    status: string;
    message: string;
    oldFingerprint?: string | null;
  };

  const captureListener = () => {
    const holder: { fire?: (event: { payload: HostKeyPayload }) => void } = {};
    vi.mocked(listen).mockImplementation(async (_eventName, callback) => {
      holder.fire = callback as typeof holder.fire;
      return () => {};
    });
    return holder;
  };

  const payload = (overrides: Partial<HostKeyPayload> = {}): HostKeyPayload => ({
    requestId: 'req-x',
    host: 'host-x',
    port: 22,
    fingerprint: 'fp-new',
    keyType: 'ssh-key',
    keyBytes: [9, 9, 9],
    status: 'Unknown',
    message: 'prose',
    ...overrides,
  });

  // IPC-002: the webview names the pending request only; the backend persists the key the
  // handshake actually presented.
  it('trusts permanently by request id alone, sending no key material', async () => {
    const listener = captureListener();
    vi.mocked(invoke).mockResolvedValue(undefined);
    const { result } = renderHook(() => useSshHostKeyVerification());
    act(() => listener.fire?.({ payload: payload() }));

    await act(async () => {
      result.current.handleTrust(true);
    });

    expect(vi.mocked(invoke)).toHaveBeenCalledWith('trust_ssh_host_key', { requestId: 'req-x' });
  });

  // TEST-012: the live trust-once branch answers the pending request and persists nothing.
  it('trusts once via respond_ssh_host_key_verification without persisting', async () => {
    const listener = captureListener();
    vi.mocked(invoke).mockResolvedValue(undefined);
    const { result } = renderHook(() => useSshHostKeyVerification());
    act(() => listener.fire?.({ payload: payload() }));

    await act(async () => {
      result.current.handleTrust(false);
    });

    expect(vi.mocked(invoke)).toHaveBeenCalledWith('respond_ssh_host_key_verification', {
      requestId: 'req-x',
      accepted: true,
    });
    expect(vi.mocked(invoke)).not.toHaveBeenCalledWith('trust_ssh_host_key', expect.anything());
  });

  // FE-010: the "previous fingerprint" is the stored one, not the prose message.
  it('uses the backend oldFingerprint for a changed key', () => {
    const listener = captureListener();
    const { result } = renderHook(() => useSshHostKeyVerification());
    act(() =>
      listener.fire?.({
        payload: payload({ status: 'Changed', message: 'WARNING ... New: fp-new', oldFingerprint: 'fp-old' }),
      })
    );
    expect(result.current.promptData?.isChanged).toBe(true);
    expect(result.current.promptData?.oldFingerprint).toBe('fp-old');
  });

  // FE-007: unmounting before listen() resolves must still unlisten.
  it('unlistens even when unmounted before listen resolves', async () => {
    const unlisten = vi.fn();
    let resolveListen: (fn: () => void) => void = () => {};
    vi.mocked(listen).mockImplementation(
      () => new Promise((resolve) => { resolveListen = resolve; })
    );
    const { unmount } = renderHook(() => useSshHostKeyVerification());
    unmount();
    await act(async () => {
      resolveListen(unlisten);
    });
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
