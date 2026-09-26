import { useEffect, useRef, useState } from 'react';

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]';

// Each open dialog's opener. A dialog opened from inside another (e.g. the credential
// selector handing off to the manual prompt in one render) inherits that dialog's opener,
// since the control it was opened from is about to disappear.
const openers = new WeakMap<Element, HTMLElement | null>();

function captureOpener(): HTMLElement | null {
  const active = document.activeElement;
  if (!(active instanceof HTMLElement)) return null;
  const hostDialog = active.closest('[role="dialog"]');
  if (hostDialog && openers.has(hostDialog)) return openers.get(hostDialog) ?? null;
  return active;
}

/** The elements Tab actually visits: a negative tabIndex (e.g. a hidden autofill field) is skipped. */
function tabbables(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => el.tabIndex >= 0 && !(el instanceof HTMLInputElement && el.type === 'hidden'),
  );
}

/**
 * Keyboard behaviour for a modal (FE-020). Attach the returned ref to the `role="dialog"`
 * element (give it `tabIndex={-1}`):
 * - focus moves into the dialog when it opens (unless a child already took it with
 *   `autoFocus`) and back to where it was when it closes;
 * - Tab and Shift+Tab stay inside the dialog;
 * - Escape calls `onClose`. Leave `onClose` out for prompts that need an explicit answer
 *   (host-key trust, vault setup).
 */
export function useModalDialog<T extends HTMLElement = HTMLDivElement>(onClose?: () => void) {
  const ref = useRef<T>(null);
  const onCloseRef = useRef(onClose);
  // Captured on the first render: a child's autoFocus moves focus during commit, before any
  // effect runs, so reading it in the effect would record that child instead of the opener.
  const [opener] = useState(captureOpener);

  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return undefined;
    openers.set(dialog, opener);
    if (!dialog.contains(document.activeElement)) {
      (tabbables(dialog)[0] ?? dialog).focus();
    }

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (onCloseRef.current) {
          e.stopPropagation();
          onCloseRef.current();
        }
        return;
      }
      if (e.key !== 'Tab') return;
      const items = tabbables(dialog);
      if (items.length === 0) {
        e.preventDefault();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      if (e.shiftKey && (document.activeElement === first || document.activeElement === dialog)) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    };

    dialog.addEventListener('keydown', onKeyDown);
    return () => {
      dialog.removeEventListener('keydown', onKeyDown);
      if (opener && document.contains(opener)) opener.focus();
    };
  }, [opener]);

  return ref;
}
