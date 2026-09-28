# ADR 0231: one-shot two-partition OLS slope ownership

Status: accepted for bounded development after SCC confirmation, 2026-09-27.

## Decision and scope

Give automatic two-partition OLS with at least one absorbed slope a private
owner for the coefficient-space Schur kernel already used by PPML. The OLS
owner copies the finalized design once and freezes normalized weights for its
entire lifetime. It has no weight, topology or sample mutation API. It retains
weighted rank-aware QR geometry and admitted coupling products across initial
RHS blocks and terminal residual projection, then releases them before
inference and saved-effect recovery.

The numerical kernel eliminates the largest partition's orthonormal basis and
solves the remaining Schur system with independent per-column stopping. Fast
mode admits all initial RHSs; low mode admits only the existing bounded block
width. This is the same kernel, with different storage capacities. It adds no
public option or competing fitting procedure.

Explicit solver choices, single-partition slopes and categorical-only fits
retain their existing routes. A caller that requires retained FE coefficients
is eligible only when it authorizes existing final-target recovery. There is no
benchmark-name, seed, regressor-width or observation-count guard.

The initial candidate also routed three-plus partitions. The sparse three-FE
pilot preserved all native scientific checks but cost a median 6.8% more in
paired command time. That wider routing is rejected. Eliminating one partition
leaves a differently coupled multi-partition system; merely sharing the current
kernel is not evidence of an efficient preconditioner for that problem.
Three-plus OLS stays on its prior route. PPML routing is unchanged.

## Scientific acceptance

Only cheap coefficient-space diagnostics determine when to reconstruct. Each
accepted projection reconstructs from its original RHS and current coefficient
product, then passes the existing fresh full residual certificate. Prior
projection results never certify another RHS. The final residual can pass the
existing fresh certificate before correction; otherwise the same solver
corrects it under the requested final tolerance. A failed call revokes geometry
authority in the shared engine. No tolerance or iteration cap changes.

Finite rank, pivoted finite solving, scores, bread, covariance, DoF, sample,
saved effects and transactional publication remain at their existing owning
boundaries. The private geometry label is scoped to one immutable owner and
cannot transfer to another fit. This does not extend the PPML cache lifetime or
authorize reuse under changing weights.

The public automatic report identifies the MAP-owned coefficient kernel. Its
iteration count is the maximum RHS Krylov count, not symmetric MAP sweeps.

## Admission and memory

Admit design construction before copying. Then admit the resident design,
normalized weights, serial reusable-owner scratch, rank-aware QR bank and
coefficient workspace against the existing execution parent. Zero-control
fits also charge the established outcome/residual overlap. Coupling products
and ordered parallel certificate scratch receive separate optional admissions.
Low mode charges its actual RHS block capacity. No existing workspace is
credited as a speculative saving.

The optional owner resides behind a fallibly allocated fixed-size box, keeping
its full state out of the common OLS fit descriptor. Its storage is covered by
the admitted owner byte count. Excluded routes allocate no box.

All failed construction attempts contribute to the eventual reported peak.
Budget/allocation refusal drops the optional owner and keeps the prior route.
An admitted numerical failure fails closed. The reusable engine's generic
unmodeled report is replaced by this complete one-shot lifetime bound; a new
boundary test caught the initially missing replacement. Construction and
solve storage are combined by their lifetime maximum, and final projection
releases the owner. The static whole-action allocation inventory still reports
its documented source drift; this ADR is not qualification of that inventory.

## Evidence

The [source-bound report](../../benchmarks/reports/development/ols-slope-owner-local-2026-09-27/README.md)
preserves both candidates, the rejected wider routing, original-source
diagnostics, benchmark failures and fresh confirmation. Independent explicit
WLS and sandwich checks cover both memory modes, unit/analytic/frequency/
probability weights, T1/T4, multiple slopes and saved reconstruction. Existing
rank-deficient/changing-weight kernel tests and categorical OLS/PPML controls
remain required. Native and reference disagreement stays unranked.

The [boxed-owner SCC confirmation](../../benchmarks/reports/development/ols-slope-owner-local-2026-09-27/linux-boxed07/README.md)
passes 90 applications, 36 native science pairs and 18 strict-stock pairs on
03292138. Two-partition paired medians are 0.425/0.526 of baseline and
0.847/1.173 of comparable stock. Excluded fast/low three-FE controls retain
0.8%/2.1% median losses and broad pair distributions. The prior 20% low-control
loss does not persist at that magnitude in this block; these timings do not
establish its cause. The larger case still misses the 0.90 stock objective.
This acceptance is bounded development evidence, not a universal speed or
non-regression claim, release qualification, or closure of the broader plan.
