# ADR 0190: certify canonical PPML residuals before corrective absorption

Status: accepted for local development, 2026-09-25. Not qualification.

## Decision

At canonical retained rank construction, try the existing full current-weight
FE certificate on the finite residual assembled from projected response/control
columns. If it passes, return those residual rows unchanged with a certified
zero-iteration report. If it refuses, continue through the existing coefficient
solve, reconstruction and full final certificate. Finite residual combinations
need not inherit their inputs' tolerance, so the fresh check is mandatory.

Admit this entry only for the PPML coefficient owner without an alternate
categorical canonical projector, one residual RHS and already authorized
sample/topology/weight geometry. The generation covers both local QR geometry
and the optional coupling operator. A fresh generation must run the existing
solve even if an input certificate could pass: preparing QR alone must not
publish authority for an unprepared operator. The ordinary absorption API and
all other RHS boundaries retain their prior checking schedule.

Take generation authority before any validation or fallible work. Invalid
shape, nonfinite input, invalid configuration, failed solving or unwind leaves
it absent. Restore it only after the returned result has passed independent
certification. Refusal of the preliminary certificate is a request for the
existing correction, not a new fallback solver or an accepted result.

## Scientific purpose

Projection is required to establish that the actual returned residual is
orthogonal to every current-weight FE block at the configured tolerance. A
fresh full rank-aware span and original raw-moment check establishes that
condition without changing an already valid residual. Use the same stable
input norm, fresh residual norm, infinity/RMS diagnostics and tolerance as the
existing final certificate. Never infer acceptance merely from the input
columns' certificates or a coefficient-space recurrence.

No new speculative coefficient solution needs a reconstruction witness on the
early return: the exact input rows are the returned rows. A correction still
receives its original reconstruction and final check. Canonical rank, inverse,
finite and original-input scores, working-response/model-space reconstruction,
outer objective and score, strict comparisons, exact final-Hessian conditions,
sample, weight, DoF, separation and transactional publication remain unchanged.
This revises an unconditional corrective solve, not a scientific threshold.

## Scope, memory and evidence

The private entry shares the existing absorption implementation with a scoped
initial-check flag. There is no new solver, buffer, descriptor, allocation
allowance, numerical cache or automatic routing rule. Other coefficient calls,
alternate categorical projectors, non-MAP routes and public OLS retain their
routes. Eligible four-plus categorical coefficient owners can also use the gate;
the optimization is not defined by benchmark names or slope-only guards.

The [source-bound report](../../benchmarks/reports/development/ppml-residual-gate-local-2026-09-25/README.md)
records analytic independent weighted projection, contaminated/zero residuals,
changing weights, rank-deficient channels, extreme scaling, T1/T4, authority
revocation/recovery, weighted Poisson/Hessian oracles, OLS controls and real
warmed allocation fixtures. All 232 PPML states, 150 baseline comparisons,
68 direct stock comparisons, nine OLS pairs and nine licensed executions pass.
There are 132 distinct focused Rust passes and the known baseline checkpoint-
count assertion failure. Numerical equality uses the established envelopes;
old residual-correction bits are not an acceptance requirement.

Diagnostic residual projection is roughly halved on the primary threaded slope
cases. Fresh public medians improve on five of eight slope cases by 1–3 ms and
lose 1 ms on the other three. Four-FE categorical improves 4 ms; narrow
categorical loses 1 ms. A single diagnostic check on the three slope losses
confirms accepted gates and reduced phase cost, without replacing their headline
results. Retain the inexpensive shared work reduction; do not tune thread/size
or topology guards to rescue individual clocks. Scratch/tracked peaks are
unchanged; selected-byte telemetry varies by -48 to +48 bytes. The allocation
audit covers its established ordinary-iteration window, not every canonical
allocation or physical RSS.

The broader 0.90-times-stock objective remains open. Frozen benchmark definitions,
static allocation drift, constrained-fit/CG failures and deferred platforms are
unchanged. Current implementation priorities belong only in STATUS.md.
