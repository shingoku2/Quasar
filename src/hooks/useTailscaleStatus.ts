import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getErrorMessage } from '../lib/utils';

/** Mirrors `src-tauri/src/tailscale.rs::TailscalePeer`. */
export interface TailscalePeer {
  id: string;
  hostname: string;
  dns_name: string;
  ipv4: string | null;
  os: string;
  online: boolean;
  tailscale_ssh: boolean;
  exit_node: boolean;
  preferred_address: string;
}

/** Mirrors `src-tauri/src/tailscale.rs::TailscaleStatus`. */
export interface TailscaleStatus {
  installed: boolean;
  backend_state: string;
  magic_dns_enabled: boolean;
  magic_dns_suffix: string | null;
  self_node: TailscalePeer | null;
  peers: TailscalePeer[];
}

const POLL_INTERVAL_MS = 30000;

/**
 * Finds the peer (if any) matching a saved host's stored address, comparing
 * case-insensitively against the peer's MagicDNS name, IPv4 address, and
 * plain hostname — saved hosts may have been added via any of the three.
 */
export function findTailscalePeer(
  status: TailscaleStatus | null,
  address: string | undefined | null,
): TailscalePeer | undefined {
  if (!status || !address) return undefined;
  const target = address.trim().toLowerCase();
  if (!target) return undefined;
  return status.peers.find((peer) =>
    [peer.dns_name, peer.ipv4, peer.hostname].some(
      (candidate) => !!candidate && candidate.toLowerCase() === target,
    ),
  );
}

export interface UseTailscaleStatusResult {
  status: TailscaleStatus | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

interface TailscaleStatusState {
  status: TailscaleStatus | null;
  loading: boolean;
  error: string | null;
}

const initialState: TailscaleStatusState = { status: null, loading: true, error: null };

// Module-level shared state. The hook is mounted in several places at once
// (RemoteManager, HostList, TailscalePeers), and every fetch spawns a
// `tailscale status --json` subprocess on the backend — so all subscribers
// share a single poll timer, a single in-flight request, and one cached
// result instead of each running their own 30s poll.
let sharedState: TailscaleStatusState = initialState;
const subscribers = new Set<(state: TailscaleStatusState) => void>();
let pollTimer: ReturnType<typeof setInterval> | null = null;
let inFlight: Promise<void> | null = null;

function publish(next: TailscaleStatusState) {
  sharedState = next;
  subscribers.forEach((notify) => notify(next));
}

/** Fetches status, coalescing concurrent callers onto one backend request. */
function fetchSharedStatus(): Promise<void> {
  if (inFlight) return inFlight;
  const request = (async () => {
    try {
      const result = await invoke<TailscaleStatus>('get_tailscale_status');
      publish({ status: result, loading: false, error: null });
    } catch (err) {
      // Keep the last known status so a transient failure doesn't blank the UI.
      publish({
        status: sharedState.status,
        loading: false,
        error: getErrorMessage(err, 'Failed to check Tailscale status'),
      });
    } finally {
      inFlight = null;
    }
  })();
  inFlight = request;
  return request;
}

/** Test-only: clears the shared cache, timer, and subscribers. */
export function resetTailscaleStatusCache(): void {
  sharedState = initialState;
  subscribers.clear();
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
  inFlight = null;
}

/**
 * Polls `get_tailscale_status` on mount and every 30s thereafter. The poll and
 * its result are shared by every consumer of this hook.
 */
export function useTailscaleStatus(): UseTailscaleStatusResult {
  const [state, setState] = useState<TailscaleStatusState>(sharedState);

  useEffect(() => {
    const notify = (next: TailscaleStatusState) => setState(next);
    subscribers.add(notify);
    // Adopt whatever another subscriber has already fetched.
    setState(sharedState);
    if (!pollTimer) {
      pollTimer = setInterval(() => {
        void fetchSharedStatus();
      }, POLL_INTERVAL_MS);
    }
    void fetchSharedStatus();

    return () => {
      subscribers.delete(notify);
      if (subscribers.size === 0 && pollTimer) {
        clearInterval(pollTimer);
        pollTimer = null;
      }
    };
  }, []);

  return { ...state, refresh: fetchSharedStatus };
}
