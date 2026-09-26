/** How long a copied secret may stay on the clipboard. */
export const COPIED_SECRET_TIMEOUT_MS = 30_000;

// The one secret this app last put on the clipboard. Module-level, so the wipe on vault
// lock (VaultProvider) still works after the dialog that copied it has closed.
let pending: { secret: string; timer: ReturnType<typeof setTimeout> } | null = null;
// Bumped by every flush and copy, so a copy whose clipboard write was still in flight
// when the vault locked can tell and wipe itself instead of scheduling a timer.
let generation = 0;

/**
 * Wipes the clipboard, but only if it still holds `secret`: text the user copied since is
 * left alone. If the clipboard can't be read, it is wiped anyway (a leaked password is
 * worse than a lost clipboard).
 */
async function clearClipboardIfUnchanged(secret: string): Promise<void> {
  try {
    const current = await navigator.clipboard.readText();
    if (current !== secret) return;
  } catch {
    // Unreadable: fall through and clear.
  }
  try {
    await navigator.clipboard.writeText('');
  } catch {
    // No clipboard to clear.
  }
}

/** Wipes the last copied secret from the clipboard now, if one is pending. */
export function flushCopiedSecret(): void {
  generation++;
  if (!pending) return;
  clearTimeout(pending.timer);
  const { secret } = pending;
  pending = null;
  void clearClipboardIfUnchanged(secret);
}

/** Copies `secret` to the clipboard and schedules its wipe. Throws if the write fails. */
export async function copySecret(secret: string): Promise<void> {
  const gen = ++generation;
  await navigator.clipboard.writeText(secret);
  if (gen !== generation) {
    // Flushed (vault locked) or superseded while the write was in flight.
    void clearClipboardIfUnchanged(secret);
    return;
  }
  if (pending) clearTimeout(pending.timer);
  pending = { secret, timer: setTimeout(flushCopiedSecret, COPIED_SECRET_TIMEOUT_MS) };
}
