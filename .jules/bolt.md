## 2026-09-22 - [O(n²) lookups to O(n) lookups]
**Learning:** Found an inefficient nested loop iteration where for every peer, we were mapping over a list of host arrays, leading to O(n²) complexity. This was rewritten to utilize an already existing O(1) `savedAddresses` Set for O(N) complexity overall.
**Action:** Always look for existing Sets/hash maps that cache data and utilize them for lookups instead of iterating over arrays, especially for functions checking memberships within larger lists.

## 2026-09-23 - [Expensive Date Formatting]
**Learning:** `new Date().toLocaleTimeString(...)` is surprisingly expensive in a high-frequency polling environment because it internally instantiates a new `Intl.DateTimeFormat` on every call. In a frontend that plots incoming system metrics over time, this creates noticeable overhead. Reusing a single `Intl.DateTimeFormat` instance is ~25x faster.
**Action:** Pre-instantiate an `Intl.DateTimeFormat` only for genuinely hot loops (thousands of calls per render). A cached formatter pins the time zone it was created in, so for anything that renders local wall-clock time and runs ~1/sec or slower (like `MonitoringView`'s metric samples), don't cache — correctness across OS time-zone/DST changes beats a few microseconds.
