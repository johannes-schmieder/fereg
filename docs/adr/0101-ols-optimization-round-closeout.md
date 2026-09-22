# ADR 0101: close the current OLS optimization round

**Status:** Local development checkpoint; cross-platform acceptance deferred
**Date:** 2026-09-22
**Decision owner:** Johannes Schmieder

## Decision

Johannes requested finishing this round and moving to other optimization work,
even with remaining serial absorbed-slope losses. Preserve the locally measured
implementation as a committed development checkpoint. Stop further serial-slope
experiments in this round. This narrows the work being completed; it does not
declare the original all-domain performance goal achieved.

Retain the following previously measured changes at their existing guards:

- A capped public automatic two-FE Schur attempt can continue from untouched
  inputs through the admitted LSMR route. Serial saved-effect fits retain its
  coefficient products. Subsequent projections follow the solver actually used;
  a failed full Schur attempt does not authorize another probe reuse.
- The retained serial fallback uses the scoped transport and memory policy in
  ADR 0095. Generic, shared, explicit, narrow and threaded transport retain their
  previous policy. Returned banks and later-window failures remain accounted for.
- Public automatic two-FE CMG can retain stable correction products and reuse
  its same-sample, same-weight graph for final residual correction when the
  combined workspace fits. Fresh observation-space certification remains the
  acceptance test. Batched subtraction retains its N >= 16,384, at least three
  RHS columns and at least eight requested workers guards, and checks the whole
  batch before mutation.
- Existing raw column bounds and direct numeric bindings reduce residual-mean
  passes. Subnormal loss triggers the original maximum-scan fallback. Guarded
  narrow weighted residual overlap retains error ordering; unit-weight residual
  sums are never substituted for weighted sums.

Keep all already committed slope geometry, recovery, loading, score-mean and
covariance improvements. Do not introduce the experimental cycle detector from
ADR 0093, widen guards, change public options or restore rejected experiments.
Do not change samples, ranks, weight semantics, inference, tolerances or fresh
certificates. No plugin installation, push or release follows from this decision.

## Evidence and limits

The [closeout report](../../benchmarks/reports/development/ols-round-closeout-local-2026-09-22/README.md)
binds the complete computational source to the previously measured `dtk1`
snapshot and records focused integration checks on that source. The historical
paired screens, final Mac blocks, control checks and independent oracles retain
their original artifact identities. They are not relabeled as final-matrix
evidence for this combined checkpoint.

Linux T1/8/16 and constrained-memory confirmation remain deferred because SCC
access has not been renewed. ADR 0095's cross-platform acceptance is still open.
The existing allocation inventory reports source drift; its reviewed successor
and qualification remain separate work. Explicit or insufficient-budget
nonretaining CG span instability and weighted constrained-fit failures are not
repaired by this checkpoint. Serial weighted-slope reference losses remain
documented rather than masked by different tolerances or comparator settings.
