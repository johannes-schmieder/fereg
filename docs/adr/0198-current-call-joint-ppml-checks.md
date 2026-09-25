# ADR 0198: share current-call joint PPML acceptance work

Status: accepted for local development, 2026-09-25. Not qualification.

## Decision

Retain jointchecks1, based on 181735c6 with the three source overlays recorded
in the [source-bound report](../../benchmarks/reports/development/ppml-joint-checks-local-2026-09-25/README.md).
Reduce repeated original-column norms and serial acceptance passes at the
ordinary/strict joint PPML boundary. Keep the existing solver, checkpoint order,
scientific bounds and current-weight final-Hessian construction.

The streamed joint solver computes stable weighted input-column norms before
its recurrence. Its selected-column scale bank remains immutable through a
successful solve and fresh FE certification. Expose that bank only when this
call has a certified report. Failed calls clear the report before validation;
they cannot expose norms from an earlier call. The session borrows these norms
only when the supplied columns were immutable originals, with the existing
sample/topology, selected-column and current-weight identity checks.

Fast dense columns are FE-equivalent representatives, whose norms differ from
original norms. Their acceptance continues to recompute original-column norms.
This is a same-call denominator handoff, not authority to reuse weighted state
across generations or to validate a score with a recurrence residual.

## Independent acceptance and arithmetic

Every finite score numerator and current finite mean numerator still scans the
original prepared column and current normalized weights, using the same
compensated sums and observation order. Only its immutable input norm may be
handed off. Score normalization retains the existing division order and exact
zero-moment handling; a rounded intermediate zero must not bypass later scaling.

One serial response pass computes the stable original-response and reconstructed-
residual norms, compensated weight sum and optional normalized-intercept response
mean. Each statistic keeps its prior row order and arithmetic. This shares row
loads and square roots without changing the reference norm or denominator.
The optional mean goes directly to current-call coefficient reporting; the
other frozen reporting entry continues to calculate its own mean.

The existing independent reconstruction certificate runs on bounded row tiles.
It checks every original response/residual pair with the same cancellation-aware
bound. Its maximum-relative-error and boolean conjunction are order-independent.
These response checks and the independent column checks run concurrently on the
already installed worker team. Errors remain errors, and no partial result is
published. Cheap recurrence diagnostics, full fresh FE certification, outer
objective/score, model-space, rank, finite solve, strict consistency, reconstruction
and exact final q*mu Hessian requirements are unchanged.

This replaces redundant input-norm computation and the serial ordering of
independent acceptance work. It does not remove or delay a required certificate.
Ordinary and strict joint slope paths and admitted low categorical joint paths
benefit. Categorical LSMR, public OLS and other frozen solves retain their routes.

## Ownership and memory

Borrow the existing joint scale slice. During acceptance, the existing finite-
solve vector stores per-column norms until the independent response scale is
available. Initial canonical rank construction already reserves it for all
stored columns; the active rank cannot exceed that capacity. Its prior solve
contents have no live consumer at this boundary. Subsequent canonical/frozen
solves initialize their scratch normally.

Response moments and reconstruction reductions use stack state and bounded
existing-team tasks. There is no new retained owner, observation bank, workspace
allowance or worker team. All numerical state still follows existing failure,
sample, weight and basis invalidation. Main-thread SPI, safe-core/FFI boundaries
and transactional publication are unchanged.

## Evidence and limits

Thirteen focused Rust tests, six licensed scientific fixtures and four fixed-
configuration warmed-allocation executions pass. Independent weighted analytical
truth, Poisson/Hessian/sandwich oracles, changed frames, rank-deficient slopes,
extreme scales, failure revocation, strict rollback and budget refusal are covered.
Allocation windows measure sixteen iterations with zero events/current growth.

All 182 public states, 120 baseline-anchored comparisons and 58 direct stock
pairs pass; all 62 native PPML pairs have byte-identical coefficients, covariance
and means. Fresh seed431 has 32 paired wins, five losses and three ties. Seven
of eight medians improve by 1–5 ms, and the excluded narrow median ties. Four
medians meet the 0.90-stock target, all boundaries already met by their baseline
in this block. Low categorical and threaded slopes retain substantial deficits.
No further tuning of this bounded checking change is warranted.

Tracked peak and scratch deltas are zero. OLS controls have nine exact-export
pairs but a small-fit median loss of one millisecond; no universal non-regression
claim follows. Static allocation drift, constrained-fit/CG failures, the older
three-FE discrepancy and platform/qualification gaps remain open. The next
structural work must remove larger canonical/preparation or narrow-lifecycle
costs. This accepted shared improvement does not complete the broad goal.
