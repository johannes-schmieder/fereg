# ADR 0196: close current-weight coarse canonical CG experiment

Status: rejected for production, 2026-09-25. Local development, not qualification.

## Decision

Retain e7731129/finalreplay1 runtime. Do not adopt canonicalcoarse1, based on
planning checkpoint a4792e9d with a single canonical_products.rs overlay.
The [source-bound report](../../benchmarks/reports/development/ppml-coarse-cg-closeout-local-2026-09-25/README.md)
preserves the patch, complete candidate source, tests and public measurements.
No fresh validation seed follows the mixed development pilot.

## Candidate and mathematical boundary

The existing grounded, diagonally scaled current-weight Schur system A receives
an additive rank-one inverse preconditioner P*r = r + v*(v'r)/(v'Av).
The free-coordinate diagonal roots, normalized by their maximum within each
component, supply v. This represents the constant original-coordinate direction
that grounding makes slowly varying. Each positive finite energy is computed
once per component and shared across RHSs. Invalid energy disables the optional
correction; numerical recurrence failure retains the existing certified fallback.
No previous-frame factor or matrix is used.

The implementation reuses Q-double assembly totals for the coarse directions
and component energies, charging Q additional doubles conservatively alongside
Krylov storage for their longer lifetime. The matrix-product scratch becomes P*r
after its old value is consumed, preserving three scratch vectors per RHS.
Every phase fits the unchanged allowance. Weighted state dies with the call.

Unpreconditioned Euclidean recurrence residual stopping, the configured iteration
cap, original-input projection certificate/direct Gram, canonical finite rank,
score, reconstruction, objective, exact final q*mu Hessian, inference, samples,
weights and failure atomicity remain unchanged. Affordable Cholesky is unchanged.
The independent original-row certificate remains the acceptance authority.

## Evidence and reason for rejection

All 13 focused Rust tests and six licensed scientific fixtures pass. A new
explicit weighted grounded-graph test recovers known solutions across scalar
extremes with fewer steps. Existing independent nonzero-residual WLS, changed
weights, T1/T4, exact budget and one-byte-below refusal tests remain green.
The actual-plugin ABI/self-test and intentional panic-containment harness pass.

The native trace confirms initial canonical maximum iteration counts falling
15 to 11 and strict counts 23 to 19 on both large low and fast targets. Outer
counts are unchanged. Three public pilot rotations nevertheless improve each
iterative target's median by only 1 ms: low 113 to 112, fast 99 to 98. Low has
one paired win, one loss and one tie; fast has two wins and one loss. Narrow
worsens 39 to 40 despite five fewer reported inner iterations. The excluded slope
control improves 77 to 76 with unchanged iteration counts. These clocks are too
small and mixed to warrant retaining another numerical mechanism.

All 52 public states, 32 baseline-anchored comparisons and 12 direct stock pairs
pass across the pilot, closing trace and prior isolated canonical ledger.
Tracked peak and scratch deltas are zero; selected telemetry varies by -48 bytes.
No physical-cap or allocation qualification claim follows. No tolerances change.
The earlier detailed ledger places canonical RHS solves at 7.426 ms inside an
18.106-ms build, versus 2.257 ms writing controls. Reducing some matvecs therefore
cannot supply the larger complete-command saving required by the current goal.

## Consequences

Do not tune this rank-one correction, its energy scaling or admission thresholds
against these cases. Preserve its evidence and move to repeated current-weight
preparation, reconstruction and verification work across the ordinary/canonical
boundary. The accepted checkpoint schedule and prior rejected matrix-cache and
storage experiments remain closed. Stronger preconditioning is not ruled out in
general, but another candidate must address a measured substantial cost and earn
its full-public complexity. The multi-FE <=0.90-times-stock goal remains active.
