## 2023-10-27 - Sidebar Keyboard Navigation and ARIA Labels
**Learning:** Icon-only navigation buttons in the sidebar lacked proper screen reader labels and clear keyboard focus states, making the primary navigation of the app difficult for some users.
**Action:** Always add explicit `aria-label`s to interactive elements without text, and use Tailwind's `focus-visible` utility classes (like `focus-visible:ring-2`) to ensure that keyboard users have a visual indicator of their current position without disrupting mouse users.
