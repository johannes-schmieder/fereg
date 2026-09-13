# ADR 0060: joint admission for prepared direct CG

**Status:** Accepted for local development
**Date:** 2026-09-13
**Decision owner:** Johannes Schmieder

## Context

The prepared simple and categorical direct-CG adapters accepted `AbsorbConfig`
but passed only its numerical MAP configuration into CG. An explicit CG request
or a preselected automatic CG route could therefore bypass the supplied joint
execution/solver budget. The automatic LSMR/CG planner already checks that budget;
its guard does not cover every direct adapter call.

## Decision

Before direct prepared intercept CG allocates its workspace, apply the existing
CG estimate to the actual observation/RHS axes, finalized FE level counts and
installed Rayon team. Add the existing five coefficient-batch allowance when
the categorical adapter retains coefficients. Reject a joint execution-plus-CG
estimate that does not fit, using the existing allocation error and transaction.

Reuse the checked solver estimator. Planning adds only fallibly reserved FE-level
metadata; it introduces no observation-sized state or SPI call. Do not change
projection arithmetic, reference scales, stopping, rank, weights, inference or
certificates. An absent budget retains the existing behavior. Empty topology and
absorbed-slope adapters remain outside this intercept-specific guard.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/prepared-cg-admission-local-2026-09-13/README.md)
records a failing pre-fix closed-form regression, 65 passing isolated prepared-fit
tests, 48 saved-output dummy-oracle fits, weighted direct-CG dummy checks and the
public exact/one-byte-short admission boundary with rollback. The installed team
differs from the default solver configuration in the four-worker tests.

Eight local paired command checks preserve exact scientific exports and solver
decisions. None crosses the prospective 5% and 10 ms review envelope; the largest
absolute increase is 18 ms/0.5%. Explicit-CG pairs cover the changed adapters.
There is no speedup or general memory-saving claim. Both architecture builds and
native arm64 ABI checks pass; no broader qualification was run.

Public OLS selected-memory reporting still excludes solver increments. This
repair does not finish generic/slope/fallback accounting or final-recovery
admission, and does not adopt the separate saved-Schur prototype. Earlier timing
flags and memory-model gaps remain evidence.
