# ADR 0166: close the frame-owned PPML operator integration

Status: rejected for production; development decision, 2026-09-24.

## Decision

Retain ADR 0165's accepted computation and the independent test-accounting
repair at `86124f72`. Do not adopt this frame-owned two-FE Schur matrix,
generation plumbing or optional memory admission. Preserve both implementations,
their tests and all timing/scientific evidence in the
[source-bound report](../../benchmarks/reports/development/ppml-frame-operator-integration-local-2026-09-24/README.md).
No experimental solver code remains in the worktree.

The implementation safely reuses current-weight geometry across compatible
canonical projections. It borrows existing group indexes, builds symmetric
positive couplings once per valid sample/topology/weight frame, and applies
the Laplacian through coefficient differences. Existing component-scaled PCG,
original-input reconstruction and independent full certificates remain
authoritative. Structural work bounds and complete optional memory admission
preserve the original route on refusal. Public LSMR checkpoints and low MAP
canonical projections can use the same owner without changing ordinary solvers.

## Evidence

Development whole-command gains are promising: wide low 120→110 ms, and the
final candidate's narrower fast case 50→44. Fresh confirmation does not support
broad adoption. Wide low improves only 137→136, unbalanced low loses 98→101,
larger low improves 185→183 and 70-control low 357→351. Serial low improves
108→101 and narrow fast 60→57, but their accepted baselines already beat stock
by at least 10% on these designs. Some excluded controls lose 1 ms. All paired
outer and reported inner iteration counts remain unchanged. These are local,
source-bound development measurements, not qualification claims.

All 300 public applications and independent public-state checks pass, as do
100 native paired comparisons, 75 distinct final-source Rust tests and six
licensed fixtures with explicit Poisson/Hessian/sandwich oracles. One fresh
disconnected stock comparison fails the coefficient envelope on all five
repetitions, identically for baseline and candidate; other checks pass and the
matrix is excluded there. It remains scientifically unresolved and unranked.
The other 95 direct stock comparisons pass. No tolerance is relaxed.

The matrix has a modest complete memory cost: fresh wide adds 57,920 modeled
bytes and 57,912 tracked peak bytes; N48000 adds 323,328 and 323,320. Exact
optional budget refusal retains the original solver and scientific results.
Small memory cost alone does not justify another production mechanism whose
gains are weak in the principal remaining-loss families.

## Consequences and next boundary

Restore accepted source and keep the broad PPML <=0.90-times-stock objective
unfinished. Seed202609378 is consumed. Do not tune matrix thresholds, fit a
benchmark selector, or pursue another isolated application-kernel variant in
this closed experiment. This is not a rejection of frame ownership in general,
nor evidence that stronger verification makes the public speed gap inevitable.

The next intervention is canonical weighted-product construction. Investigate
which current-weight FE/finite products the joint owner already computes, then
compare raw moments plus certified FE coefficient products with constructing,
writing and repeatedly scanning full residualized control panels. Preserve
canonical rank/status/QR/span authority; require explicit cancellation bounds
and materialized-row fallback when needed. Any redesigned verification schedule
requires its own invariant argument, decision and adversarial independent tests.
Fresh residual, score, objective, reconstruction and final-Hessian acceptance
remain mandatory. A small reusable factorization may support that larger
organization, rather than become another standalone overlapping solver layer.

OLS improvements, frozen definitions, historical discrepancies, allocation-
inventory drift, constrained-fit failures and deferred platform gates remain
unchanged. No push, release or qualification follows this decision.
