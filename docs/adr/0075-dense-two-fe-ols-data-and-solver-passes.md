# ADR 0075: dense two-FE OLS data and solver passes

**Status:** Accepted for local development
**Date:** 2026-09-20
**Decision owner:** Johannes Schmieder

## Context

Johannes requested that the completed one-FE optimizations carry over where
appropriate, followed by local Mac work to beat stock fixest by at least ten
percent on the dense two-FE design. The frozen development matrix includes
inference, saved effects, full outputs, fresh seeds and shuffled observations.
It protects the committed one-FE endpoint and lower-thread/sparse controls.
It does not replace release qualification or change the statistical contract.

## Data preparation and scheduling

The existing bounded numeric row loader, optional loaded RHS and raw-moment
overlap accept multiple ordinary intercept effects. Each effect must have one
categorical source and no slopes. Existing numeric, unrestricted candidate,
unit-weight, row-count, width, thread and memory guards remain. Final prepared
sample, recipe and value matching determines whether the speculative RHS and
moments can be consumed. Stata SPI calls remain on Stata's main thread.

Independent outcome statistics may overlap eligible multi-FE fitting. The
scan allocates no buffer. Ordinary borrowed topology validation can accompany
it; categorical multi-FE DoF construction remains after fitting. Errors are
observed at their original semantic boundary. No-FE, weighted, small,
low-memory and slope-bearing fits retain their previous scheduling.

## Initial and final projection

Extend ADRs 0035/0037's initial Schur admission to 8–15 actual RHS columns for
unit weights on at least eight requested workers. The existing 16–64-column
weighted route remains. All other guards remain: automatic fast OLS, exactly
two intercept effects, at least 16,384 rows, joint memory admission, the
eight-step three-RHS probe and contraction at most 0.01. A rejecting probe
retains the legacy route. Both probe and attempted work are charged.

After an accepted unit-weight Schur probe on at least eight workers, the same
fit session may reuse that probe for its final one-RHS projection. It reuses
only the admission decision for the immutable graph and weights. It builds a
fresh solve, uses the final requested tolerance and requires a fresh direct FE
certificate. No pilot work is invented. The 64-step Schur cap, charged symmetric
CG continuation and original reference scales remain. Saved-output projection
retains the separate 1e-14 requirement from ADRs 0063/0064.

When final-target recovery is authorized and the initial Schur fit produced no
FE coefficient products, the final residual need not produce an unusable
residual-only coefficient product. Eligible fits can then use the existing
final-projection/bread/score overlap. Its owners were already jointly admitted;
allocation-phase probes keep sequential execution and errors keep their order.

## Counted graph edges and certificates

The capped unit-weight Schur attempt on at least eight workers may reuse idle
group-statistics scratch for neighboring FE codes and multiplicities. Admission
requires at most u32::MAX rows and an existing bank of at least 2N words. Each
group's neighbors are sorted by canonical code and repeated observation pairs
are combined. Disjoint checked membership ranges permit parallel construction.

Each private f64 storage word carries the bits of a u32 neighbor and a u32
multiplicity; no floating-point operation is applied to that representation.
A zero count terminates the unused group tail. The operator uses multiplicity
times the neighboring value. This preserves the mathematical unit-weight
operator but changes summation order. Independent observation-space checks
remain authoritative. The certificate overwrites this scratch after iteration.
There is no new heap owner; insufficient scratch and supplied weights use the
previous operator. PPML admission is unchanged.

The existing bounded group-partial certificate reduction extends from one to
two effects at 8–15 RHS columns. It still requires at least 65,536 group/RHS
entries and sufficient existing scratch. Individual moments, maxima and
pass/fail arithmetic remain; reported two-norm reduction can differ in its
last bits and does not determine acceptance on this route.

## Saved effects

Large unit-weight two-intercept OLS output recovery on at least eight workers
can propagate an already projected target through a spanning forest. It then
uses the existing component normalization and checks reconstruction on every
observation. An inconsistent or insufficiently accurate target continues with
the existing recovery chain. Requested channels and publication are unchanged.

The coefficient and traversal banks need 16 bytes per level; the traversal
bank is released before normalization. The existing 128-byte-per-level
finishing allowance covers both lifetimes. The attempted route must fit its
joint execution budget before allocation. Allocation and transaction rules
remain those of ADR 0064.

## Evidence boundary

The development reports preserve the
[loading and solver increments](../../benchmarks/reports/development/two-fe-dense-initial-local-2026-09-20/README.md),
[strict final projection](../../benchmarks/reports/development/two-fe-dense-finish-local-2026-09-20/README.md),
[failed shuffled holdout](../../benchmarks/reports/development/two-fe-dense-shuffled-holdout-local-2026-09-20/README.md),
and [counted-edge/overlap experiments](../../benchmarks/reports/development/two-fe-dense-operator-local-2026-09-20/README.md).
The original failures remain evidence; preliminary screens do not establish
final acceptance. The [final local report](../../benchmarks/reports/development/two-fe-dense-final-local-2026-09-20/README.md)
records both passing five-pair blocks across all 24 cells, 240/240 wins and
all 41 passing regression cells. The worst final-block paired ratio is 0.891304;
the regression maximum/geometric mean is 1.034483/0.995921. All 123 regression
allocation peaks equal baseline, and the checked native numerical bundles are
exact. Source/artifact identities, focused unit checks, public Stata fixtures
and independent output audits accompany the timings.

Default fixest saved outputs miss the unchanged output tolerance on this
design. Separate tight-reference fitting and independent FE normalization,
reconstruction and normal-equation checks adjudicate output accuracy. Stock
default timing remains the speed target; slower tight-reference timings cannot
substitute for it. No tolerance, sample, weight, inference or output contract
is weakened. No cross-platform or release claim follows from these local tests.
