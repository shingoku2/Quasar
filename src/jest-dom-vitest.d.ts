/**
 * jest-dom matchers for vitest 5's `expect` typings. vitest 5 gave `Assertion` two type
 * parameters, so the augmentation @testing-library/jest-dom 7 ships (`Assertion<T>`) no
 * longer merges; vitest 5 exposes `Matchers<R, T>` for custom matchers instead. The
 * runtime matchers are still registered by `import '@testing-library/jest-dom'`.
 * Drop this file once jest-dom ships vitest 5 typings.
 */
import type { TestingLibraryMatchers } from '@testing-library/jest-dom/matchers';

declare module 'vitest' {
  interface Matchers<R = void, T = unknown> extends TestingLibraryMatchers<unknown, R> {}
}
