# ADR 0110: Contiguous compensated rows for scratch certificates

Status: accepted for local development, 2026-09-22.

## Decision

Use a private row update for the fresh scratch-backed OLS score Gram and
robust covariance certificate meat. Each matrix coordinate keeps its scalar
Neumaier operations and observation order. The weighted Gram keeps the product
`(weight * left) * right`; the robust certificate keeps its existing scaled
score products. Both still compute the full matrix, without assuming weighted
product symmetry or changing a reduction tree.

Validate all three slice lengths before mutation. Compute the independent
coordinates of one private matrix row without per-coordinate early exits,
remembering every nonfinite product, sum or correction. Reject the row before
the caller can continue if any such value occurred. This may modify more
private coordinates on an error than scalar early exit did; the enclosing
certificate returns the same invalid-data class and cannot publish a result.
Shape errors leave the row untouched. Finite checks are retained, not deferred
past a certificate or replaced with an aggregate tolerance.

The update borrows existing score, sum and correction slices. It introduces no
allocation, worker team, memory admission change or retained state. Score Gram
and robust covariance certification remain separate fresh traversals. Production
influence accumulation, bread multiplication, tolerances, DoF, weights, rank,
sample, outputs, dense kernels and PPML iteration state are unchanged.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/low-memory-gram-local-2026-09-22/README.md)
records 5.5–21.4% paired median gains across eligible low-memory cases and
median ratios of 1.00 for excluded fast/IID controls. The wide trace reduces
the two certificate phases from 308/340 ms to 112/110 ms, while production
covariance remains 366 ms. All 48 native scientific pairs are byte-identical.

Twenty-seven distinct Rust tests pass, including debug and optimized scalar
bit/error comparisons, known cancellation, corruption and allocation failure.
Three licensed independent weight/rank/transaction fixtures and five explicit
dummy pilot cases pass. The disconnected three-FE stock covariance comparison
has different DoF and remains unranked; diagnostic rescaling is not a revised
headline result or an independent rank adjudication. Most measured stock gaps
remain large. Parallel multi-FE tracked-peak variation is preserved; no tighter
whole-action memory, cross-platform or qualification claim follows.
