## 2026-09-22 - [O(n²) lookups to O(n) lookups]
**Learning:** Found an inefficient nested loop iteration where for every peer, we were mapping over a list of host arrays, leading to O(n²) complexity. This was rewritten to utilize an already existing O(1) `savedAddresses` Set for O(N) complexity overall.
**Action:** Always look for existing Sets/hash maps that cache data and utilize them for lookups instead of iterating over arrays, especially for functions checking memberships within larger lists.

## 2026-09-23 - [Expensive Date Formatting]
**Learning:** `new Date().toLocaleTimeString(...)` is surprisingly expensive in a high-frequency polling environment because it internally instantiates a new `Intl.DateTimeFormat` on every call. In a frontend that plots incoming system metrics over time, this creates noticeable overhead. Reusing a single `Intl.DateTimeFormat` instance is ~25x faster.
**Action:** Pre-instantiate an `Intl.DateTimeFormat` only for genuinely hot loops (thousands of calls per render). A cached formatter pins the time zone it was created in, so for anything that renders local wall-clock time and runs ~1/sec or slower (like `MonitoringView`'s metric samples), don't cache — correctness across OS time-zone/DST changes beats a few microseconds.

## 2026-09-23 - React Derived State Performance Pattern
**Learning:** Components filtering arrays (like `AuditLogViewer.tsx`) often synchronize a `filteredLogs` state via `useEffect` depending on a base `logs` array and a `searchQuery`. This is an anti-pattern that causes double renders: the component renders when the query changes, the effect runs and sets the filtered state, and the component renders again.
**Action:** Always prefer `useMemo` over `useEffect` and `useState` for deriving filtered arrays from props or other state. Additionally, hoist any static computations (like `searchQuery.toLowerCase()`) outside the `.filter()` loop to save redundant string allocations on each keypress.
