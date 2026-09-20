# ADR 0083: compact weighted OLS edge scratch

**Status:** Accepted for local development
**Date:** 2026-09-20
**Decision owner:** Johannes Schmieder

## Decision

An eligible weighted OLS right-edge cache may be compacted and transposed
within its existing code and f64 weight banks, allowing both Schur gathers
to use the same normalized edge sums. With E unique edges, code capacity must
cover max(N,2E)+(L+1)+(R+1) words and weight capacity max(N,2E). Capacity
rejection preserves the one-sided cache. Offsets lie past both the original
N-slot representation and the two disjoint compact orientations. Codes and
offsets are private bits; edge weights retain full f64 precision and their
original row-order sums. Each compact adjacency has canonical neighbor order.

Retain ADR 0080's bounded repeated-edge, capped OLS, worker, u32 observation,
restoration and explicit caller-authority guards. Shared/PPML callers opt out.
There is no new heap owner, workspace allowance, tolerance, weight semantics,
solver admission or fallback policy. Both gathers use the same mathematical
weighted operator; changed summation order still requires fresh independent
observation-space certification.

## Evidence boundary

The [local report](../../benchmarks/reports/development/two-fe-weighted-both-cache-local-2026-09-20/README.md)
retains 100 successful application calls, 41 native numerical pairs without a
peak increase, sixteen independent output audits, thirteen Schur tests and
three final-artifact Stata fixtures. Tests independently validate both weighted
adjacencies, capacity rejection, continued one-sided operation, common scales,
connected/disconnected MAP projections, 8/16 workers and charged continuation.
PW/FW, weight-distribution, tiny-bridge, unit, thread, memory and one-FE controls
pass. Nine default-reference output disagreements remain explicit.

Three-pair shuffled AW K=6 IID improves 336 to 316 ms versus fixest's 298 ms;
ordered K=50 improves 770 to 747 ms. No cell has a median paired native
regression above 5%. Remaining statistics and probe costs still matter, and
the broad repeated scoreboard remains open.
