# ADR 0031: arithmetic-preserving wide raw-moment scans

**Status:** Accepted for local development
**Date:** 2026-09-09
**Decision owner:** Johannes Schmieder

## Context

After the guarded wide finite solve in ADR 0030, raw-moment construction is
the largest measured phase in the one-thread, one-FE K65 fit. The remaining
wide path scans a row-major matrix twice for each column, repeatedly loading
the same cache lines and weights. Johannes requested the next promising
optimization from tracker #26.

## Decision

For raw matrices wider than 64 columns, scan blocks of eight adjacent columns
when there are at least as many blocks as native workers. Each block owns its
output columns and uses fixed-size stack accumulators. It first computes raw
maxima and weighted scaled norms, then compensated weighted means. The
arithmetic and observation order within each column are identical to the
prior columnwise path, including normalization and norm finalization.

This amends the wide raw-cache implementation in ADR 0012 decision 8. The
at-most-64 row-partial algorithm and the columnwise fallback for insufficient
parallel blocks remain unchanged. Raw width includes the response and all
generated columns; it is distinct from the finite-candidate width in ADR 0030.

There is no new heap allocation, row reduction, matrix transpose, or persistent
cache. The five eight-element accumulator arrays occupy 320 bytes per active
block, independent of sample size and model width. Existing memory admission,
weight semantics, finite rank, solver selection, independent certificates,
failure atomicity and public output contracts remain unchanged.

## Acceptance and limits

Require exact scientific baseline/candidate comparisons, not the numerical
exceptions used for ADR 0030. Focused tests compare scalar and blocked bits
across weights, extreme scales, cancellation, zero columns, block tails and
thread dispatch, and exercise nonfinite and overflow rejection. Complete
command comparisons use the existing development harness and pinned reference
records; phase traces are collected separately from the untraced timing pairs.

The source-bound local development receipt is
`benchmarks/reports/development/ols-raw-moments-local-2026-09-09.json`.
This decision does not claim frozen-matrix, RSS, SCC, Windows or release
qualification. It does not implement streaming FWL, change the one-shot
estimation boundary, or alter the remaining tracker milestones.
