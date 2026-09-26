import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { listen, type EventCallback } from '@tauri-apps/api/event';
import { subscribeSystemMetrics, type SystemMetrics } from './useSystemMetrics';

const sample = (cpu: number) => ({ cpu_usage_percent: cpu }) as SystemMetrics;

describe('subscribeSystemMetrics (FE-024)', () => {
  let emit: EventCallback<SystemMetrics> = () => {};
  const unlisten = vi.fn();

  beforeEach(() => {
    unlisten.mockClear();
    vi.mocked(listen).mockReset();
    vi.mocked(listen).mockImplementation(async (_event, handler) => {
      emit = handler as EventCallback<SystemMetrics>;
      return unlisten;
    });
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(sample(1));
  });

  const fire = (m: SystemMetrics) => emit({ event: 'system-metrics', id: 0, payload: m });
  const flush = () => new Promise((r) => setTimeout(r, 0));

  it('shares one listener and one initial fetch between subscribers', async () => {
    const a = vi.fn();
    const b = vi.fn();
    const stopA = subscribeSystemMetrics(a);
    await flush();
    const stopB = subscribeSystemMetrics(b);
    expect(listen).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(b).toHaveBeenCalledWith(sample(1)); // the latest sample, at once

    fire(sample(2));
    expect(a).toHaveBeenLastCalledWith(sample(2));
    expect(b).toHaveBeenLastCalledWith(sample(2));

    stopA();
    expect(unlisten).not.toHaveBeenCalled();
    stopB();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('drops a listener that resolves after the last subscriber left', async () => {
    const stop = subscribeSystemMetrics(vi.fn());
    stop();
    await flush();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('keeps a live sample that beats the initial fetch', async () => {
    let finishFetch: (m: SystemMetrics) => void = () => {};
    vi.mocked(invoke).mockReturnValue(new Promise((r) => { finishFetch = r as typeof finishFetch; }));
    const seen = vi.fn();
    const stop = subscribeSystemMetrics(seen);
    await flush();
    fire(sample(5));
    finishFetch(sample(1));
    await flush();
    expect(seen.mock.calls.map(([m]) => m.cpu_usage_percent)).toEqual([5]);
    stop();
  });
});
