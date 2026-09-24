## 2023-10-27 - Sidebar Keyboard Navigation and ARIA Labels
**Learning:** Icon-only navigation buttons in the sidebar lacked proper screen reader labels and clear keyboard focus states, making the primary navigation of the app difficult for some users.
**Action:** Always add explicit `aria-label`s to interactive elements without text, and use Tailwind's `focus-visible` utility classes (like `focus-visible:ring-2`) to ensure that keyboard users have a visual indicator of their current position without disrupting mouse users.

## 2025-01-22 - Missing ARIA Labels on Icon Buttons
**Learning:** The application has a pattern of icon-only buttons (like the Close X and Trash delete buttons) lacking `aria-label` attributes, forcing screen readers to ignore them or announce them incorrectly, and tests to use brittle DOM selectors.
**Action:** Always verify icon-only buttons include descriptive `aria-label`s and update testing patterns to use `getByRole('button', { name: '...' })` to ensure both accessibility and robust tests.

## 2025-02-01 - Hidden Action Buttons and Keyboard Accessibility
**Learning:** Action buttons that are hidden by default using `opacity-0` (and only appear on hover, like `group-hover:opacity-100`) are invisible to keyboard users when they receive focus. This creates a confusing experience where users tab to an invisible element.
**Action:** When using `opacity-0` to hide buttons until hovered, always include `focus-visible:opacity-100` along with explicit focus ring styles (`focus-visible:outline-none focus-visible:ring-2`) to ensure they become visible and clearly highlighted when navigated to via keyboard.

## 2026-09-20 - Improve Global Search UX
**Learning:** Adding type='search' triggers specialized mobile keyboards, and when used in combination with 'aria-label', it creates a much better semantic experience for screen readers compared to just a generic type='text' with a placeholder. Also, explicit aria-hidden='true' on decorative search icons prevents redundant 'Search, Search...' reading.
**Action:** Always prefer type='search' over type='text' for global search inputs.
