## 2026-09-17 - React.memo Optimization for Sidebar Component
**Learning:** The Sidebar component is complex but receives simple props (`activeView` and `onViewChange`). However, it re-rendered frequently due to updates in the parent `Layout` component holding all view states.
**Action:** Applied `React.memo()` to the `Sidebar` to prevent unnecessary re-renders when other unassociated states change in the root layout. This is a common pattern for top-level navigation components in React where props are often unchanged despite extensive state changes elsewhere.

## 2026-02-01 - React Array Filtering Optimization
**Learning:** In highly dynamic components rendering lists (like `HostList`, `QuickConnectWidget`, and `Discovery`), computing array `.filter()` operations inline inside the component body can cause significant performance degradation. Specifically, string operations like `.toLowerCase()` inside the filter loop are expensive and redundant since the filter string doesn't change during the iteration. In benchmarks, extracting the `filter.toLowerCase()` operation outside the loop and memoizing the result with `useMemo` reduced array filtering time by roughly 50%.
**Action:** Next time you encounter lists being filtered or transformed in render bodies, always look to wrap them in `useMemo` and extract any loop-invariant computations out of the callback.

## 2025-05-18 - MonitoringView Event-Driven Re-renders
**Learning:** Components that listen to the high-frequency `system-metrics` event (emitted every 5s) will re-render their entire tree, causing expensive child components (like `RemoteHostsList`) to needlessly re-render 12 times a minute even if their specific data only updates every 30s.
**Action:** Extract unrelated, complex UI blocks from such high-frequency event listeners into separate `React.memo` components, ensuring their props (including callbacks via `useCallback`) remain referentially stable to prevent crossfire re-renders.
