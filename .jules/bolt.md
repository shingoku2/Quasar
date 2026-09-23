## 2026-09-17 - React.memo Optimization for Sidebar Component
**Learning:** The Sidebar component is complex but receives simple props (`activeView` and `onViewChange`). However, it re-rendered frequently due to updates in the parent `Layout` component holding all view states.
**Action:** Applied `React.memo()` to the `Sidebar` to prevent unnecessary re-renders when other unassociated states change in the root layout. This is a common pattern for top-level navigation components in React where props are often unchanged despite extensive state changes elsewhere.

## 2026-02-01 - React Array Filtering Optimization
**Learning:** In highly dynamic components rendering lists (like `HostList`, `QuickConnectWidget`, and `Discovery`), computing array `.filter()` operations inline inside the component body can cause significant performance degradation. Specifically, string operations like `.toLowerCase()` inside the filter loop are expensive and redundant since the filter string doesn't change during the iteration. In benchmarks, extracting the `filter.toLowerCase()` operation outside the loop and memoizing the result with `useMemo` reduced array filtering time by roughly 50%.
**Action:** Next time you encounter lists being filtered or transformed in render bodies, always look to wrap them in `useMemo` and extract any loop-invariant computations out of the callback.

## 2026-09-23 - React Derived State Performance Pattern
**Learning:** Components filtering arrays (like `AuditLogViewer.tsx`) often synchronize a `filteredLogs` state via `useEffect` depending on a base `logs` array and a `searchQuery`. This is an anti-pattern that causes double renders: the component renders when the query changes, the effect runs and sets the filtered state, and the component renders again.
**Action:** Always prefer `useMemo` over `useEffect` and `useState` for deriving filtered arrays from props or other state. Additionally, hoist any static computations (like `searchQuery.toLowerCase()`) outside the `.filter()` loop to save redundant string allocations on each keypress.
