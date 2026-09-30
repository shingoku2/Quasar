import type { ScanResult } from './NetworkScanner';

/** Protocols with an in-app client, in the order a match prefers them. */
const CLIENT_PROTOCOLS = ['ssh', 'rdp'];

/**
 * Builds a lookup from a scan result to the saved host it belongs to. A saved address matches
 * the result's IP or its hostname, case-insensitively (IP matches win). The scanner resolves
 * hostnames only for hosts that answer, so a hostname-addressed saved host matches an offline
 * result only when `carryKnownHostnames` kept the hostname from an earlier sighting.
 *
 * One machine can be saved several times (the hosts table allows one row per address, protocol
 * and port). Among several matches the lookup returns the first SSH record, then the first RDP
 * one, then the first of the rest, so connecting opens a client whenever one of them has one.
 */
export function savedHostLookup<T extends { address: string; protocol?: string }>(
  savedHosts: readonly T[],
): (host: ScanResult) => T | undefined {
  const byAddress = new Map<string, T[]>();
  savedHosts.forEach(saved => {
    const key = saved.address.trim().toLowerCase();
    byAddress.set(key, [...(byAddress.get(key) ?? []), saved]);
  });
  const preferred = (matches: readonly T[]): T | undefined => {
    for (const protocol of CLIENT_PROTOCOLS) {
      const match = matches.find(saved => saved.protocol?.toLowerCase() === protocol);
      if (match) return match;
    }
    return matches[0];
  };
  return host => {
    const byIp = byAddress.get(host.ip.toLowerCase());
    if (byIp) return preferred(byIp);
    const byName = host.hostname ? byAddress.get(host.hostname.toLowerCase()) : undefined;
    return byName ? preferred(byName) : undefined;
  };
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
