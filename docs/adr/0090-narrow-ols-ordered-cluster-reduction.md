# ADR 0090: ordered reduction for narrow threaded OLS clustering

**Status:** Accepted for local development under the approved optimization plan
**Date:** 2026-09-21
**Decision owner:** Johannes Schmieder

Dense OLS cluster-score designs with dimension six or seven, at least 16,384
observations and at least eight workers use the existing ordered parallel
subset reduction. Wider threaded designs already use that route. Smaller,
serial and low-memory streaming designs retain their prior selection.

Stable radix ordering preserves row order within each cluster group. Existing
compensated group scores, complete-group partition boundaries, deterministic
partial reduction and independent covariance certification remain unchanged.
Every clustering subset and its inclusion-exclusion sign is still evaluated.
The PPML paired-independent route returns before this OLS selection guard.
Reservations retain the existing fallible ordering/partial-workspace admission.

The [local checkpoint](../../benchmarks/reports/development/narrow-cluster-local-2026-09-21/README.md)
records unit/AW/PW/FW screens, independent weighted inference and saved-output
audits, plus an exact integer-arithmetic group-meat oracle across 1/8/16 workers.
It preserves reference output discrepancies. Protected controls, final blocks,
Linux confirmation and a complete memory proof remain part of the active work.
