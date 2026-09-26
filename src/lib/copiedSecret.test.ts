import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { COPIED_SECRET_TIMEOUT_MS, copySecret, flushCopiedSecret } from './copiedSecret';

describe('copiedSecret (FE-009)', () => {
  const originalClipboard = navigator.clipboard;
  let clipboard: string;
  const writeText = vi.fn(async (t: string) => { clipboard = t; });
  const readText = vi.fn(async () => clipboard);
  const settle = () => vi.advanceTimersByTimeAsync(0);

  beforeEach(() => {
    vi.useFakeTimers();
    clipboard = '';
    readText.mockClear();
    Object.defineProperty(navigator, 'clipboard', { value: { writeText, readText }, configurable: true });
  });
  afterEach(() => {
    flushCopiedSecret();
    vi.useRealTimers();
    Object.defineProperty(navigator, 'clipboard', { value: originalClipboard, configurable: true });
  });

  it('wipes a copied secret after the timeout', async () => {
    await copySecret('secret123');
    expect(clipboard).toBe('secret123');
    await vi.advanceTimersByTimeAsync(COPIED_SECRET_TIMEOUT_MS);
    expect(clipboard).toBe('');
  });

  it('leaves the clipboard alone if the user copied something else since', async () => {
    await copySecret('secret123');
    clipboard = 'something else';
    await vi.advanceTimersByTimeAsync(COPIED_SECRET_TIMEOUT_MS);
    expect(clipboard).toBe('something else');
  });

  it('wipes at once when flushed (vault lock)', async () => {
    await copySecret('secret123');
    flushCopiedSecret();
    await settle();
    expect(clipboard).toBe('');
  });

  it('wipes the clipboard when it cannot be read', async () => {
    readText.mockRejectedValueOnce(new Error('denied'));
    await copySecret('secret123');
    flushCopiedSecret();
    await settle();
    expect(clipboard).toBe('');
  });

  // Review: a lock during the clipboard write must not leave the password behind.
  it('wipes a copy whose write finishes after the vault locked', async () => {
    let finishWrite: () => void = () => {};
    writeText.mockImplementationOnce(async (t: string) => {
      await new Promise<void>((r) => { finishWrite = r; });
      clipboard = t;
    });
    const copying = copySecret('secret123');
    flushCopiedSecret();
    finishWrite();
    await copying;
    await settle();
    expect(clipboard).toBe('');
  });
});
