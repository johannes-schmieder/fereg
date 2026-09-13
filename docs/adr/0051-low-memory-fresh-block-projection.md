# ADR 0051: fresh low-memory OLS blocks and outcome ownership

**Status:** Accepted for local development
**Date:** 2026-09-13
**Decision owner:** Johannes Schmieder

## Decision

Low-memory OLS identifies newly built raw outcome and regressor blocks as
fresh inputs to the existing absorption dispatcher. Eligible automatic
one-intercept-FE paths use their existing direct initial projector and norm
scheduling, including intercept-only categorical interactions. This extends
the low-memory coverage left open in ADR 0050. No new projector, solver
family, worker pool or observation-by-control allocation is introduced.

Consume the one-column projected outcome matrix as the owned outcome vector.
Reuse certified within norms only when the linear frame's solver divisor
equals the projector's max-weight normalization (or weights are structural
unit weights). An explicit frame with another divisor keeps its original
weighted norm calculation. Missing cached norms also keep that calculation.
Raw reference norms, stable rank/QR policy and independent final certificates
remain authoritative.

Only newly constructed blocks enter the fresh path. Final residuals retain
their warm certificate. Retained LSMR/CMG dispatch and explicit solver or
saved-coefficient requirements remain intact. Outcome ownership and eligible
norm reuse also benefit those low-memory consumers without changing their
projection entry points. Scratch layout and admission budgets are unchanged.
Weights, samples, DoF, covariance, output ordering and transactional publication
retain their existing contracts.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/low-one-fe-local-2026-09-13/README.md)
records seventeen paired comparisons against accepted native `4e18c22`,
32 focused-filter Rust tests and both artifacts' independent Stata dummy
oracle. Across three seeds, narrow AW low-memory OLS improves 8.9% in median
paired command time and saves exactly 7,864,320 peak native bytes (8.2%).
The unit and PW-cluster targets improve 4.6% and 2.6%; no protected command
slows in the selected single pairs. Full b/V values and discrete scientific
decisions are exact; floating certificate diagnostics retain registered
tolerances.

All candidate primary native peaks fit unchanged models. The narrow AW
baseline exceeds its model; the reduced candidate fits it. Interacted low-FW
and weak-LSMR controls still exceed their models, so this is not closure of
P0 memory reporting or universal qualification. Sampled RSS is a separate
process measure and rises on some controls. This local adoption does not
claim hardware traffic, a cross-platform gain or a release result.
