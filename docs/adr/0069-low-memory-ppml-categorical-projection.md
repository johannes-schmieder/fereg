# ADR 0069: Bounded categorical PPML projection in low memory

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-14

German PPML low memory spent more than four minutes on 75,350 input rows in
generic weighted FE projection. The existing categorical engine already handles
the same positive changing-weight operator with independent certificates.

Allow the PPML-only categorical projector in low memory for one to three
intercept effects without retained individual coefficients. Retain the design
on its existing scratch artifact and project pool-width blocks and the response
separately. Dense response/design fusion remains exclusive to fast storage.
Cap projector RHS capacity by generated model width before reservation, matching
the admitted pool. Charge the actual group/dot and level-PCG or observation-space
recurrence capacities in low preflight, including simultaneous scratch buffers.

This extends ADR 0039's earlier fast-only implementation; it changes no model,
weight, sample/separation, rank/reference-scale, stopping, certification, public
solver-family or transactional policy. Current-weight guards and certified
fallbacks belong to the existing engine. Generic OLS/slope routes retain their
projector, and workers operate only on owned/borrowed Rust data.

The [source-bound report](../../benchmarks/reports/development/german-ppml-low-map-local-2026-09-14/README.md)
records independent oracles, scratch/capacity and memory-boundary checks, weak
weighted graphs, German masks/results, phase/resource costs, the incomplete
baseline and remaining regressions. The 75,350-row case completes in 9.824 s;
full German PPML remains untested. No qualification claim is made.
