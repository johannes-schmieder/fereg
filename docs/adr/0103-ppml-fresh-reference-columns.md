# ADR 0103: bind numeric columns for fresh PPML reference statistics

**Status:** Accepted for local development
**Date:** 2026-09-22

## Decision

Reuse `NumericOlsColumns` inside the retained weighted-linear session's
reference-statistic refresh. Bind immutable, sample-aligned numeric sources
once per refresh, then compute reference norms and means from the current
weight frame. The existing binding admits 8–64 numeric/intercept columns and
at least 4,096 active rows; other recipes retain the checked generic accessor.
The binding is stack storage holding borrowed slices, not a persistent cache.

Extend independent-column scheduling to numeric categorical PPML when the
retained width is at least eight, N is at least 16,384, and N times retained
width is at least 524,288. Require the existing fast execution mode and an
active worker pool with multiple threads. This generalizes the earlier
one-FE, N>=65,536 rule according to actual column work. Preserve that earlier
rule for generated recipes. Low-memory and slope projectors retain serial
reference accumulation. The public one-shot OLS path does not call this method.

Every column keeps the original row order, normalized weights, scaled norm
accumulator and compensated mean. The denominator is freshly recomputed in
row order. No weighted geometry, working response, norm, residual or score is
reused across generations. Existing rank, strict checkpoint, final-Hessian,
fresh certificate, admission and transaction rules remain authoritative.
No new heap buffer or matrix owner is introduced.

## Evidence and limits

The [local report](../../benchmarks/reports/development/ppml-reference-refresh-local-2026-09-22/README.md)
binds the exact source/artifact, isolated diagnostic instrumentation, alternating
pairs, stock comparator and independent dummy checks. One-FE screen and fresh
confirmation medians improve 179→162 and 158→143 ms. Sparse two-FE improves
359→347 ms; dense/other multi-FE gains are small. Large one-FE cases already
above the old scheduling threshold barely change. The narrow excluded control
has a small measured loss. Retain this inexpensive generalization and close
the experiment instead of tuning guards to individual cases.

All 39 completed native pairs have matching scientific exports. There are
375 passing focused Rust tests and four licensed fixtures, including a new
one-/two-/three-FE numeric PPML test with unit/FW/PW weights, offsets, independent
dummy Poisson coefficients, a directly formed current-Hessian sandwich and
T1/T4 checks. Threaded tracked peaks vary despite no new buffer; no exact
peak-neutrality claim is made. The frozen allocation inventory still fails.

Six native zero-outcome warmups fail the public separation execution-bounds
validator. Direct replay also fails on the original `7189e57a` artifact. This
pre-existing publication issue remains unresolved and takes priority over
another positive-outcome timing cycle. The change does not establish PPML/F0
qualification or eliminate the remaining stock-reference losses.
