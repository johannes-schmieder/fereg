# ADR 0186: profiled slope coordinates for cold multi-FE Poisson starts

Status: accepted for local development, 2026-09-25.

## Decision

For cold publication PPML with at least two FE partitions, each containing an
intercept, extend ADR 0161's initial guess to absorbed slopes. First finish one
exact conditional-intercept sweep over all partitions. Then, in declared
partition/group/channel order, make one safeguarded Newton proposal per slope,
profiling its group intercept exactly. Finite coefficients remain zero. There
is no new public option, sweep count, solver, topology guard or allocated bank.
One FE, slope-only partitions, explicit starts and Mu exploration retain their
previous initialization. Categorical fits retain their existing intercept sweep.

This supersedes the slope exclusion in ADR 0161 and the global-only start in
ADR 0016 for this scope. It does not change their scientific acceptance rules.

## Local mathematics and safeguards

For a group, let `M = sum(q*y)`, `A = sum(q*mu)`, and scale the current slope
to `z` with maximum absolute value one. With `zbar = sum(q*mu*z)/A`, the
intercept-profiled slope score and curvature are

    score = sum(q*y*z) - M*zbar
    curvature = (M/A) * sum(q*mu*(z-zbar)^2).

Propose `step = score/curvature`. The exact conditional intercept correction is

    shift = log(M) - log(sum(q*mu*exp(step*z))).

Evaluate that sum with a log shift. The group Poisson log-likelihood difference,
including an intercept that was moved by an earlier partition, is

    step*sum(q*y*z) + M*shift - (M-A).

Accept a provisional coordinate only when that difference is finite and
nonnegative and all updated predictors/means satisfy the existing bounds.
Otherwise halve the proposal within the existing maximum-halving budget. An
exhausted coordinate leaves the prior guess intact. An exactly constant slope
or zero curvature proposes a zero slope step but still profiles the intercept;
this is not an identified-rank decision. Nonfinite moment/step arithmetic
declines the entire provisional seed and restores the complete global start.
Initial intercept sums retain ADR 0161's compensated raw arithmetic and refusal
rules. Scale normalization, compensated moments and exact constant detection
avoid using a rounded tiny variance as a meaningful constant-slope direction.

Finish the complete intercept sweep before slope proposals. Interleaving them
made a slope explain another partition's intercept variation and failed the
prescribed separable-optimum seed oracle. That draft is not adopted. Subsequent
tests exposed missing intercept correction on a constant slope; that draft is
also preserved and replaced. No acceptance tolerance was adjusted.

Every update adds a declared group intercept and slope, so the offset-aware
predictor remains in the declared model space. These are initial guesses only:
no accepted IRLS state, rank certificate or final-Hessian authority exists yet.
The first full-width canonical solve still freezes the finite basis. All
ordinary current-weight solves, original finite/FE checks, objective line search,
two consecutive independent score/deviance passes, strict witness, canonical
rank/bread, synchronized final Hessian, reconstruction, inference and transactional
publication remain mandatory at unchanged tolerances. Prior weights and the
provisional predictor are read afresh; no geometry or solution crosses frames.

## Ownership and scope

The seed uses finalized membership, original slopes, prior weights and the
already admitted predictor. All new moments, sums and trial values are scalar
stack state. There is no temporary design, per-group factor, observation bank,
cache or worker team. Computation stays in the safe Rust core; the Stata control
plane, main-thread SPI and FFI/publication boundaries are unchanged.

The actual compact-route fast/low T1/T4 allocation windows remain zero-events
and zero-growth. Exact tracked peaks and scratch reports in all 68 accepted-
candidate headline pairs support the no-added-bank result; selected-memory
telemetry differs by at most 48 bytes. This is not physical-cap/RSS qualification.

## Evidence, rejection and remaining work

The [source-bound report](../../benchmarks/reports/development/ppml-profiled-slope-start-local-2026-09-25/README.md)
preserves both candidates, failed drafts, independent analytic and licensed
Poisson/Hessian oracles, matched rotations, fresh cases and excluded controls.
The intercept-only candidate does not address the principal two-slope loss and
is not retained separately. The adopted two-stage seed removes one or two outer
iterations in the fresh slope cases, improving their medians 2.5--11.4%, with
38 wins and two ties in 40 native slope pairs. Eight of ten fresh PPML medians
clear 0.90 times comparable pinned stock. No universal speed claim follows.

The final prepared-IRLS test run has 53 passes and one reproduced baseline
assertion failure: the dynamic-range test expects one strict rejection but
observes zero in both artifacts. This existing failure is not repaired or
hidden. All new tests and all measured public scientific states pass.

Threaded two-slope PPML still takes 102 ms versus stock 83 ms; narrow categorical
PPML takes 39 versus 32 ms. Actual outer traces show substantial nonlinear score
error before convergence, so matching the staged tolerance schedule does not
establish that the schedule causes the remaining iterations. Next investigate
ordinary nonlinear coefficient corrections and the residual canonical cost,
without another sweep-count or benchmark-threshold tuning cycle. Frozen
definitions, static allocation drift, constrained-fit/CG failures and deferred
platform checks remain unchanged. The broader goal remains active.
