import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface ProcessInfo {
  pid: number;
  name: string;
  cpu_usage: number;
  memory_mb: number;
}

export interface DiskInfo {
  name: string;
  mount_point: string;
  total_gb: number;
  used_gb: number;
  free_gb: number;
  usage_percent: number;
}

/** The `system-metrics` event payload and `get_system_metrics` result (`monitoring/collector.rs`). */
export interface SystemMetrics {
  cpu_usage_percent: number;
  memory_used_mb: number;
  memory_total_mb: number;
  memory_usage_percent: number;
  disk_read_mb: number;
  disk_write_mb: number;
  network_rx_mb: number;
  network_tx_mb: number;
  timestamp: number;

  uptime_seconds: number;
  load_average_1m: number;
  load_average_5m: number;
  load_average_15m: number;
  process_count: number;
  boot_time: number;

  cpu_count: number;
  cpu_per_core: number[];
  cpu_frequency_mhz: number;

  disk_total_gb: number;
  disk_used_gb: number;
  disk_free_gb: number;
  disk_usage_percent: number;
  disks?: DiskInfo[];

  network_packets_rx: number;
  network_packets_tx: number;
  network_errors_rx: number;
  network_errors_tx: number;

  top_cpu_processes: ProcessInfo[];
  top_memory_processes: ProcessInfo[];
}

type Subscriber = (metrics: SystemMetrics) => void;

// One `system-metrics` listener and one initial fetch for the whole app, however many
// components show metrics (FE-024). The stream starts with the first subscriber and
// stops with the last.
const subscribers = new Set<Subscriber>();
let latest: SystemMetrics | null = null;
let unlisten: (() => void) | null = null;
let generation = 0;

function publish(metrics: SystemMetrics) {
  latest = metrics;
  subscribers.forEach((notify) => notify(metrics));
}

function start() {
  const gen = ++generation;
  listen<SystemMetrics>('system-metrics', (event) => {
    if (gen === generation) publish(event.payload);
  })
    .then((stop) => {
      // The last subscriber may have left while listen() was pending.
      if (gen === generation) unlisten = stop;
      else stop();
    })
    .catch((err) => console.error('Failed to listen for system metrics:', err));
  invoke<SystemMetrics>('get_system_metrics')
    .then((metrics) => {
      // A live event that arrived first is newer: keep it.
      if (gen === generation && latest === null) publish(metrics);
    })
    .catch((err) => console.warn('Failed to get system metrics:', err));
}

function stop() {
  generation++;
  unlisten?.();
  unlisten = null;
  latest = null;
}

/**
 * Calls `notify` with every metrics sample (and at once with the latest one, if any).
 * Returns the unsubscribe function.
 */
export function subscribeSystemMetrics(notify: Subscriber): () => void {
  subscribers.add(notify);
  if (subscribers.size === 1) start();
  else if (latest) notify(latest);
  return () => {
    subscribers.delete(notify);
    if (subscribers.size === 0) stop();
  };
}

/** The latest system metrics sample, or null before the first one. */
export function useSystemMetrics(): SystemMetrics | null {
  const [metrics, setMetrics] = useState<SystemMetrics | null>(latest);
  useEffect(() => subscribeSystemMetrics(setMetrics), []);
  return metrics;
}
