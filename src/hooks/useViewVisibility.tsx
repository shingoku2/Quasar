import React, { createContext, useContext, useEffect, useRef } from 'react';

/**
 * Tracks whether the surrounding view is currently on screen.
 *
 * Layout keeps every view mounted and hides the inactive ones with CSS so that
 * their state (scroll position, form input, live SSH sessions) survives tab
 * switches. The side effect is that background polling in a hidden view keeps
 * running. Components wrap their polling in `useVisiblePolling` so that work
 * pauses while they are off screen.
 *
 * Defaults to `true` when no provider is present, so components rendered
 * outside Layout — including in tests — poll exactly as they did before.
 */
const ViewVisibilityContext = createContext<boolean>(true);

interface ViewVisibilityProviderProps {
  visible: boolean;
  children: React.ReactNode;
}

export const ViewVisibilityProvider: React.FC<ViewVisibilityProviderProps> = ({ visible, children }) => (
  <ViewVisibilityContext.Provider value={visible}>{children}</ViewVisibilityContext.Provider>
);

/** True when the surrounding view is the active one. */
export const useIsViewVisible = (): boolean => useContext(ViewVisibilityContext);

/**
 * Runs `callback` immediately and then every `intervalMs`, but only while the
 * surrounding view is visible. On becoming visible again the callback fires
 * right away so the view never renders stale data.
 *
 * If `callback` returns a promise, a tick is skipped while the previous run is
 * still in flight. Some backend polls (host health, for one) can take longer
 * than their own interval, and without this the requests would pile up.
 *
 * `callback` is held in a ref, so an inline function does not restart the timer
 * on every render.
 */
export const useVisiblePolling = (
  callback: () => void | Promise<void>,
  intervalMs: number,
  deps: readonly unknown[] = [],
): void => {
  const visible = useIsViewVisible();
  const savedCallback = useRef(callback);

  useEffect(() => {
    savedCallback.current = callback;
  }, [callback]);

  useEffect(() => {
    if (!visible) return;

    let cancelled = false;
    // Scoped to this effect instance on purpose. It exists to stop *repeated
    // ticks* of one interval from stacking; a restart (visibility or a changed
    // dependency) supersedes whatever was in flight, so it must begin unguarded
    // or the refetch would be swallowed by the request it replaces.
    let inFlight = false;

    const run = async () => {
      if (inFlight) return;
      inFlight = true;
      try {
        await savedCallback.current();
      } finally {
        inFlight = false;
      }
    };

    void run();
    const id = setInterval(() => {
      if (!cancelled) void run();
    }, intervalMs);

    return () => {
      cancelled = true;
      clearInterval(id);
    };
    // `deps` lets a caller restart the poll when the thing being polled changes
    // (for example the host a badge is tracking), so it refetches immediately
    // instead of showing stale data until the next tick.
  }, [visible, intervalMs, ...deps]);
};
