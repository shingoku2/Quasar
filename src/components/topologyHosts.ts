import type { ScanResult } from './NetworkScanner';

/**
 * Builds a lookup from a scan result to the saved host it belongs to. A saved address matches
 * the result's IP or its hostname, case-insensitively. The scanner resolves hostnames only for
 * hosts that answer, so a hostname-addressed saved host matches an offline result only when
 * `carryKnownHostnames` kept the hostname from an earlier sighting.
 */
export function savedHostLookup<T extends { address: string }>(
  savedHosts: readonly T[],
): (host: ScanResult) => T | undefined {
  const byAddress = new Map<string, T>();
  savedHosts.forEach(saved => byAddress.set(saved.address.trim().toLowerCase(), saved));
  return host =>
    byAddress.get(host.ip.toLowerCase()) ??
    (host.hostname ? byAddress.get(host.hostname.toLowerCase()) : undefined);
}

/**
 * Returns `results` with each hostname-less offline result given the hostname last seen for
 * its IP in `previous` (the persisted discovered hosts, or an earlier scan), so a saved host
 * addressed by hostname stays on the topology while it is down.
 */
export function carryKnownHostnames(previous: readonly ScanResult[], results: readonly ScanResult[]): ScanResult[] {
  const known = new Map<string, string>();
  previous.forEach(host => {
    if (host.hostname) known.set(host.ip, host.hostname);
  });
  return results.map(result => {
    const hostname = known.get(result.ip);
    return result.is_alive || result.hostname || !hostname ? result : { ...result, hostname };
  });
}
