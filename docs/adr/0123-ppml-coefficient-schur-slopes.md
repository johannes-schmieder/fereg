# ADR 0123: current-weight coefficient Schur absorption for PPML slopes

Status: accepted for local development, 2026-09-23.

## Problem and decision

Repeated generic weighted group solves and symmetric observation-space MAP
are expensive for multipartition slopes. The previous certificate geometry
cache removes repeated QR construction inside a call, but not the projection
Grams or slow recurrence. The user authorized replacing this organization and
revising intermediate verification while preserving scientific acceptance.

For publication PPML in the MAP-owned family with multiple effects and at
least one slope, admit one coefficient-space Schur workspace and the existing
rank-aware weighted QR geometry. This replaces ordinary residual MAP on that
admitted path. Keep the generic bounded route for insufficient optional
workspace; categorical, one-effect, mu separation, explicit non-MAP families,
and public OLS routing remain unchanged. There are no sample-size, topology,
seed, width or thread thresholds fitted to the development cases.

For each group, let Q be its weighted orthonormal basis, using the existing
scaled pivoted reorthogonalized QR and rank threshold. Choose the partition
with the largest declared coefficient dimension as Q0; let B concatenate the
other bases. Solve

    B' (I - Q0 Q0') B a = B' (I - Q0 Q0') sqrt(W) y

with conjugate gradients in coefficient coordinates. Reconstruct the residual
as `(I - Q0 Q0') (sqrt(W)y - B a)`, then undo weighting. Global coefficient
null directions need not be identified: the fitted subspace is the estimand.
Curvature is evaluated as the squared observation norm. Nonfinite/zero
curvature or iteration exhaustion fails closed, never publishes a speculative
iterate, and never invokes the unrelated nonretaining generic CG route.

Each RHS is scaled by its original weighted norm, solves independently, and
stops independently. Balanced contiguous blocks use at most min(RHS, threads)
workers without allocating a task list. No per-column arithmetic depends on
which worker owns it. The session retains only capacity across calls. Every
absorption invocation rebuilds weighted QR geometry; it is never trusted under
new weights, a new response call, or a sample generation merely because a
pointer or vector length matches. No cross-call numeric cache is introduced.

## Verification scheduling

The coefficient recurrence is only a cheap diagnostic. A sufficiently small
recurrence residual, or the iteration cap, triggers reconstruction from the
original RHS and current coefficients, followed by fresh moments against all
current-weight local bases. A failed diagnostic reconstruction replaces the
recurrence moments and restarts its search direction within the same cap.
Only a finite reconstructed residual with the required local projection bound
can proceed to the existing full fresh certificate, which recomputes residual
norms, raw moments and rank-aware span projections at the unchanged tolerance.

The full certificate on the *input* RHS was an early-return optimization,
not an acceptance requirement. The new route prepares identical QR geometry
without that input scan. Already-projected inputs are handled by the initial
coefficient moments/reconstruction and still receive the full final check.
This supersedes ADR 0121's unchanged-check-schedule restriction for the new
kernel only. Generic MAP still follows its original schedule. The unused
input diagnostic does not establish sample, model space, rank, finite-solve,
score, objective, reconstruction or Hessian validity; all those acceptance
conditions remain independently enforced at their existing owning boundaries.
No tolerance or iteration cap is relaxed.

The unchanged public MAP engine row denotes the canonical projection owner,
as with its existing checkpoint accelerators. On this slope kernel its
iteration count is the maximum per-column Krylov count for an absorption,
not a count of symmetric MAP sweeps. The public help documents this distinction;
operation-level diagnostic receipts provide comparisons between algorithms.

## Ownership, fusion and memory

Dense slope sessions use the existing response/control fusion formula:
previous response residual plus the change in working response, together with
previous within-controls. These are initial guesses only; new weights cause
new geometry and new acceptance. A separate retained response vector prevents
aliasing the generic owner's normalized-weight scratch. Low-memory execution
keeps bounded blocks and does not require this fusion.

Charge the QR bank, a normalized N-by-RHS bank, N square-root weights, group
metadata, worker descriptors, four coefficient vectors and one observation
vector per worker, the boxed owner, and N prior-response values. Coefficient and QR vector
reservations are fallible and checked against actual capacity. Prior-response
storage uses the existing fallible exact-reservation helper and is included in
the session capacity fingerprint. No inner step grows workspace. The PPML plan charges this optional state through ordinary
and strict linear work, re-admits it after fresh telemetry, and releases it
before inference/recovery. Admission refusal preserves the generic route.
The documented whole-action/static-allocation inventory drift remains open.

## Evidence and boundaries

The source-bound [development report](../../benchmarks/reports/development/ppml-coefficient-schur-local-2026-09-23/README.md)
records the full-normal Krylov and coefficient-acceleration investigations,
Schur reduction, independent WLS and explicit Poisson/Hessian checks, operation
counts, public rotations, exclusions, memory costs and remaining losses.
The rejected residual extrapolation from ADR 0120 is not resumed. Its costly
fresh speculative certificates do not apply to this coefficient solver's
reconstructed final acceptance procedure.

This kernel also solves the mathematical multipartition OLS projection problem.
Public OLS has a distinct one-shot ownership/admission/routing boundary; it is
not silently rerouted by this PPML change. Existing saved-effect recovery and
retained coefficient products remain in place. This decision does not repair
constrained-fit failures, generic nonretaining-CG span instability, comparator
accuracy discrepancies, or qualify any platform or release.
