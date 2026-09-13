# P3 combined low-memory cluster score experiment

Baseline c37686b96b633db43eb1d7210f5ca3e7f9520e93, installed dist untouched.

Try one original-row score reconstruction feeding every nonempty cluster subset. All compensated group values, corrections, presence flags and subset descriptors must fit in the old N*sizeof(usize) ordering budget. Allocation-free planning before bank admission. No partial bank coexists with a fallback order. Preserve lexicographic group order, cardinality/mask merge order, per-group independent bread, scientific/error/sample policy and exact results. Keep one-way and fast routes as controls.

Prospective target: >=10% paired complete-command improvement on two-/three-way low-memory targets; protected review envelope max(10 ms,5%). Local rotated three-seed comparisons plus a small stratified control set; no SCC or qualification. Preserve rejected/failed attempts.
