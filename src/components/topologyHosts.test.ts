import { describe, it, expect } from 'vitest';
import { carryKnownHostnames, savedHostLookup } from './topologyHosts';
import type { ScanResult } from './NetworkScanner';

function result(overrides: Partial<ScanResult>): ScanResult {
  return { ip: '10.0.0.1', is_alive: true, open_ports: [], device_type: 'unknown', services: [], last_seen: 0, ...overrides };
}

describe('savedHostLookup', () => {
  const saved = [
    { id: 'a', address: '10.0.0.5' },
    { id: 'b', address: ' Pi.LAN ' },
  ];

  it('matches by IP, or by hostname ignoring case and surrounding spaces', () => {
    const find = savedHostLookup(saved);
    expect(find(result({ ip: '10.0.0.5' }))?.id).toBe('a');
    expect(find(result({ ip: '10.0.0.7', hostname: 'pi.lan' }))?.id).toBe('b');
    expect(find(result({ ip: '10.0.0.8' }))).toBeUndefined();
  });
});

describe('carryKnownHostnames', () => {
  it('gives an offline result without a hostname the one last seen for its IP', () => {
    const previous = [result({ ip: '10.0.0.7', hostname: 'pi.lan' })];
    const [carried] = carryKnownHostnames(previous, [result({ ip: '10.0.0.7', is_alive: false })]);
    expect(carried.hostname).toBe('pi.lan');
    expect(carried.is_alive).toBe(false);
  });

  it('leaves live results, results with their own hostname, and unknown IPs alone', () => {
    const previous = [
      result({ ip: '10.0.0.1', hostname: 'old-live' }),
      result({ ip: '10.0.0.2', hostname: 'old-name' }),
    ];
    const fresh = [
      result({ ip: '10.0.0.1', is_alive: true }),
      result({ ip: '10.0.0.2', is_alive: false, hostname: 'new-name' }),
      result({ ip: '10.0.0.3', is_alive: false }),
    ];
    const out = carryKnownHostnames(previous, fresh);
    expect(out[0].hostname).toBeUndefined();
    expect(out[1].hostname).toBe('new-name');
    expect(out[2].hostname).toBeUndefined();
  });
});
