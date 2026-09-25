# ADR 0187: profiled slope trials for two-partition Poisson fits

Status: accepted for local development, 2026-09-25.

## Decision

Extend ADR 0174's optional publication line-search curve to exactly two FE
partitions with intercepts and absorbed slopes. Select the slope-containing
partition with most groups; ties retain design order. Its exact intercept
correction is followed by one safeguarded intercept-profiled Newton update per
declared slope. Keep the certified current-weight Newton direction and finite
coefficient interpolation. The resulting correction belongs to the original FE
model space. No public option, solver family or acceptance tolerance changes.

The categorical two-partition path keeps its existing partition selection and
arithmetic. One FE, three-plus FE, slope-only partitions and Mu exploration keep
their prior routes. Admission depends on this mathematical scope and the existing
memory budget, not benchmark names, seeds, row counts or tuned thresholds.

The initial broad candidate also corrected three-plus-FE fits. In its six-case
screen, three-FE fits retained four outer iterations and became 1.1–3.4% slower.
Two-partition fits removed two iterations and improved 9.1% threaded and 11.9%
serial. Limit adoption to that demonstrated two-partition boundary; the broader
candidate and all its losses remain evidence. This is not a claim that nonlinear
corrections cannot help higher-dimensional designs.

## Mathematical work and ownership

For the selected partition, retain log prior weights, log group outcome masses,
and each group's slope scale and normalized weighted outcome moment. These
quantities belong only to the immutable prepared sample and original prior
weights. They are not changing Hessian weights. Constructor refusal preserves
the existing route, including zero group outcome or unrepresentable arithmetic.

Normalize each slope to z by its group maximum absolute value. An exactly
constant slope has no profiled direction. Let M=sum(q*y), A=sum(q*mu),
zbar=sum(q*mu*z)/A and target=sum(q*y*z)/M. The normalized Newton proposal is

    variance = sum(q*mu*(z-zbar)^2)/A
    step = (target-zbar)/variance
    shift = log(M)-log(sum(q*mu*exp(step*z))).

Recompute all current-mean moments, variance and proposed mass on every trial.
Use compensated sums and log scaling; normalization by M cancels arbitrary prior
weight scaling. The normalized group log-likelihood change is

    step*target + shift - (1-A/M).

A finite, nonnegative change and the existing predictor/mean bounds permit a
coordinate update. Otherwise halve within the existing maximum-halving budget.
A failed optional coordinate leaves the exact intercept correction intact.
No rank conclusion follows from a constant/zero-curvature coordinate. The full
trial still passes the existing freshly evaluated objective and bounds before
private trial buffers can replace the accepted state. No clipping or relaxed
objective slack is introduced.

Group proposals and row updates use the existing installed team. Only owned
Rust data reaches workers; SPI and transactional publication boundaries stay
unchanged. For G selected groups and S slopes, the additional numerical payload
beyond ADR 0174 is 8*G*(2S+1) bytes when S>0: scales, outcome targets and one
reused step bank. Three Vec descriptors add to the owner size. Full admission is

    sizeof(owner) + 2*sizeof(usize)*K + 8*(N + G*(2+2S+I[S>0])).

Allocations are fallible, exact-capacity checked and charged beside the existing
solver/checkpoints. There is no N-by-K copy or retained changing-weight geometry.

## Independent acceptance and recovery

Retain ADR 0174's complete canonical strict checkpoint and exact-state final
Hessian reuse boundary. The original current-weight projection, finite solve,
model-space/reconstruction, canonical rank/bread, objective, consecutive score
passes and final score remain mandatory. The nonlinear curve cannot certify
any of them. Same-state products require exact q*mu identity and generation;
failed reuse runs the established final-Hessian synchronization.

An initial strict disconnected test at 1e-12 and component weight disparity
1e13 exposed persistent predictor disagreement despite a tiny score. Therefore
any rejected strict slope checkpoint permanently revokes the optional curve for
that fit before continuing the existing refinement/rebuild recovery. No threshold
changes. This is a numerical fallback to the established direction path, not a
route fitted to disparity or topology labels. The corrected test passes under
both thread settings and injected final-reuse refusals. The joint-witness test
explicitly disables the optional curve so it continues exercising that retained
fallback rather than merely counting a bypassed operation.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-profiled-slope-trials-local-2026-09-25/README.md) records both complete candidates, the failed draft,
independent analytic slope and extreme-scale tests, licensed Poisson/Hessian and
cluster/weight oracles, fresh rotations, excluded controls and real allocation
windows. Known baseline test failures and deferred scientific/platform issues
remain visible. This decision is local development, not qualification or completion
of the broad ten-percent-over-stock goal.
