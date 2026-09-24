## 2026-09-22 - [O(n²) lookups to O(n) lookups]
**Learning:** Found an inefficient nested loop iteration where for every peer, we were mapping over a list of host arrays, leading to O(n²) complexity. This was rewritten to utilize an already existing O(1) `savedAddresses` Set for O(N) complexity overall.
**Action:** Always look for existing Sets/hash maps that cache data and utilize them for lookups instead of iterating over arrays, especially for functions checking memberships within larger lists.
