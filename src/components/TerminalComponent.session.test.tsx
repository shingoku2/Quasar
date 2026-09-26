import { render, screen, act, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { listen, type EventCallback } from '@tauri-apps/api/event';
import TerminalComponent, { TERMINAL_THEMES } from './TerminalComponent';
import '@testing-library/jest-dom';

// A fake xterm that records what the component does to it.
interface FakeTerm {
  options: Record<string, unknown>;
  rows: number;
  cols: number;
  written: string[];
  onDataHandler: ((d: string) => void) | null;
  onDataDispose: ReturnType<typeof vi.fn>;
  dispose: ReturnType<typeof vi.fn>;
  refresh: ReturnType<typeof vi.fn>;
}
const terms: FakeTerm[] = [];
vi.mock('@xterm/xterm', () => ({
  Terminal: class {
    options: Record<string, unknown>;
    rows = 24;
    cols = 80;
    written: string[] = [];
    onDataHandler: ((d: string) => void) | null = null;
    onDataDispose = vi.fn();
    dispose = vi.fn();
    refresh = vi.fn();
    open = vi.fn();
    loadAddon = vi.fn();
    constructor(opts: Record<string, unknown>) {
      this.options = { ...opts };
      terms.push(this as unknown as FakeTerm);
    }
    write(s: string) {
      this.written.push(s);
    }
    onData(h: (d: string) => void) {
      this.onDataHandler = h;
      return { dispose: this.onDataDispose };
    }
  },
}));

const fit = vi.fn();
vi.mock('@xterm/addon-fit', () => ({
  FitAddon: class {
    fit = fit;
  },
}));

let resizeCallback: (() => void) | null = null;
const disconnect = vi.fn();
class FakeResizeObserver {
  constructor(cb: () => void) {
    resizeCallback = cb;
  }
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = disconnect;
}

// Event listeners registered through `listen`, by event name.
const listeners = new Map<string, EventCallback<unknown>>();
const emitTo = (name: string, payload: unknown) => listeners.get(name)?.({ event: name, id: 0, payload });
const unlisteners: ReturnType<typeof vi.fn>[] = [];

const props = { sessionId: 's1', host: 'box.lan', port: 2200, username: 'ops' };

describe('TerminalComponent session', () => {
  const originalResizeObserver = window.ResizeObserver;
  let rectSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.clearAllMocks();
    terms.length = 0;
    listeners.clear();
    unlisteners.length = 0;
    resizeCallback = null;
    window.ResizeObserver = FakeResizeObserver as unknown as typeof ResizeObserver;
    // jsdom reports a zero-size box; the terminal only initialises once it has a size.
    rectSpy = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({ width: 800, height: 400 } as DOMRect);
    vi.mocked(listen).mockImplementation(async (name, cb) => {
      listeners.set(String(name), cb);
      const un = vi.fn();
      unlisteners.push(un);
      return un;
    });
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  afterEach(() => {
    window.ResizeObserver = originalResizeObserver;
    rectSpy.mockRestore();
    vi.useRealTimers();
  });

  const written = () => terms[0]?.written.join('') ?? '';

  it('does not start a session while the container has no size', async () => {
    rectSpy.mockReturnValue({ width: 0, height: 0 } as DOMRect);
    render(<TerminalComponent {...props} />);
    await act(async () => {});
    expect(terms).toHaveLength(0);
    expect(invoke).not.toHaveBeenCalledWith('connect_ssh', expect.anything());
  });

  it('connects with lowerCamelCase args and a credential id, then syncs its size', async () => {
    render(<TerminalComponent {...props} credentialId="cred-1" password="" />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('resize_ssh', { id: 's1', rows: 24, cols: 80 }));
    expect(invoke).toHaveBeenCalledWith('connect_ssh', {
      id: 's1',
      host: 'box.lan',
      port: 2200,
      user: 'ops',
      password: undefined,
      credentialId: 'cred-1',
    });
    expect(written()).toContain('Connecting to box.lan as ops...');
    expect(written()).toContain('Session Established.');
    expect(screen.getByRole('application', { name: 'SSH terminal — ops@box.lan' })).toBeInTheDocument();
  });

  it('defaults to port 22 and passes a typed password through', async () => {
    render(<TerminalComponent sessionId="s2" host="h" username="u" password="pw" />);
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('connect_ssh', expect.objectContaining({ port: 22, password: 'pw', credentialId: undefined }))
    );
  });

  it('listens on the session-scoped event channels before connecting', async () => {
    render(<TerminalComponent {...props} />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('connect_ssh', expect.anything()));
    expect([...listeners.keys()]).toEqual(['ssh_data_s1', 'ssh_closed_s1', 'ssh_stats_s1', 'ssh_timeout_s1']);
  });

  it('writes incoming data, close and timeout notices to the terminal', async () => {
    render(<TerminalComponent {...props} />);
    await waitFor(() => expect(listeners.size).toBe(4));
    act(() => {
      emitTo('ssh_data_s1', 'hello\r\n');
      emitTo('ssh_closed_s1', null);
      emitTo('ssh_timeout_s1', null);
    });
    expect(written()).toContain('hello\r\n');
    expect(written()).toContain('Connection closed.');
    expect(written()).toContain('Session timed out due to inactivity.');
  });

  it('shows latency and bandwidth from the stats channel', async () => {
    render(<TerminalComponent {...props} />);
    await waitFor(() => expect(listeners.has('ssh_stats_s1')).toBe(true));
    act(() => { emitTo('ssh_stats_s1', { bandwidth: '1.2 KB/s', latency: 42 }); });
    expect(await screen.findByText(/42/)).toBeInTheDocument();
    expect(screen.getByText(/1\.2 KB\/s/)).toBeInTheDocument();
  });

  it('prints a connection error instead of claiming success', async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === 'connect_ssh') throw 'Authentication failed';
      return undefined;
    });
    render(<TerminalComponent {...props} />);
    await waitFor(() => expect(written()).toContain('Connection Error: Authentication failed'));
    expect(written()).not.toContain('Session Established.');
  });

  it('forwards keystrokes and hides "Session not found" write errors', async () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    try {
      render(<TerminalComponent {...props} />);
      await waitFor(() => expect(terms[0]?.onDataHandler).not.toBeNull());
      vi.mocked(invoke).mockRejectedValueOnce('Session not found');
      await act(async () => terms[0].onDataHandler?.('ls\r'));
      expect(invoke).toHaveBeenCalledWith('write_ssh', { id: 's1', data: 'ls\r' });
      expect(errorSpy).not.toHaveBeenCalled();

      vi.mocked(invoke).mockRejectedValueOnce('broken pipe');
      await act(async () => terms[0].onDataHandler?.('x'));
      expect(errorSpy).toHaveBeenCalledWith('Write error:', 'broken pipe');
    } finally {
      errorSpy.mockRestore();
    }
  });

  it('refits and resizes the remote pty when the container resizes', async () => {
    render(<TerminalComponent {...props} />);
    await waitFor(() => expect(resizeCallback).not.toBeNull());
    vi.mocked(invoke).mockClear();
    terms[0].rows = 40;
    terms[0].cols = 120;
    act(() => resizeCallback?.());
    expect(fit).toHaveBeenCalled();
    expect(invoke).toHaveBeenCalledWith('resize_ssh', { id: 's1', rows: 40, cols: 120 });
  });

  it('skips the remote resize while the terminal has no rows', async () => {
    render(<TerminalComponent {...props} />);
    await waitFor(() => expect(resizeCallback).not.toBeNull());
    vi.mocked(invoke).mockClear();
    terms[0].rows = 0;
    act(() => resizeCallback?.());
    expect(invoke).not.toHaveBeenCalledWith('resize_ssh', expect.anything());
  });

  it('unmounting disconnects, unlistens and disposes everything', async () => {
    const { unmount } = render(<TerminalComponent {...props} />);
    await waitFor(() => expect(unlisteners).toHaveLength(4));
    unmount();
    expect(invoke).toHaveBeenCalledWith('disconnect_ssh', { id: 's1' });
    unlisteners.forEach((u) => expect(u).toHaveBeenCalled());
    expect(disconnect).toHaveBeenCalled();
    expect(terms[0].onDataDispose).toHaveBeenCalled();
    expect(terms[0].dispose).toHaveBeenCalled();
  });

  it('changing the theme or font updates the terminal in place without reconnecting', async () => {
    const { rerender } = render(<TerminalComponent {...props} />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('connect_ssh', expect.anything()));
    rerender(<TerminalComponent {...props} theme="nord" fontFamily="Fira Code" fontSize={16} />);
    expect(terms).toHaveLength(1);
    expect(terms[0].options.theme).toEqual(TERMINAL_THEMES.nord);
    expect(terms[0].options.fontFamily).toBe('Fira Code');
    expect(terms[0].options.fontSize).toBe(16);
    expect(terms[0].refresh).toHaveBeenCalledWith(0, 23);
    expect(invoke).not.toHaveBeenCalledWith('disconnect_ssh', expect.anything());
    expect(vi.mocked(invoke).mock.calls.filter((c) => c[0] === 'connect_ssh')).toHaveLength(1);
  });

  it('the initial theme and font come from props', async () => {
    render(<TerminalComponent {...props} theme="solarizedLight" fontSize={12} />);
    await waitFor(() => expect(terms).toHaveLength(1));
    expect(terms[0].options.theme).toEqual(TERMINAL_THEMES.solarizedLight);
    expect(terms[0].options.fontSize).toBe(12);
    expect(terms[0].options.fontFamily).toBe('monospace');
  });

  it('fits after the initial delay and reports a fit failure without crashing', async () => {
    vi.useFakeTimers();
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    try {
      fit.mockImplementationOnce(() => {
        throw new Error('no renderer');
      });
      render(<TerminalComponent {...props} />);
      await act(async () => { await vi.advanceTimersByTimeAsync(200); });
      expect(errorSpy).toHaveBeenCalledWith('Fit error:', expect.any(Error));
      expect(written()).not.toContain('Dimensions set.');
    } finally {
      errorSpy.mockRestore();
    }
  });
});
