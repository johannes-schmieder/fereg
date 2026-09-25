# ADR 0180: close the retained-coefficient slope product experiment

Status: rejected for production; development decision, 2026-09-24.

## Decision

Keep the accepted production solver at computational checkpoint `711802bf`.
Do not integrate either retained-coefficient canonical product prototype. Both
are accurate on their declared frames, but neither provides enough reliable
benefit to justify its additional coefficient, row-lookup and reduction banks.
All prototype source and evidence remain in the
[source-bound report](../../benchmarks/reports/development/ppml-slope-product-closeout-local-2026-09-24/README.md).
No experimental solver remains in production or as an unused parallel engine.

The first candidate retains every solved local FE coefficient, independently
recovers the eliminated partition's coefficients from original inputs, and
reconstructs physical residual rows while forming their direct Gram. It retains
the accepted per-column Schur recurrence, its intermediate reconstructed check,
and the complete final original-input FE certificate. The additional recovery
and reconstruction costs dominate the removed product pass: 116 of 120 native
pairs lose, and 23 of 24 configuration medians lose.

The successor treats the recurrence only as a stopping diagnostic, retains its
coefficient proposal, and reconstructs all columns together with a coefficient
layout contiguous across RHSs. The unchanged full fresh certificate is the
acceptance gate for those actual reconstructed residuals. This removes the
intermediate reconstructed-moment check, not the final current-weight projection
requirement. The prototype fails if final certification fails; it has no new
production retry or publication authority. Its 24 median ratios range from
0.946 to 1.086; 60 of 120 pairs lose. Main three-FE/K10/T8 gains of 2–3% are
insufficient for the larger canonical/public redesign sought here.

## What the evidence establishes

Both artifacts pass independent explicit-design weighted projection, original
FE-score, direct Gram/RHS, finite coefficient and inverse checks across two/three
partitions, two widths, two coherent positive weight/working-response frames,
and T1/T4/T8. The analytic fixture separately covers nonzero weighted residuals,
locally deficient slope channels and column scales from 1e-60 to 1e60.
Reported maximum per-column iterations are unchanged in every measured native pair.

A separate diagnostic build localizes the surviving cost. In the three-FE/K10/T8
case, parallel coefficient proposals plus eliminated-coefficient capture take
about 20 ms. Coupling preparation takes about 2 ms, local geometry/reference
preparation about 3 ms, reconstruction/products about 1.8 ms and the full final
certificate about 2 ms. These are diagnostic spans, not headline comparisons.
The result does not support blaming the full certificate for most remaining
projection cost, or spending another cycle on a geometry-only micro-optimization.

The fixed-frame stock block uses the pinned unmodified library and the same
inputs/products, but runs separately and carries no paired public speed claim.
The first standalone invocation mistakenly capped OpenMP at one thread; preserve
it as unmatched diagnostic evidence. A declared rerun sets the cap to eight,
checks effective T1/T4/T8 settings before every call, has no warnings, and passes
all independent checks. The existing public harness already sets its cap per
cell, so this mistake does not adjudicate earlier public receipts.

## Boundaries and next intervention

This experiment materializes dense output and leaves the accepted normalized
workspace present. Its recorded extra coefficient/lookup/partial payload is
about 1.1–1.7 MB, not a complete admitted peak. There is no low-memory,
zero-allocation, whole-public-command, fresh-holdout or qualification result.
No production scientific check, tolerance, memory allowance, rank/weight/sample
policy or failure behavior changes. OLS and every PPML production route are
unchanged. The rejected prototypes do not establish that fused products or
certificate consolidation can never help another organization.

The next materially different candidate is RHS batching inside the existing
coefficient operator: share immutable coupling loads across compatible columns,
with separate recurrences and convergence decisions. Measure actual evaluated
lanes, including any converged-lane work, as well as setup, layout conversion,
fallback and full fresh certification. A cheaper application alone is not
adoption evidence; it must improve complete canonical and public calls within
the full memory plan. Do not tune the closed retention layouts on these frames.

The multi-FE 0.90-times-stock goal remains open. Existing inventory drift,
constrained-fit/CG failures, comparator discrepancies and deferred platforms
retain their previous scope.
