# ADR 0185: cooperative compact products for joint PPML directions

Status: accepted for local development, 2026-09-25.

## Decision

Use idle column workers for the single-RHS compact FE product in the existing
joint finite/FE solver. This extends ADRs 0130, 0134 and 0136 without introducing
another solver or storage representation. Canonical multiple-RHS absorption
continues to distribute columns. The explicit dense Schur product and single
worker execution retain their original scalar implementation.

For the compact Schur operator, write the product as the measured local Gram
and remaining-partition cross products, less the sum of eliminated-group
`C_g' C_g x` contributions. Divide the ordered eliminated groups into contiguous
blocks across admitted workers. The calling worker computes the base product
and its first block. Each helper accumulates its contribution into its existing
coefficient product bank, using its existing eliminated-group scratch. Merge
partial products in worker order after all workers join. No atomic accumulation,
new vector, descriptor, factor, thread team or memory-admission term is added.
Selection depends only on representation, group count and available workers.

The shared compact helpers preserve the old scalar arithmetic. Parallel
accumulation changes reduction grouping; cross-thread bitwise equality is not
required. A fixed worker count has fixed group boundaries and merge order.
Measured QR Gram blocks remain authoritative, including deficient local ranks;
there is no identity-Gram or full-rank shortcut.

## Scientific and ownership boundary

The curvature cancellation guard is unchanged. Refusal still invokes the
observation-space operator and squared norm. Recurrence diagnostics only
schedule reconstruction. Original-input residual reconstruction, full fresh
current-weight FE checks, original finite scores, model space, objective,
frozen rank, canonical strict/final construction, exact final Hessian, inference
and transactional publication keep their existing authority and tolerances.
No acceptance check or generation validation is removed or rescheduled.

Helpers are borrowed only during an ordinary joint product, when they have no
live column solve. Mutable worker banks are disjoint and all accesses stay in
the safe core on the existing installed thread team. SPI and interruption
boundaries are unchanged. The optional coupling's topology/weight generation
authority remains governed by ADRs 0124 and 0126.

The path serves admitted fast/low multipartition slopes and admitted low-memory
categorical joint directions. In particular, four categorical FE can use the
compact operator and benefit directly. Two-/three-FE categorical routes and
public OLS receive no new routing. Small dense operators are already inexpensive
and do not acquire cooperative work.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-cooperative-slope-products-local-2026-09-25/README.md)
preserves all distributions, controls, diagnostics and immutable receipts.
Independent analytic weighted projection tests explicitly force compact
representation, vary worker count and current weights, and cover deficient
slopes, disconnected components and scaled controls. Explicit-design licensed
Poisson/Hessian/sandwich fixtures protect estimation and inference. A new real
compact-route allocation fixture proves zero allocator events/growth through
16 warmed iterations in fast/low T1/T4 configurations; diagnostic counters
separately establish that it executes cooperative products.

Fresh large three-FE slope medians improve 4.4--5.6%, with about 18 to 6--7 ms
in the corresponding diagnostic FE-product phase. Four-FE low-memory PPML
improves about 11%; its separately instrumented product falls 29.8 to 9.8 ms.
There is no added tracked allocation peak or scratch bank. Selected-memory
telemetry varies by up to 112 bytes; this is not a physical-cap/RSS claim.

The serial slope control loses 4 ms initially and 3 ms on same-input
confirmation. An excluded OLS control loses 1 ms initially and 2 ms on
confirmation; scientific OLS exports are exact. These small losses remain
visible, and no universal non-regression or 0.90-times-stock claim follows.
The two-partition threaded slope case and narrow categorical PPML still lose to
stock. Reduce repeated Poisson weighted solves next instead of tuning this
operator further. Rejected dense factors and paired-lane experiments stay
closed. Existing scientific, allocation-inventory and platform deferrals remain.
