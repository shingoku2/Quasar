## 2026-09-17 - React.memo Optimization for Sidebar Component
**Learning:** The Sidebar component is complex but receives simple props (`activeView` and `onViewChange`). However, it re-rendered frequently due to updates in the parent `Layout` component holding all view states.
**Action:** Applied `React.memo()` to the `Sidebar` to prevent unnecessary re-renders when other unassociated states change in the root layout. This is a common pattern for top-level navigation components in React where props are often unchanged despite extensive state changes elsewhere.
