import { vi } from 'vitest';

// Provide the Tauri internals stub that the real @tauri-apps/api modules
// look for at runtime. This is essential for dynamic imports that bypass
// vi.mock hoisting (e.g. `const { listen } = await import(...)`)
const callbackMap = new Map<number, Function>();
let callbackId = 0;

(window as any).__TAURI_INTERNALS__ = {
  transformCallback: (callback?: Function, _once?: boolean) => {
    const id = callbackId++;
    if (callback) {
      callbackMap.set(id, callback);
    }
    return id;
  },
  invoke: vi.fn(() => Promise.resolve()),
  metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
  convertFileSrc: (src: string) => src,
};

// Required by _unlisten in @tauri-apps/api/event.js
(window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
  unregisterListener: vi.fn(),
};

// jsdom doesn't implement scrollIntoView
window.HTMLElement.prototype.scrollIntoView = vi.fn();

// Global mock for @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
  transformCallback: (window as any).__TAURI_INTERNALS__.transformCallback,
}));

// Global mock for @tauri-apps/api/event
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
  once: vi.fn(() => Promise.resolve(() => {})),
}));

// Global mock for @tauri-apps/plugin-dialog
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(() => Promise.resolve(null)),
  save: vi.fn(() => Promise.resolve(null)),
  message: vi.fn(() => Promise.resolve()),
  ask: vi.fn(() => Promise.resolve(false)),
  confirm: vi.fn(() => Promise.resolve(false)),
}));

// Global mock for @tauri-apps/api/path
vi.mock('@tauri-apps/api/path', () => ({
  appDataDir: vi.fn(() => Promise.resolve('/mock/app/data')),
  appConfigDir: vi.fn(() => Promise.resolve('/mock/app/config')),
}));

// Global mock for @tauri-apps/plugin-sql
vi.mock('@tauri-apps/plugin-sql', () => {
  const mockDb = {
    execute: vi.fn(() => Promise.resolve()),
    select: vi.fn(() => Promise.resolve([])),
    close: vi.fn(() => Promise.resolve()),
  };
  return {
    default: {
      load: vi.fn(() => Promise.resolve(mockDb)),
    },
  };
});
