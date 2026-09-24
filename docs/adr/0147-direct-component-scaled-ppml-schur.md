# ADR 0147: direct component-scaled two-FE PPML absorption

Status: accepted for local development, 2026-09-23.

## Decision

An admitted two-FE categorical publication PPML owner enters its existing Schur
PCG engine directly after the initial current-weight certificate. Remove the
mandatory four symmetric MAP warmup sweeps for that owner: the prior route chose
PCG regardless of measured pilot contraction. Keep reconstruction, the fresh
original-weight residual certificate and the existing reserved MAP continuation.
The result reports zero pilot iterations. No public solver family is added.

Use a separate internal PPML constructor. Ordinary OLS construction retains its
prior banks, pilot policy and capped/probed Schur routes. Checkpoint-LSMR selection,
its original-input candidate/retry path, and one-/three-FE and slope algorithms
are unchanged. A checkpoint's existing MAP fallback may use the direct PPML
engine. This supersedes ADR 0146's retained four-sweep policy for the PPML role;
its idle-bank loan still serves initial certification.

## Disconnected components and current weights

Direct entry alone regresses a six-step PPML model with disconnected components
whose working weights differ by roughly thirteen orders of magnitude. Inner
acceptance still passes, but the independent strict mean check exposes a second
profile movement above its unchanged threshold. One finishing sweep and a
centered Schur RHS accumulation do not repair that case. Do not adopt those
probes, add a case-specific warmup guard, or relax outer acceptance.

For each disconnected component c, solve the equivalent system

    (S_c / m_c) a_c = b_c / m_c,

where S is the current-weight two-FE Schur operator and m_c is the largest
current right-group mass in that component. Scale the Jacobi diagonal by the
same m_c. A scalar constant throughout a disconnected block preserves symmetry,
the null space and the fitted projection. It keeps low-mass blocks represented
in the shared CG energy reduction. This is not arbitrary per-group row scaling.

Construct immutable right-level component roots once per PPML owner by joining
right labels that share a left group. Use idle operator storage for union ranks.
Connected designs discard the temporary roots and keep no scaling bank.
Disconnected owners retain roots and per-right-level scales. Every absorption
rebuilds group masses and RHS moments from its current weights, then refreshes
component scales using idle operator scratch. No numerical geometry receives
authority across weight frames, and no observation-sized weight copy is added.

Only recurrence equations, residuals and diagonal preconditioning use this
scaling. The operator first uses original normalized weights; its output is
scaled componentwise. Cheap readiness recovers the original group mean and
retains the original group-mass factor. Reconstruction and the full acceptance
certificate use the original weights. Finite rank, scores, model-space checks,
objective, strict profile, final Hessian, inference, sample, weight semantics,
DoF, separation and transactional publication remain authoritative.

## Ownership and memory

Admit two words per level of the smaller partition: one component root and one
current-weight scale, at most 16 bytes per level on supported 64-bit platforms.
Construction uses already-admitted operator storage for union ranks and later
component maxima. No new RHS-sized matrix, observation vector or worker team is
introduced. The PPML route model and its four exact arithmetic tests include
these banks; other phases may still determine the overall selected peak.

The separate PPML constructor is essential: ordinary OLS does not reserve these
banks, and its existing exact Schur admission must remain sufficient. A prototype
that initialized components in the shared constructor is not adopted. The
ordinary solve borrows the retained graph directly instead of removing it from
its owner, preserving ownership through errors and unwinding. Actual numerical
contents remain scratch until fresh initialization and acceptance.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-direct-two-fe-local-2026-09-23/README.md)
preserves rejected prototypes, independent weighted projection and Poisson/
Hessian/sandwich oracles, changing masses and RHS widths, strict-profile recovery,
threads, exact memory arithmetic, OLS admission and licensed transaction checks.
All thirty fresh target pairs improve, with median reductions about 7–14%.
Dense cases still lose to stock fixest, and small fast/OLS control losses remain
visible. No universal timing non-regression, physical-cap or qualification claim.

The original full initial certificate remains, including its zero-iteration
early return. This decision removes checks attached to an unneeded pilot, not
the fresh certificate of an accepted result. No tolerances or iteration budgets
are loosened. Close this direct-solver experiment rather than tune warmup counts
or guards to its consumed validation seed.
