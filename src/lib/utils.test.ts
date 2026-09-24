import { describe, it, expect } from 'vitest';
import { cn, getErrorMessage, isUserCancelled } from './utils';

describe('utils', () => {
  describe('cn', () => {
    it('should merge basic class names', () => {
      expect(cn('class1', 'class2')).toBe('class1 class2');
    });

    it('should handle conditional classes', () => {
      expect(cn('class1', { class2: true, class3: false })).toBe('class1 class2');
    });

    it('should resolve tailwind conflicts using twMerge', () => {
      expect(cn('px-2 py-1', 'p-4')).toBe('p-4');
      expect(cn('bg-red-500', 'bg-blue-500')).toBe('bg-blue-500');
    });

    it('should ignore falsy values', () => {
      expect(cn('class1', null, undefined, false, 0, '', 'class2')).toBe('class1 class2');
    });

    it('should handle arrays of classes', () => {
      expect(cn(['class1', 'class2'], 'class3')).toBe('class1 class2 class3');
    });
  });

  describe('getErrorMessage', () => {
    it('should return the message from an Error instance', () => {
      const err = new Error('This is an error');
      expect(getErrorMessage(err)).toBe('This is an error');
    });

    it('should return the string if the error is a string', () => {
      expect(getErrorMessage('String error')).toBe('String error');
    });

    it('should return the message property if the error is an object with a string message', () => {
      expect(getErrorMessage({ message: 'Object error' })).toBe('Object error');
    });

    it('should use the default fallback for objects without a string message property', () => {
      expect(getErrorMessage({ message: 123 })).toBe('An unknown error occurred');
      expect(getErrorMessage({ someOtherProp: 'value' })).toBe('An unknown error occurred');
      expect(getErrorMessage({})).toBe('An unknown error occurred');
    });

    it('should use the default fallback for null or undefined', () => {
      expect(getErrorMessage(null)).toBe('An unknown error occurred');
      expect(getErrorMessage(undefined)).toBe('An unknown error occurred');
    });

    it('should use a custom fallback if provided', () => {
      expect(getErrorMessage(null, 'Custom fallback')).toBe('Custom fallback');
      expect(getErrorMessage({ message: 123 }, 'Custom fallback')).toBe('Custom fallback');
      expect(getErrorMessage(123, 'Custom fallback')).toBe('Custom fallback');
    });

    it('should use the default fallback for other types like numbers or booleans', () => {
      expect(getErrorMessage(123)).toBe('An unknown error occurred');
      expect(getErrorMessage(true)).toBe('An unknown error occurred');
    });
  });

  describe('isUserCancelled', () => {
    it('matches only the backend native-dialog decline', () => {
      expect(isUserCancelled('Cancelled')).toBe(true);
      expect(isUserCancelled(new Error('Cancelled'))).toBe(true);
      expect(isUserCancelled('Cancelled by server')).toBe(false);
      expect(isUserCancelled('Failed to remove host key')).toBe(false);
      expect(isUserCancelled(null)).toBe(false);
    });
  });
});
