# ADR 0035: guarded two-FE Schur absorption for wide OLS

**Status:** Accepted for local development
**Date:** 2026-09-12
**Decision owner:** Johannes Schmieder
**Extended by:** ADR 0037 for weighted-operator OLS admission

## Context

Johannes approved local work to approach fixest on one-FE overhead and
well-connected 50-control absorption while preserving fereg's strengths on
weak graphs. This extends ADR 0021's categorical OLS routing within the
restricted envelope below. It does not change explicit solver semantics,
PPML routing, scientific tolerances, or qualification policy.

## Decision

Before the existing CMG/LSMR selector, fast OLS may probe a two-effect,
intercept-only design with unit weights, at least 16,384 observations and
16–64 actual RHS columns, including the outcome and any explicit constant. Saved-FE coefficient
retention is excluded. Automatic certified continuation must be enabled and
the caller must allow more than 72 iterations. Low-memory, weighted, narrow,
more-than-two-effect and slope-bearing fits retain their existing routes.

The probe runs eight degree-preconditioned Schur CG steps on the outcome,
first regressor and middle RHS. It uses the full prepared graph and three
coefficient-space vectors, without copying the observation matrix. A maximum
remaining/initial preconditioned residual norm ratio of at most 0.01 selects
the full-width solver. A nonfinite or degenerate recurrence rejects selection.
The probe workspace is released before constructing the full solver or the
previous route. These are conservative development thresholds, not a proven
bound on routing regret for arbitrary graphs or outcomes.

For left/right incidence matrices L and R, B = L'R and degree matrices D_L
and D_R, the solver applies S = D_R - B'D_L^{-1}B without materializing B or S.
It eliminates the larger FE dimension and solves the smaller coefficient
system with D_R preconditioning. RHS recurrences are independent. Membership
traversals preserve each group's canonical row order and use disjoint group
and observation slices for parallel preparation and reconstruction.

The internal tolerance uses the existing direct-LSMR tightening function:
two decimal orders tighter than the caller's tolerance, with a 1e-14 floor
only when the caller did not request something smaller. Recurrence readiness
includes the square-root group-weight factor used by the true FE-moment
certificate. A fresh observation-space FE certificate is mandatory. The
existing independent OLS rank, score, bread, covariance, residual and output
checks remain authoritative. No certificate is replaced by the probe or by a
recurrence estimate.

After at most 64 full-width Schur steps, a nonconverged/broken recurrence
continues from the untouched original matrix with ordinary symmetric
CG/Hestenes. A finite reconstructed candidate that misses its certificate
continues from that candidate. Both retain the original raw-RHS certificate
scale. All Schur workspace is released before CG allocation. Probe, attempted
Schur and continuation iterations share the original budget; setup and solve
times include discarded work. Allocation and nonfinite-data errors remain
fatal. A zero/negative remaining budget fails closed.

The private workspace preflight admits the route only when both Schur and
its CG continuation fit the joint execution/solver budget. For L >= R levels,
Q RHSs and N rows, Schur floating-point storage is bounded by
8 * [L + R + (2L + 4R)Q + 12Q] bytes, plus 8 * [LQ + L + ceil(N/512)Q]
for the existing parallel certificate/reduction buffers when applicable.
Take the maximum with the released three-RHS probe storage. Neither path
adds an observation-scale copy for routing.

Reports retain requested `auto`, used `cg`, eight pilot iterations and an
accepted contraction ratio for Schur selection. This is coefficient-space Schur CG; an explicit `solver(cg)`
continues to mean symmetric MAP with its Hestenes stopping rule. Probe
rejection reports the actual legacy solver and charges the probe to setup
and the shared iteration budget; it is a planned choice, not a failure.
A nonfinite probe ratio is reported as 1 (rejected), rather than publishing
nonfinite diagnostics. Schur-to-symmetric-CG continuation additionally sets
fallback and records a warning. No new public solver option is introduced.

## Shared and one-FE improvements

The exact one-FE OLS entry point now reaches its existing direct initial
projection instead of entering the generic direct-LSMR wrapper. Freshly
certified residual norms move with the residual matrix into the finite OLS
solve, avoiding a second scan. The cache is private, per result, and applies
only to that exact matrix and its normalized weights.

Initial one-FE OLS and the new Schur route use fixed 4,096-row LASSQ partitions
with a canonical merge for raw/residual norms at supported widths. Serial
and parallel executions use identical partitions. The existing dot scratch
stores partials; there is no added N-by-Q allocation. PPML norm arithmetic and
its initial four MAP sweeps are unchanged. The shared Schur kernels gain
parallel group preparation/reconstruction and the stricter group-weight
readiness scaling.

## Evidence boundary

Focused local tests and comparisons belong in the development report and
STATUS.md. Weak/cycle, disconnected and unbalanced graphs, omitted route
classes, weighted shared kernels, memory bounds, forced continuation and
independent dummy fits accompany the principal timing comparisons. A native
reference accuracy/DoF disagreement remains visible and is not converted
into an equal-accuracy speed claim. No SCC, frozen F0, cross-platform
scientific qualification or release admission is implied.
