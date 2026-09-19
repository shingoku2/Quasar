## 2025-01-22 - Missing ARIA Labels on Icon Buttons
**Learning:** The application has a pattern of icon-only buttons (like the Close X and Trash delete buttons) lacking `aria-label` attributes, forcing screen readers to ignore them or announce them incorrectly, and tests to use brittle DOM selectors.
**Action:** Always verify icon-only buttons include descriptive `aria-label`s and update testing patterns to use `getByRole('button', { name: '...' })` to ensure both accessibility and robust tests.
