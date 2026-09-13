# ADR 0047: bounded cluster-boundary searches

Status: accepted for local development, 2026-09-12.

OLS ordered clustered inference and PPML's paired-independent reducer share an
upper-bound search when extending a provisional worker boundary to the end of
its sorted cluster. An already aligned boundary returns after one comparison.
Sorted group equality is a prefix predicate, so the binary search returns the
same boundary as the former adjacent-row scan while avoiding repeated linear
traversals through a giant group.

The existing chunk count, monotone boundaries, empty chunks, within-group row
order, forward/reverse merge, score weights, independent covariance certificate,
multiway corrections and PSD policy remain unchanged. There is no allocation,
new worker team, SPI access, numerical regrouping or broader solver admission.
The giant group's own numerical reduction stays intact; this bounds scheduling
setup without claiming complete load balance.

The [source-bound report](../../benchmarks/reports/development/cluster-boundaries-local-2026-09-12/README.md)
contains twenty pairs against `fdf4624`. OLS command medians fall 0.430→0.401
seconds, with repeatable 27–37 ms savings meeting the prospective criterion.
PPML inference nearly halves, but its total gain is smaller and more variable
(1.1% paired). Primary native/scratch allocations are unchanged and all full
scientific/non-time solver exports are exact. Twenty-five focused tests and both
independent Stata/Mata oracle arms pass; adopted native inputs match the measured
artifact. Universal static/native arm64 ABI checks pass.

An initial two-way PPML slowdown is retained and localized to unchanged solve
phases; three fresh rechecks improve, without establishing the source of timing
variation. Weak OLS retains a pre-existing memory-model discrepancy. This is
bounded local development evidence, not release or cross-platform qualification.
