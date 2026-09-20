# ADR 0084: weighted independent statistics overlap

**Status:** Accepted for local development
**Date:** 2026-09-20
**Decision owner:** Johannes Schmieder

## Decision

Extend the existing bounded OLS outcome overlap to supplied weights in IID,
robust and clustered inference. Eligible fast intercept-FE calls retain the
existing active-row minimum (16,384), model-width maximum (64), eight-worker
minimum and prepared-view guards. The worker scans immutable source weights
and outcome independently of fitted moments. It retains only source borrows,
scalar weight facts and deferred reference-sum/TSS results; no heap bank is
added and no worker calls Stata SPI.

Source identity includes outcome and weight slices, active N, weight kind and
structural-intercept status. Consume speculative reference sums and TSS only
when the fitted weight divisor and multiplier match exactly. Predict that
multiplier from the direct compensated normalized sum used by the fit; retain
the original raw-sum round trip for the independent fitted-scale check. A
source or exact-scale mismatch discards speculative statistics and uses the
original fitted-view scans. Original fit, DoF, weight and statistic error
boundaries, reference-scale checks and certificates remain authoritative.
Frequency effective N and PW's rejection of IID inference are unchanged.

## Evidence boundary

The [local report](../../benchmarks/reports/development/two-fe-weighted-statistics-overlap-local-2026-09-20/README.md)
retains 170 successful application calls, 67 bitwise-exact native scientific
pairs including two diagnostic pairs, sixteen exact and independently audited
output pairs, nine focused tests and three final-artifact Stata fixtures.
All native allocation peaks are unchanged. Tests cover AW/PW/FW, extreme
common scales, exact-scale rejection, distinct source allocations, invalid
weights, error ordering, one/two/three FE and size/thread boundaries.

Three-pair shuffled AW K=6 IID improves 305 to 292 ms versus default fixest's
302 ms; ordered K=50 improves 748 to 721 ms. No cell median paired native
regression exceeds 5%. The isolated profile shows the post-fit weight report
falling from 10.192 ms to 0.001 ms in the large shuffled case; independently
computed source statistics now overlap fitting. Nine default-reference
saved-output discrepancies remain explicit. Small graph losses and the broad
repeated scoreboard remain open; this is not release qualification.
