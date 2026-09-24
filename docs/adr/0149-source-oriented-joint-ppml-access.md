# ADR 0149: source-oriented access for joint PPML

Status: accepted for local development, 2026-09-24.

## Decision

Resolve each original finite column independently in the existing joint PPML
owner. Borrow a numeric slice after checking its active-row length, independently
of the complete model's width or other column recipes. Resolve an ordinary
factor's codes once per column scan; constants and vanished levels have implicit
values. Other recipes retain fallible generated-column evaluation. The source
owner, compact row axis and generated-column mapping remain authoritative.

This supersedes the joint column visitor's dependence on whole-model
`NumericOlsColumns` admission. It does not change that binding's existing width,
row-count or recipe guards for other consumers, including canonical construction
and OLS. A K64 public model includes an intercept and exceeds the 64-column
binding limit; isolated current-source counters establish that actual boundary.

Choose the scan organization from available storage:

* With complete direct numeric row bindings, retain the fused row scans for
  stable norms, residual reconstruction and inner residual moments. Their
  independent accumulators and observation reuse are valuable, especially
  under a serial team.
* When rows require recipe evaluation, scan one resolved source column at a
  time. Independent stable norm reductions can use the already admitted worker
  team. Reconstruction subtracts the FE contribution first, then updates the
  residual by finite coefficient order. Each residual moment still visits rows
  in order.
* Finite-product preparation and independent original-input finite scores use
  the per-column visitor regardless of complete-model binding availability.
  Dense retained representatives preserve their existing row organization.

The existing single-worker finite-Hessian row kernel and rare observation-space
curvature fallback retain their prior row visitor. This decision does not claim
that every recipe dispatch or whole-model access cliff has been removed.

## Scientific and memory boundaries

Loop interchange preserves each column norm's stable row-order recurrence,
each residual's finite-coefficient FMA order and each score's row-order
accumulation. Independent original-input scores and means retain fresh
current-weight arithmetic. The finite product helper and its fixed reduction
tree retain ADR 0148's contract. No weighted statistics or solver result survives
its existing generation merely because immutable source slices are borrowed.

Current-weight geometry, projection/model-space and finite-solve acceptance,
reconstruction, outer score/objective, strict agreement, canonical rank and final
bread/Hessian remain authoritative. No tolerance, sample, weight, DoF, separation
or failure-atomicity change. Generated recipes still return errors through the
existing safe-core boundary; SPI remains on the main thread.

The new visitors borrow immutable source storage and reuse existing scales,
coefficient, observation and residual banks. There is no new heap bank, N-by-K
copy, cache generation, worker team or admission formula. Norm jobs own only
disjoint scalar destinations and read the shared current roots. The source
orientation is an access-cost decision, not a solver-selection or benchmark guard.

## Rejected competing organization

Unconditional column scans produced a substantial public K64 improvement but
lost on the directly bound serial and narrow slope development cases. Keep that
artifact and its passing scientific evidence. The successor retains fused row
access when it is already direct, rather than adding thread/width thresholds
fitted to those timing cells. No third access-layout candidate is planned for
this milestone.

## Evidence and remaining scope

The [source-bound report](../../benchmarks/reports/development/ppml-column-access-local-2026-09-23/README.md)
records current-source binding counters, both candidate screens, fresh
confirmation, independent oracles, control losses and isolated phase attribution.
The final artifact passes 58 distinct Rust tests and seven licensed fixtures.
All 66 native export comparisons are exact and all 60 direct stock scientific
comparisons pass. The two fresh threaded cases above the old binding limit
improve by about 30%; wide serial gains are smaller and several controls retain
small losses. The mathematical weighted oracle exercises both
scan organizations under changing frames, disconnected/rank-deficient slopes,
T1/T4, zero responses and failure revocation. Licensed checks include K65
Poisson/Hessian/sandwich comparisons, a K65 allocation window, mixed generated
terms, separation and transaction failure.

This change serves streamed ordinary/strict joint directions and the independent
original-input checks for both streamed and dense joint directions. Fast
categorical LSMR, one-FE profiling and public OLS retain their algorithms.
Remaining canonical product ownership, fast categorical organization and public
output costs require separate measured work. Physical memory-cap and
cross-platform qualification are outside this local decision.
