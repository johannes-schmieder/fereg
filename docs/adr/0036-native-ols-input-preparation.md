# ADR 0036: native OLS input preparation

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-12

Johannes requested further local one-FE optimization, specifically experiments
moving suitable Stata preparation into Rust while preserving fereg's other
strengths. This development decision replaces the loading procedure in
ADR 0013, items 1–3. Its sample, numerical, transaction, ownership, and
qualification requirements remain in force.

For an unrestricted all-numeric OLS candidate universe, the existing single
substantive plugin call loads observation-order numeric batches into native
columns. The C bridge returns raw SPI values and the SDK's runtime
`SV_missval`; Rust constructs missing masks using Stata's ordered double
missing range. The ado no longer scans all sources with `markout` first.
The protocol-1.5 `input_complete` field remains accepted and checked if set;
current ado sends zero. Requests with restrictions or string sources retain
candidate-gated scalar SPI loading. Authoritative native classification still
determines availability and domain behavior for missing and weighted inputs.

The OLS transaction preserves observation order directly and omits the
`sortpreserve` wrapper. Syntax/factor metadata and time-series materialization
remain in Stata. None of these operations sorts; native row maps govern
stores. Any future sorting operation must preserve order on all exits.

After all SPI loading finishes, wide IID preparation without categorical
interaction/slope/saved-effect requirements may run on the same requested
worker team used by estimation. The guard requires at least 16 numeric
sources and 16,384 candidate-axis rows. The completeness predicate checks
independent columns in parallel only inside an installed team. It selects
an ownership shortcut. Incomplete unweighted IID inputs use the same row
classifier in independent 16,384-row blocks, with errors selected in original
row order. Stable column compaction consumes its retained-row map. Input
buffers are reused within the former candidate-axis capacity bound; restricted
samples otherwise allocate compact columns fallibly. Narrow inputs classify
and compact serially. Weighted and other preparation paths, including PPML's
allocation-free preflight visitor, are unchanged. No SPI session enters the
worker closure and no global Rayon pool is created.

No solver, sample, weight, rank, covariance, tolerance, certificate, or public
result formula changes. A smaller loading chunk was tested and rejected for
lack of a consistent gain. The retained chunk is 65,536 rows, with interruption
checks between chunks. Obsolete column-by-column bulk-read helpers are removed.

The [local input report](../../benchmarks/reports/development/fixest-input-local-2026-09-12/README.md)
binds the source, artifacts, comparisons, independent dummy oracles, missing
code/storage checks, and transaction tests. This is local development evidence;
Linux, Windows, SCC, and release qualification were not run.

The [compaction report](../../benchmarks/reports/development/fixest-compaction-local-2026-09-12/README.md)
records the subsequent incomplete-unweighted implementation, source patches,
repeated missing-input comparisons, retained-pair memory checks, singleton
negative controls and independent restricted-sample dummy regressions.
