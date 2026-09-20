# ADR 0080: weighted Schur edge scratch

**Status:** Accepted for local development
**Date:** 2026-09-20
**Decision owner:** Johannes Schmieder

## Decision

Eligible capped OLS Schur attempts may cache repeated right-gather edges using
two existing, disjoint scratch banks. The left RHS means are idle during the
recurrence: temporarily use their first N words for neighbor codes, and the
idle group-statistics bank for normalized f64 edge-weight sums. Both banks
must already fit N, N must fit u32, at least eight workers must be present,
and the bounded repeated-edge sample from ADR 0079 must pass. This adds no
heap owner or memory allowance. Unit weights retain their counted-edge path.

Sort packed code/row words by code then original row, sum each edge's weights
in original row order, and apply the resulting weighted right operator. The
left operator retains its observation loop. Restore the exact original left
RHS means from the immutable observations and weights before reconstruction.
The independent observation-space certificate remains authoritative; scratch
reuse ends before that certificate. Failed capped attempts leave original
observations untouched for the existing charged continuation.

Explicit call-site authority restricts the new cache to OLS. Shared reusable
and PPML checkpoint calls opt out, including capped PPML calls. Weighted
probe admission, tolerances, convergence, weight-kind semantics, fallback,
low-memory policy and forced-solver policy remain unchanged. Aggregating edge
weights changes floating-point summation order, requiring numerical rather
than bitwise parity for affected projections.

## Evidence boundary

The [local report](../../benchmarks/reports/development/two-fe-weighted-edge-cache-local-2026-09-20/README.md)
retains 121 application calls, fifty native pairs without a peak increase,
22 independent output audits, thirteen Schur tests and three licensed Stata
fixtures. Operator tests cover arbitrary fractional weights, extreme common
scales, disconnected graphs, scratch rejection and 8/16 workers. Separate MAP
and forced-continuation checks preserve the original absolute error bound.
An initial test-only tolerance mismatch and all slower controls remain in
the evidence; three-pair confirmations resolve the apparent regressions.

Three-pair shuffled AW K=6 IID improves 372 to 340 ms, still above default
fixest's 299 ms. Ordered K=50 IID improves 858 to 768 ms. This is a bounded
development improvement, not completion of the broader performance milestone.
