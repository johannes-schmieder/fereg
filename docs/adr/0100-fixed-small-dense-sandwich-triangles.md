# ADR 0100: fixed small dense sandwich triangles

**Status:** Accepted for local development; milestone validation remains open
**Date:** 2026-09-22
**Decision owner:** Johannes Schmieder

## Decision

The shared dense OLS/PPML sandwich reducer selects a fixed-array update for
every reported dimension from 1 through 16. Existing score and meat slices are
borrowed without allocation. Check all four shapes before modifying either
meat. Wider dimensions retain the contiguous generic loop.

Preserve production and certificate multiplication operands, per-coordinate
observation order, independent multipliers, score scaling, finite checks,
block partitions and compensated forward/reverse merges. Fresh covariance
certification remains mandatory. Solver selection, clustering, scratch-backed
inference and public syntax are unchanged.

## Evidence and limits

The [local report](../../benchmarks/reports/development/fixed-dense-triangle-local-2026-09-22/README.md)
binds 390 successful worker results, including 162 repeated target fits in four
sampling workers, for 548 target fits excluding warm-ups. All 166 timed native
pairs have exact exports, samples, saved arrays and non-timing solver fields,
with equal tracked allocation peaks. Nineteen focused Rust fixtures pass,
including independent legacy arithmetic, failure atomicity and shared PPML
weight/thread checks. Both new plugins pass actual ABI checks.

Two unchanged five-pair blocks confirm roughly 3–4% gains for serial AW K10
robust/full output fits and about 6% for unit weights, with smaller variable
PW gains. Ordinary one-/two-FE controls remain below the 5% regression limit.
Separate diagnostic traces roughly halve the narrow covariance phase.
Remaining serial fixest losses, Linux checks, broader milestone validation
and final source integration remain open. The captured computational sources
include the separately pending graph/window drafts.
