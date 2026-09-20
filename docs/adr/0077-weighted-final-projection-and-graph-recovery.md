# ADR 0077: weighted final projection and graph recovery

**Status:** Accepted for local development
**Date:** 2026-09-20
**Decision owner:** Johannes Schmieder

## Context

The approved broad two-way OLS plan begins with weighted final projection and
saved-effect recovery. On the committed narrow-weighted endpoint, a dense AW
fit with six regressors spends 237 ms projecting its strict final residual and
369 ms recovering effects; the complete requested output command takes 1022 ms.

## Decision

Extend the existing fit-lifetime accepted Schur probe reuse to supplied weights
at eight or more requested workers. The initial probe and final solve borrow
the same immutable design and normalized weights. The final solve retains its
own tolerance, fresh certificate, memory admission, 64-step attempt and charged
CG continuation. It performs no new probe and omits the unused coefficient
product, as the existing unit-weight route does.

Permit supplied weights in the existing two-intercept graph-recovery attempt
at N >= 16384 and at least eight actual workers. Spanning-forest propagation
acts on an already projected target. Weighted component normalization, the
weighted FE certificate and the unweighted pointwise reconstruction check
remain authoritative. Tiny edge weights cannot hide an inconsistent target.
An uncertified attempt retains the existing iterative fallback chain. The
same admitted finishing workspace covers the coefficient vector, traversal
queue and returned channels; the queue is released before normalization.

The ordinary OLS recovery entry point enables this route; PPML recovery keeps
its existing exclusion. Loading, initial-probe admission, edge caching,
overlap, low-memory solver policy and thread thresholds are unchanged. No new
production allocation or numerical kernel is introduced.

## Evidence

The [local development report](../../benchmarks/reports/development/two-fe-weighted-finish-local-2026-09-20/README.md)
contains 82 successful application calls, 25 native pairs with no allocation
peak increase, 22 independent output audits, twelve Schur tests, three graph
recovery tests, the actual-plugin ABI check and three licensed-Stata fixtures.
Weighted strict final projection falls to 45 ms and recovery to 37 ms in the
separate phase call. Three-pair AW full-output medians improve 1013 to 493 ms;
default fixest takes 706 ms but misses the unchanged saved-output tolerance.
Separate matched-accuracy AW/PW comparisons pass at K=6 and K=50.

Shuffled AW fitting remains slower than fixest (median paired ratio 1.199).
Shuffled full outputs approach parity (0.991), with two of three candidate
wins. This checkpoint closes neither the broad graph milestone nor release
qualification. No installed package or remote state changes.
