# ADR 0034: fixed LSMR parallel reduction order

**Status:** Accepted for local development
**Date:** 2026-09-10
**Decision owner:** Johannes Schmieder

## Context

Four large seven-thread OLS captures fail exact repetition after otherwise
successful estimation. The pinned LSMR vector kernels use indexed parallel
chunk sums whose leaf grouping can change when workers steal tasks. Different
floating-point associations can perturb subsequent recurrence coefficients.
Johannes requested the repair and approved publishing the tested upstream
patch and updating fereg's dependency pins locally.

## Decision

Pin both `within` and `schwarz-precond` to upstream commit
`1ac3522a251c2622dbeab451415924f9ebe117b9`. In the fused update/norm and
parallel dot kernels, `with_max_len(1)` makes each existing 4096-element chunk
one reduction leaf. Rayon's indexed midpoint splits and ordered left/right
merges then fix the addition tree independently of worker stealing.

Keep the existing parallel thresholds, per-chunk arithmetic, allocations,
solver APIs, routing, RHS scales, iteration budgets and independent residual
certificates. The change stays in the upstream implementation; fereg uses a
published git pin without a local path override or copied solver internals.
Other upstream reductions retain their existing contracts. This does not
promise whole-estimator bitwise equality across thread counts or platforms.

## Evidence and limits

The two upstream tests exercise threshold boundaries, long tails and nested
parallel work against the same fixed tree in a one-worker pool. They fail on
the prior source and pass with the patch. The unchanged strict Stata capture
requires exact repetition of coefficients, covariance, status, fit, DoF and
certificates; all four original failures and seven selected controls pass
alongside complete pinned-reference comparisons. Independent OLS oracles,
shared PPML tests, weighted saved-output fixtures and transaction checks
protect the shared boundary.

The source-bound prepublication evidence is
`benchmarks/reports/development/ols-lsmr-repeatability-local-2026-09-10.json`.
The published git pin is verified separately in
`benchmarks/reports/development/ols-lsmr-repeatability-pin-local-2026-09-10.json`.
Source-extracted kernel measurements support the retained iterator-bound
implementation; the slower recursive prototype and unnecessary scatter edit
are excluded. These are component measurements, not command-speed evidence.
Failed baseline captures never provide accepted performance evidence.

Changing the addition tree can intentionally change last bits across source
revisions. Record those differences separately while preserving exact
self-repetition and the existing independent certificates and references.
The frozen F0 exact-extraction policy remains unchanged; local development
does not constitute F0, cross-platform or release qualification. Historical
receipts and the reviewed patch remain immutable.
