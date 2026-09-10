# ADR 0032: reuse LSMR preparation during low-memory OLS projection

**Status:** Accepted for local development
**Date:** 2026-09-09
**Decision owner:** Johannes Schmieder

## Context

Low-memory OLS absorbs the response and successive bounded regressor batches
against the same finalized FE operator and weights. The borrowed adapter
previously called `within::solve_batch` for each batch, rebuilding the operator
and preconditioner each time. Johannes requested the next useful optimization
from tracker #26, including a check of the earlier multi-FE repeat failures.

## Decision

Retain one lazily constructed `within::Solver` across the projection batches
of a categorical, intercept-only low-memory fit when the topology planner
selects direct LSMR, or when LSMR is explicitly requested. The session borrows
the finalized design and fixed frame weights. Weighted construction makes one
fallibly reserved owned weight copy, consumed by `within` for its square-root
weights. Unit weights remain structural. No state survives the fit or crosses
a change of sample, weights, design, or solver configuration.

Each batch retains the existing RHS transport, independent FE certificate,
iteration budget, tolerance and coefficient reconstruction. Setup timing
includes operator construction only when it occurs; per-batch setup and solve
work remain charged. Private solver tracing records successful constructions,
batches and RHS counts. These are not independent operator-application counts.

The existing automatic positive-definiteness fallback drops the rejected
preconditioner before preparing the unpreconditioned replacement. Later
batches reuse that replacement while retention remains active. Before any CG
continuation, drop the retained solver and disable retention for the rest of
the projection; subsequent batches use the existing one-shot route. This
avoids overlapping retained LSMR and CG workspaces. Allocation and interruption
errors remain fatal. Existing warning and report-merging policies are preserved.

Drop the retained solver immediately after the projection loop, before finite
QR, VCE, final residual correction and saved-FE recovery. The joint execution
and FE-solver memory admission still covers the projection workspace. This
change extends the lifetime of the existing operator inside that phase; it
does not retain another design matrix or add a second preconditioner. Fast
fits, generic/slope adapters and output recovery retain their existing paths.
PPML's changing-weight checkpoints do not acquire an OLS fit cache.

## Acceptance and limits

Require exact scientific equality with the prior raw-moment candidate,
including exact within-process repeats, solver decisions and certificates.
Use alternating untraced command pairs for performance and separate traces
for setup counts. Focused unit comparisons cover multiple batch widths,
weighted and unit cases, explicit/automatic LSMR and one/seven native threads.
An independent licensed explicit-dummy fixture checks weighted covariance,
residuals and saved FE outputs across pool sizes with explicit Schwarz-LSMR.
Automatic and diagonal fixture failures reproduced on the baseline remain
separate failed evidence; they do not count as passing output guards.

The local receipt is
`benchmarks/reports/development/ols-lsmr-reuse-local-2026-09-09.json`.
Earlier failing large multi-FE cases remain failed evidence: a tiny parallel
repeat difference is not rounded away, and mixed LSMR/CG reports are not
silently reclassified. This is a bounded portion of tracker M1a. Reuse across
finite/output phases, generic adapters, changing weights and broader memory
qualification remains outside this decision. No F0, SCC, Windows, RSS or
release qualification is claimed.
