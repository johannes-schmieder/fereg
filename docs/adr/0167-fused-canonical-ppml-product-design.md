# ADR 0167: select fused canonical PPML products for integration

Status: design selected for implementation; not production admission, 2026-09-24.

## Decision

Integrate the fused direct-product organization tested by
`benchmarks/ppml_canonical_products`, rather than the prototype's algebraic
Gram subtraction. No production estimator, route, certificate or memory policy
changes at this checkpoint. The [source-bound experiment](../../benchmarks/reports/development/ppml-canonical-products-local-2026-09-24/README.md)
is fixed-weight design-selection evidence, not a whole-command speedup.

For admitted two-categorical-FE systems, eliminate the larger partition, form
the weighted Schur Laplacian, ground one coordinate per component, scale and
factor the remaining system. Solve all compatible response/control columns
against that factor. Reconstruct residuals once in bounded group-disjoint
work. That pass produces the full direct Gram/RHS, residual norms and original-
design FE moments. Parallel partitions own complete eliminated groups and
reduce private retained-group moments, avoiding a private large-group-by-width
bank per worker. Rows need not be retained merely to recompute these products.

The alternative `H-U'U` organization is mathematically sound and accurate on
the frozen ordinary cases. Its subtraction can lose the small within variance
of an almost-FE-collinear column. The probe detects that condition but has no
complete factorization error argument for production rank decisions. Direct
products avoid that additional proof burden and have comparable measured cost.
Do not introduce both alternatives into production.

This is materially different from ADR 0164's rejected lazy coefficient-backed
rows and ADR 0166's rejected matrix-operator substitution. The useful boundary
is a shared fully absorbed product result, not repeated lazy reconstruction or
a faster matvec embedded in unchanged canonical consumers.

## Evidence

Eight fixed frames comprise four existing designs and a declared positive
weight perturbation of each. Both alternatives and the accepted low-memory
panel/moment pipeline use identical weights, original columns and tolerance
1e-12 at T1/T4/T8. Seven alternating rotations compare complete product calls;
setup is measured separately and charged once in the cold estimate. The
baseline includes its separate response and ten-control scratch blocks; it
does not imply that all originating public fits used low mode.

The final block passes 72 independent explicit-design projection/Gram
configurations (1,134 column checks) and 72 separate explicit-FE/QR finite
coefficient/inverse configurations. Three focused Rust boundary tests cover
analytic weighted contrasts, disconnected component mass ratios of 160 orders,
column scales spanning 140 orders, changing weights, near-FE-collinearity and
invalid/unrepresentable inputs. This is not full PPML or rank-policy validation.

The fused wide T8 product call improves 19.177 to 2.267 ms, with 0.352 ms
candidate setup; unbalanced T4 improves 7.270 to 1.204, with 0.997 ms setup.
Preparation-charged estimates are 0.136/0.299 times baseline. Disconnected T4
is only 0.961 times baseline after 5.255 ms setup. An earlier algebraic block
loses on that shape after setup. Such small margins are not an admission case.
All measured warm candidate pairs improve; this does not establish a public gain.

## Required production boundary

1. Own immutable topology separately from weighted factors, selected columns,
   responses and accepted products. Borrow original source-oriented controls;
   do not introduce an N-by-K normalized original bank. Rebind or invalidate
   every dependent generation before fallible work. No prior-weight result or
   failed construction grants authority.
2. Preflight all persistent and transient capacities with fallible allocation.
   Account for coefficient storage, factorization, private reductions, source
   bindings, row output and overlap with existing canonical/strict state.
   Structural factorization/assembly work and complete memory admission must
   decline unsuitable large reduced systems. The prototype's ordinary vectors
   and payload estimates are not production admission.
3. Transfer fully absorbed products directly to canonical norm, Gram, finite
   solve and bread consumers. Preserve kept/omitted/FE-collinear policy and
   guarded factorization. Materialize the established row representation when
   canonical QR/span checks require it. Never substitute the ordinary joint
   owner's partially eliminated Hessian for the final within Gram.
4. Keep the original-input projection witness independent of the coefficient
   recurrence. Derive a documented reduction-error argument for its fused
   implementation. Preserve model-space, finite-solve, inverse, original-input
   score, objective, reconstruction and final-Hessian acceptance; no changed
   tolerance, sample, weight, DoF, separation or publication policy follows.
5. Charge final inference, required materialization, outputs and fallback in
   an early public baseline/candidate/stock screen. Confirm a promising result
   on predeclared fresh cases and OLS/slope/one-FE controls before adoption.
   Replace superseded consumers instead of accumulating an unused parallel
   product interface. The broad <=0.90-stock objective remains unfinished.

No stock, licensed-Stata, new public Poisson, constrained-memory, OLS timing or
platform campaign was run for this development-only module. Existing unresolved
science and deferred failures retain their prior status.
