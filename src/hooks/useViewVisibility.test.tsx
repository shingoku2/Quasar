import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, renderHook, act } from '@testing-library/react';
import { ViewVisibilityProvider, useIsViewVisible, useVisiblePolling } from './useViewVisibility';

/** Renders nothing; exists so the polling hook can be mounted under a provider. */
const PollProbe: React.FC<{ callback: () => void; intervalMs?: number }> = ({
  callback,
  intervalMs = 1000,
}) => {
  useVisiblePolling(callback, intervalMs);
  return null;
};

const tree = (visible: boolean, callback: () => void) => (
  <ViewVisibilityProvider visible={visible}>
    <PollProbe callback={callback} />
  </ViewVisibilityProvider>
);

describe('useViewVisibility', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('reports visible when no provider is present', () => {
    // Components rendered outside Layout must keep their previous behaviour.
    const { result } = renderHook(() => useIsViewVisible());
    expect(result.current).toBe(true);
  });

  it('reports the value supplied by the provider', () => {
    const { result } = renderHook(() => useIsViewVisible(), {
      wrapper: ({ children }) => <ViewVisibilityProvider visible={false}>{children}</ViewVisibilityProvider>,
    });
    expect(result.current).toBe(false);
  });

  it('polls immediately and on the interval while visible', async () => {
    const callback = vi.fn();
    render(tree(true, callback));

    expect(callback).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });
    expect(callback).toHaveBeenCalledTimes(4);
  });

  it('does not poll at all while hidden', async () => {
    const callback = vi.fn();
    render(tree(false, callback));

    expect(callback).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(10000);
    });
    expect(callback).not.toHaveBeenCalled();
  });

  it('skips ticks while a previous async poll is still in flight', async () => {
    // A poll that outlives its own interval must not stack up requests.
    let resolvePoll: (() => void) | undefined;
    const callback = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolvePoll = resolve;
        }),
    );

    render(tree(true, callback));
    expect(callback).toHaveBeenCalledTimes(1);

    // Three intervals elapse while the first call is still pending.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });
    expect(callback).toHaveBeenCalledTimes(1);

    // Once it settles, the next tick is allowed through again.
    await act(async () => {
      resolvePoll?.();
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(callback).toHaveBeenCalledTimes(2);
  });

  it('fires immediately when a hidden view becomes visible', () => {
    const callback = vi.fn();
    const { rerender } = render(tree(false, callback));
    expect(callback).not.toHaveBeenCalled();

    rerender(tree(true, callback));
    expect(callback).toHaveBeenCalledTimes(1);
  });

  it('stops polling when a visible view becomes hidden', async () => {
    const callback = vi.fn();
    const { rerender } = render(tree(true, callback));
    expect(callback).toHaveBeenCalledTimes(1);

    rerender(tree(false, callback));
    callback.mockClear();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(10000);
    });
    expect(callback).not.toHaveBeenCalled();
  });

  it('refetches immediately when a declared dependency changes', async () => {
    const callback = vi.fn();
    const Probe: React.FC<{ host: string }> = ({ host }) => {
      useVisiblePolling(() => callback(host), 30000, [host]);
      return null;
    };
    const withHost = (host: string) => (
      <ViewVisibilityProvider visible>
        <Probe host={host} />
      </ViewVisibilityProvider>
    );

    const { rerender } = render(withHost('10.0.0.1'));
    expect(callback).toHaveBeenCalledTimes(1);
    expect(callback).toHaveBeenLastCalledWith('10.0.0.1');

    // Must not wait out the 30s interval before showing the new host's status.
    rerender(withHost('10.0.0.2'));
    expect(callback).toHaveBeenCalledTimes(2);
    expect(callback).toHaveBeenLastCalledWith('10.0.0.2');
  });

  it('does not restart the interval when an unchanged dependency re-renders', async () => {
    const callback = vi.fn();
    const Probe: React.FC<{ host: string }> = ({ host }) => {
      useVisiblePolling(() => callback(host), 30000, [host]);
      return null;
    };
    const withHost = (host: string) => (
      <ViewVisibilityProvider visible>
        <Probe host={host} />
      </ViewVisibilityProvider>
    );

    const { rerender } = render(withHost('10.0.0.1'));
    expect(callback).toHaveBeenCalledTimes(1);

    rerender(withHost('10.0.0.1'));
    rerender(withHost('10.0.0.1'));
    expect(callback).toHaveBeenCalledTimes(1);
  });

  it('does not restart the interval when the callback identity changes', async () => {
    const spy = vi.fn();
    // A fresh inline arrow on every render is the common call pattern; the hook
    // holds the callback in a ref so the timer must survive re-renders.
    const treeWithInlineCallback = () => (
      <ViewVisibilityProvider visible>
        <PollProbe callback={() => spy()} />
      </ViewVisibilityProvider>
    );

    const { rerender } = render(treeWithInlineCallback());
    expect(spy).toHaveBeenCalledTimes(1);

    rerender(treeWithInlineCallback());
    rerender(treeWithInlineCallback());
    expect(spy).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(spy).toHaveBeenCalledTimes(2);
  });
});
