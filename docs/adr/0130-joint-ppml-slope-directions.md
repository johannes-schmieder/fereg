# ADR 0130: joint finite and absorbed-slope PPML directions

Status: accepted for local development, 2026-09-23.

## Decision

For admitted fast multipartition slope sessions in the publication MAP family,
compute ordinary IRLS directions with one joint finite/FE solve. Keep canonical
initial rank, strict transactions and final-Hessian construction. This extends
the coefficient Schur owner of ADRs 0123, 0124 and 0126; it does not introduce
another FE topology, QR representation, rank policy or public solver family.
Budget/allocation refusal retains the existing batched path. One effect,
categorical PPML, low memory, mu separation, explicit non-MAP families and
public OLS retain their existing routes. No benchmark-specific threshold is used.

The frozen experiment rejected treating all controls as another generic FE
partition: global QR/recovery and lost column parallelism consumed its gains.
Use a direct finite block instead. With current weighted local bases Q0 and B,
let P0 = I - Q0 Q0', Z = P0 sqrt(W) X, S = B' P0 B, G = B' Z and H = Z' Z.
Solve the joint system

    [ S  G ] [ a ] = [ B' P0 sqrt(W) y ]
    [ G' H ] [ b ]   [ Z'    sqrt(W) y ]

by block-preconditioned conjugate gradients. FE coordinates use their local QR
scaling and the finite block uses a Cholesky preconditioner. This factor is not
an inverse, a rank decision or a publishable Hessian. A deficient/nonfinite
preconditioner or failed current-weight certification invokes canonical retry;
domain, allocation and interruption failures retain their fail-closed behavior.
Prepared FE coupling remains optional. Cancellation-sensitive curvature falls
back to an observation-space operator and squared norm.

## Verification and ownership

The recurrence only schedules reconstruction. Reconstruct from the original
response/control representatives, current coefficients and current weighted FE
bases. Check finite moments and every FE basis, then run the full independent
fresh FE certificate: residual norms, raw moments and rank-aware span checks.
The session independently scans original generated controls for stable weighted
norms, compensated finite scores and current means. Combined predictor
reconstruction remains a separate acceptance condition. All tolerances and
iteration caps are unchanged.

An ordinary direction does not consume an inverse or within-design Hessian.
Its distinct result type therefore carries no bread or within-inverse authority.
This supersedes the requirement to construct/check those unused products around
every ordinary step on this admitted path, not their mathematical purpose at
canonical boundaries. Initial canonical rank establishes the finite coordinate
basis; every proposed direction satisfies the current-weight joint normal
equations. Strict acceptance independently rebuilds the canonical within design,
rank, finite solve and inverse certificates. Final score, objective, model-space,
reconstruction, final-Hessian and inference conditions remain mandatory.
An ordinary result cannot enter the final inference interface.

Borrow retained controls as FE-equivalent representatives; retain the response
representative plus the change in working response. Joint residuals include
finite projection and must never replace this response representative. Advance
its original-response anchor before fallible solving, so a canonical retry cannot
apply the same response difference twice. Ordinary joint solving does not mark
retained within-design generations current. Strict commit/rollback and subsequent
ordinary solves must work with whichever certified representative is retained.

Only FE geometry/coupling may reuse explicit sample/topology/weight generation
authority. Clear that authority before validation and restore it after fresh
acceptance; outer finite-score failure revokes it too. Recompute finite scales,
projected controls, cross-products, preconditioner and RHS for every call. A prior
weight frame never supplies current acceptance. Rank/basis identity is checked
against retained stored-column positions before using those representatives.

## Memory and execution

Reuse the existing normalized N-by-RHS column bank for the response and Z, the
existing square-root weights, and an FE worker's observation scratch. Borrow
retained controls. There is no additional N-by-K design or FE-owner copy.
For P remaining FE coordinates, K admitted controls and q eliminated channels,
the optional payload is

    8 * [P*K + 2*K*K + (q+2)*K + 5*(P+K) + N] bytes,

plus the joint owner descriptor and its option in the existing owner. Checked
sizes, fallible payload reservations, actual capacity fingerprints and combined
fit-lifetime admission cover this storage. Refusal cannot evict an affordable
existing solver. The owner is released with the linear session before inference.
The actual-plugin audit verifies zero allocator events and zero current growth
through its 16-iteration warmed window, at one and four threads; stable capacity
alone is not its acceptance criterion.

Worker operations use Rust-owned data on the existing team. SPI, safe-core/FFI,
transactional publication and main-thread interruption boundaries are unchanged.
The MAP-family iteration report counts the joint Krylov steps for this ordinary
call; canonical absorption still reports its maximum per-column Krylov count.
Neither count is comparable to a symmetric MAP sweep or a fixest iteration.

OLS must still construct the projected finite design and bread for its one-shot
fit/inference. This direction-only optimization does not remove that work and
is not an OLS routing justification. The common FE Schur mechanism remains
available; focused OLS controls protect the existing route.

## Evidence and limits

The [production development report](../../benchmarks/reports/development/ppml-joint-slope-production-local-2026-09-23/README.md)
binds the adopted artifact, independent analytic WLS and explicit Poisson/Hessian
oracles, frame/rollback/error boundaries, allocation audit, public rotations,
fresh cases, excluded paths and memory measurements. Its predecessor frozen
report preserves both candidate organizations and isolated stock comparisons.

This is a bounded improvement, not completion of the public 0.90-times-stock
target. Exact-state strict/final construction remains substantial work. Static
allocation drift, constrained-fit/CG failures, unresolved comparator families
and deferred platform qualification are not repaired by this decision.
