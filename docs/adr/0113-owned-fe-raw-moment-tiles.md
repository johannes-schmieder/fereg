# ADR 0113: Fresh owned-FE raw-moment tiles

Status: accepted for local development, 2026-09-22.

## Decision

Batch the generic owned-FE certificate's raw diagnostic moments over at most
32 RHS columns. Within each level and tile, compute the weighted residual
scales once, then accumulate each channel's moments through contiguous rows.
Reuse a row's weighted basis value across the independent RHS entries.

Preserve each entry's observation order, separately scaled operands, two
divisions, compensated addition, zero handling, final multiplication order
and saturation to signed infinity. The maximum is over nonnegative absolute
moments; changing the traversal of independent coordinates cannot change it.
Retain the old scalar implementation under tests as a bitwise reference.

The fresh weighted normalized columns, pivoted reorthogonalized QR, relative
span certificate, reference norms and all stopping rules remain unchanged.
The raw diagnostic does not replace the relative certificate. There is no
new solver route or tolerance, rank, sample, DoF, weight or publication policy.

Three fixed arrays add 768 bytes of stack payload. There is no added heap
allocation, admission change, worker team, retained topology or geometry cache.
Each call reads the supplied weights and residuals afresh. If a PPML path uses
this generic certificate, nothing survives the call into another weight frame
or generation. This is not permission to cache OLS geometry inside PPML.

## Evidence and scope

The [bounded breadth report](../../benchmarks/reports/development/owned-fe-certificate-local-2026-09-22/README.md)
establishes the gap before changing the kernel. Two multi-partition slope
commands initially cost 421/521 ms versus stock fixest's 55/75 ms. Generic
certification dominates their owned-FE absorption. Separate diagnostic builds
reduce initial-solve raw moments from 120→55 ms and 176→76 ms, while weighted
normalization, QR and span projection remain essentially unchanged.

Six eligible slope rows improve 10.6–19.8% by paired medians, including wider,
unbalanced, disconnected, analytic-weight robust and probability-weight
clustered cases. Time-series and categorical/PPML controls have small mixed
changes, including losses. There is no general speed claim for those controls.

All 42 successful native pairs and 56 repeat comparisons pass unchanged
scientific envelopes. Thirty-nine native pairs and six saved-output binaries
are exact. Tiny variation in the three parallel disconnected pairs is also
present in repeated baseline fits. Five bounded independent designs, 32
distinct Rust tests, two optimized tile tests, actual-plugin ABI containment,
four harness boundary tests and four licensed fixtures pass.

Stock disconnected coefficients, covariance and RSS disagree and remain
unranked. The weak design has native warmup failures and stock nonconvergence;
its repeated comparison was bounded and stopped. No rank adjudication or
repair is claimed. Static allocation source drift, constrained-fit/CG failures
and platform deferrals remain open. This is local development, not qualification.
