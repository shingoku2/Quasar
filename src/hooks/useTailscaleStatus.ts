import { useCallback, useEffect, useRef, useState } from 'react';
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

/** Polls `get_tailscale_status` on mount and every 30s thereafter. */
export function useTailscaleStatus(): UseTailscaleStatusResult {
  const [status, setStatus] = useState<TailscaleStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const cancelledRef = useRef(false);

  const refresh = useCallback(async () => {
    try {
      const result = await invoke<TailscaleStatus>('get_tailscale_status');
      if (cancelledRef.current) return;
      setStatus(result);
      setError(null);
    } catch (err) {
      if (cancelledRef.current) return;
      setError(getErrorMessage(err, 'Failed to check Tailscale status'));
    } finally {
      if (!cancelledRef.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    cancelledRef.current = false;
    refresh();
    const interval = setInterval(refresh, POLL_INTERVAL_MS);
    return () => {
      cancelledRef.current = true;
      clearInterval(interval);
    };
  }, [refresh]);

  return { status, loading, error, refresh };
}
