# ADR 0148: triangular blocked finite products for joint PPML

Status: accepted for local development, 2026-09-23.

## Decision

Keep the existing current-weight joint finite/FE solver. Construct only the upper
triangle of its symmetric finite Hessian, then copy that triangle to the lower
half. Dense preparation retains its projected-column dot arithmetic. Streamed
preparation forms the inner product of one normalized projected column with the
other original weighted column, and divides by the latter's checked norm after
reduction. This supersedes ADR 0145's duplicate symmetric products, per-observation
right normalization and averaging of the two computed directions.

For V = sqrt(W) X, checked column norms s, and the current orthogonal projection
P0, the entry remains

    H_ij = (P0 V_i / s_i)' V_j / s_j.

Since P0 is symmetric and idempotent, this equals the existing normalized Gram
entry. The finite factor remains a preconditioner, not rank or covariance
authority. Cholesky refusal and all independent acceptance retain their existing
policy. No difference of two large Gram matrices is introduced.

When multiple projected-column workers are admitted, use eight fixed independent
accumulators per inner product. Borrow a contiguous numeric source slice when
the existing binding supplies one; generated or unsupported sources retain
fallible column visitation. With one admitted worker, keep the existing complete-
row organization and independent entry accumulators, now restricted to the upper
triangle with right normalization after accumulation. Worker scheduling cannot
change an entry's reduction tree. Cross-thread bitwise equality is not promised;
registered numerical reproducibility and fixed-team determinism remain.

This affects ordinary and strict joint PPML directions in the existing low
categorical/slope and fast-slope routes. Fast categorical LSMR, one-FE profiling,
canonical rank/final-Hessian construction and public OLS retain their algorithms.
No public option or benchmark-specific routing guard is added.

## Scientific and memory boundaries

Every call retains current-weight geometry, stable finite norms, FE/control
products, finite factorization, reconstructed residuals, independent FE and
original-input finite scores, combined predictor checks and outer objective
acceptance. Initial rank, strict state agreement, final rank/bread/Hessian and
publication remain independently authoritative. No tolerance, iteration budget,
sample, weight semantics, DoF, separation or transactional policy changes.

The right weighted column has a checked finite norm before its product is used.
The projection's normalized column has bounded norm; Cauchy–Schwarz bounds the
exact absolute product sum by the two norms. Focused independent tests cover
scales from 1e-300 to 1e300, both access layouts and block tails. Full WLS and
Poisson/Hessian/sandwich oracles cover the accepted solve, beyond kernel agreement.
Nonfinite or deficient factors retain canonical refusal; no unchecked product
can become a published result.

There is no new heap bank, N-by-K copy, worker team, persistent cache or admission
formula. Numeric slices borrow immutable Rust-owned data. The product helper has
eight f64 stack accumulators per call. Existing RHS, factor and observation banks
retain their ownership and capacities. Main-thread SPI, the safe core/FFI boundary
and receipt-last publication remain unchanged.

## Rejected competing organization

A matrix-free prototype seeded a finite Cholesky preconditioner from an earlier
frame, transported its units by fresh column scales, and used the exact current-
weight observation operator with fresh acceptance. A frozen row-major K64 probe
improved, but public column-oriented data regressed substantially. Replacing the
forward row gather with column accumulation repaired most of that loss; the
revised public K6 and K64 cases still lost, and K24 improved only modestly.

Neither prototype is adopted. Remove their previous-factor state, selected-column
identity bank and alternate recurrence; preserve their source-bound evidence.
Do not fit width guards or preconditioner-aging rules to those cases. This rejects
the tested organization, not a mathematical claim that every matrix-free or warm
Newton method must lose.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-joint-triangular-products-local-2026-09-23/README.md)
records both rejected artifacts, frozen-frame diagnostics, stock rotations, fresh
confirmation, independent numerical boundaries and all control losses. The final
artifact passes 156 distinct Rust tests and six licensed fixtures. All final
60 PPML pairs retain iteration, scientific and modeled-memory metadata. Twenty-
five fresh low-mode pairs all improve; fast slopes include one +1-ms pair.
Dense categorical cases still lose to stock fixest. This is a bounded production
improvement, not completion of the per-case 0.90-times-stock objective.

Close this product experiment. Remaining original-column normalization,
reconstruction, certificate scans and canonical construction need separate
ownership/dataflow work. No physical-memory-cap, cross-platform or qualification
claim is made; the existing deferred failures and static allocation drift remain.
