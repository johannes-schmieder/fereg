# ADR 0097: reuse serial slope recovery geometry

**Status:** Accepted for local development; milestone validation remains open
**Date:** 2026-09-22
**Decision owner:** Johannes Schmieder

## Decision

The existing direct single-effect OLS recovery may retain each group's scaled,
compensated Gram matrix and channel scales through its fresh residual check.
This cache is admitted only with one actual worker, at most four channels and
within the existing large-sample direct-recovery guard. It belongs to one
recovery call with immutable effects and weights. It does not reuse fit geometry:
the fitting and recovery solvers have different scaling and rank policies.

The finishing certificate recomputes residual scales and compensated cross
moments from the newly reconstructed residual, then performs the unchanged
rank-aware solve and certificate calculation. Scatter, pointwise reconstruction,
norms, reduction order and all tolerances are preserved. No certificate result
is cached. PPML retains its existing recovery; threaded calls retain the prior
direct path. Automatic solver selection and the public interface are unchanged.

The compact cache requires `levels * channels * (channels + 1)` doubles.
Checked preflight includes it in the joint recovery envelope before allocation;
actual capacities are checked too. A budget that excludes the cache still tries
the existing uncached direct recovery before MAP. Failed attempts remain charged
to the recorded workspace peak. No cache survives the recovery call.

## Evidence and limits

The [local report](../../benchmarks/reports/development/serial-slope-recovery-geometry-local-2026-09-22/README.md)
preserves 251 application calls, five focused release fixtures and both artifact
ABI checks. Two independent five-pair confirmation blocks show roughly 1–4%
gains in PW saved-output cases and 3–4% in FW, with AW robust effectively neutral.
All 106 timed native pairs have exact numerical exports, samples and requested
saved arrays, with equal tracked peak allocation. Four diagnostic comparisons
also preserve exact results. Rank-degenerate, extreme-scale, threaded rejection,
budget fallback and protected PPML allocation/atomicity fixtures pass.

These results retain a modest local refinement; they do not close the remaining
serial fixest losses, complete Linux validation or qualify a release. Measurements
include the separately captured graph/window drafts, and claims attach to those
full sources rather than the isolated commit.
