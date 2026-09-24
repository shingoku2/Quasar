import clsx, { type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function getErrorMessage(err: unknown, fallback: string = 'An unknown error occurred'): string {
  if (err instanceof Error) return err.message;
  if (typeof err === 'string') return err;
  if (err && typeof err === 'object' && 'message' in err && typeof (err as { message: unknown }).message === 'string') {
    return (err as { message: string }).message;
  }
  return fallback;
}

/**
 * True when the backend reports that the user declined a native confirmation dialog
 * (`native_confirm::DECLINED`). Callers treat it as a no-op, not an error.
 */
export function isUserCancelled(err: unknown): boolean {
  return getErrorMessage(err, '') === 'Cancelled';
}
