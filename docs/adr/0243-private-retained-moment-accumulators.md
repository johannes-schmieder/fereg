# ADR 0243: private narrow retained finite-moment accumulators

Status: accepted for bounded development, 2026-09-28.

## Decision

The at-most-64-column scaled retained dense moment helper accumulates each
partition's RHS and Gram rows on its own worker stack, then copies them to the
existing disjoint output rows once. This removes observation-frequency writes
to output cache lines potentially shared with other workers. Mathematical row ownership is
unchanged; it previously did not imply cache-line ownership.

Selected columns, normalization, standardization, per-entry observation order
and fused multiply-add expressions are identical. There is no new reduction or
merge. Only the existing helper changes: wider, scratch, implicit, unscaled,
layouts assigning one left column per partition, and one-shot prepared moment
routes retain their loops. The existing guard is retained, not fitted to cases or seeds.

The arrays add 33,280 bytes per active partition to the existing 512-byte suffix
cache: 33 KiB total. At most 32 partitions are eligible simultaneously, for
1,081,344 bytes of array payload. They fit inside already admitted 8 MiB worker
stacks, add no heap allocation or retained bank, and disappear before the
partition returns. Heap receipts do not measure stack payload. Physical caps
and complete stack accounting remain separate qualification gaps.

The checked source and kernel boundary tests preserve exact moments against a
separate scalar loop at T1/2/4/16, selected/contiguous columns, extreme finite
weights/scales and the 64-column boundary, with nonfinite selected inputs still
rejected. Every current-weight projection, rank, finite solve, score, objective,
reconstruction and final-Hessian certificate remains binding. No generation,
cache lifetime, sample, inference, output or transactional policy changes.

## Evidence and interpretation

The [attribution report](../../benchmarks/reports/development/ppml-thread-cost-scc-2026-09-28/README.md)
profiles unchanged 14c753ed/d007491a under the canonical command boundary. The
largest sampled T2 Rayon helper contains finite moments, not demonstrated
scheduler overhead. Cache-line contention is a source-supported hypothesis;
no cache-coherence hardware-event attribution is claimed.

The [implementation report](../../benchmarks/reports/development/ppml-private-moments-scc-2026-09-28/README.md)
binds candidate a9fb2307, 90 distinct Rust tests and six independent licensed
fixtures on each platform, plus Linux PPML smoke. The initial/fresh/scaling
rotations complete 222 applications and all 74 native/74 stock comparisons pass.
Fresh candidate/baseline medians are .893 one-FE T2, .874 T4, .842 wide,
.774 larger, .881 three-FE and .948 slopes. Serial improves .923; low improves
.965 after an initial 1.137 loss. Small two-FE and the OLS control lose about 1%
in the fresh block; their full distributions remain visible. The initial T2
whole-command gain was absent despite cheaper finite solving. Retain the small
coherent change because its principal gains repeat across sizes and FE designs.

Direct one-FE scaling now wins all three pairs per transition: T1→T2 .850,
T2→T4 .814 and T1→T4 .788 medians. Two-FE T1→T2 still has one 4.3% loss;
the median is .955. Requested three-FE 8→16 remains effective 8→8 and is not
native 16-worker scaling. No thread cap is introduced or changed.

Fresh stock medians remain 2.769 one-FE T1, 1.627 T2, 1.855 T4 and 3.000 low;
small two-FE 1.533 and OLS 1.625 also miss the ceiling. Wide .913 and larger
.927 are competitive, while three-FE .306 and slopes .350 beat stock clearly.
These are source-/design-/output-specific timings, not transferred historical
claims. Universal 1.20/0.90 and no-core-regression targets remain open.

This updates ADR0054's temporary stack payload, preserving its row-standardization
mechanism and route. Retained/scientific ownership and all memory admission
policies remain unchanged. Frozen definitions, prior rejected experiments,
static allocation inventory drift and unresolved scientific issues are not
altered by this decision.
