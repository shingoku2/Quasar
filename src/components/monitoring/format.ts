const timeFormatOptions: Intl.DateTimeFormatOptions = {
  hour12: false,
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
};

/** MB/s with enough precision that sub-MB/s rates don't read as 0 (FE-017). */
export function formatRateMb(mb: number): string {
  return mb >= 10 ? mb.toFixed(0) : mb.toFixed(2);
}

/**
 * Formats a metric sample's timestamp in the current system time zone.
 *
 * Deliberately not cached: a module-level `Intl.DateTimeFormat` pins the time zone
 * it was built in, and there is no cheap way to detect a zone change (an offset
 * check misses switches between zones that currently share an offset, e.g. New
 * York -> Lima in winter). Metrics arrive at most about once a second, so the
 * per-call formatter cost is negligible.
 */
export const formatMetricTime = (date: Date): string =>
  date.toLocaleTimeString('en-US', timeFormatOptions);

export function formatUptime(seconds: number): string {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${mins}m`;
  return `${mins}m`;
}
