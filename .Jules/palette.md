## 2023-10-27 - Sidebar Keyboard Navigation and ARIA Labels
**Learning:** Icon-only navigation buttons in the sidebar lacked proper screen reader labels and clear keyboard focus states, making the primary navigation of the app difficult for some users.
**Action:** Always add explicit `aria-label`s to interactive elements without text, and use Tailwind's `focus-visible` utility classes (like `focus-visible:ring-2`) to ensure that keyboard users have a visual indicator of their current position without disrupting mouse users.

## 2025-01-22 - Missing ARIA Labels on Icon Buttons
**Learning:** The application has a pattern of icon-only buttons (like the Close X and Trash delete buttons) lacking `aria-label` attributes, forcing screen readers to ignore them or announce them incorrectly, and tests to use brittle DOM selectors.
**Action:** Always verify icon-only buttons include descriptive `aria-label`s and update testing patterns to use `getByRole('button', { name: '...' })` to ensure both accessibility and robust tests.
