# Deterministic remaining-cost policy for two-FE OLS Auto

This successor develops against the accepted ninther setup at native source
`1cdb225e2460e391ad38f856755ae3ea5efb2e5b` with CMG
`fd77d98da1f74009b3e5b634e1944433b647a9b1`. Johannes accepted that setup with its
reported timing uncertainties and authorized policy development on 2026-10-21.
This document records the first development formulation. The fixed
calibration completed on 2026-10-22 and produced a valid encoded fit, but no
builds among its 150 supported intervention observations. The coefficients
remain evidence only; the runtime calibration is absent and the public default
is disabled. See the [calibration assessment](VALIDATION.md#fixed-calibration-outcome-2026-10-22).

## Revision objective (2026-10-22)

Johannes authorized revising this formulation to minimize complete-command
runtime. Conservatism is not a separate objective. The revision must assess
both harmful upgrades and profitable upgrades missed through deferral,
including setup, restart, actual application and reuse costs, and monitoring
overhead. The first formulation's empirical margins, global cost envelopes,
support bins and forecasts are open to replacement; they are not scientific
safeguards or uncertainty guarantees.

Independent Pro and Fable review precedes selection of one revised formulation.
Reuse all valid calibration observations as development evidence, retaining
missing labels and failures. Freeze the revised model and fitting procedure
before evaluation, keep held-out data separate, and do not search until a pass.
The scope, scientific/resource/transaction safeguards and separate 42-cell
qualification and exact public-versus-hidden activation gates remain in force.
The revised [joint two-gate formulation](two_fe_auto_gates.md) replaces this
price model for the next hidden candidate. The remainder of this document
describes the completed first formulation and its retained evidence. No revised
runtime policy has yet been installed.

## Scope and independent witnesses

Only the new hidden `routing_two_remaining_v1` invocation may change the
upgrade decision for OLS Auto/Auto with exactly two categorical intercept FEs.
Within that family, numerical results and internal solver trajectories may
change within the existing scientific tolerances. Sample, rank, omissions,
factor slots/base flags, public solver-signature gates, tolerances and
certification remain unchanged. Outside the family, scientific bits and solver
signatures remain exact. Historical injections retain their contracts.

The existing direct selection, cutoff 5, one sampled tree, seed zero,
eligibility, memory controls, host team, restart, total iteration budget,
correction and MAP continuation remain authoritative. A successful admitted
build installs; there is no application-cost refusal. Deferral does not consume
the sole optional opportunity for the current accuracy key. A typed constructor
memory refusal does consume it; cancellation and fatal errors propagate.

Before qualification, independent dense-QR, analytic projection and bounded
Schur references cover affected results. Real integration witnesses cover Low
chunks, zero-work actions, final residuals, correction, lane retirement and
reactivation, fixed-worker repeats, cross-worker science, portable execution,
public memory controls, cancellation, joined workers and transaction rollback.
Mutations must distinguish original-width startup, stale correction history,
attempt consumption on deferral, omitted workers, positive-only future gains
and a reset action budget. Old-arm and outside-scope replays remain exact.

## Selected formulation and review

GPT Pro and Claude Fable 5.1 independently inspected the complete supplied
source and measurement packet, then read each other's complete first proposal.
The selected formulation is GPT Pro's revised remaining-cost comparison. Two
formulations were proposed; the revisions below do not introduce a third or
search cadence/coefficients against results.

The packet SHA-256 is
`8bca6cdd03011091a2c3d2dfdf361e2b716139ffbcadd88a434f09c277bd390a`
(936 source/evidence files). The original Pro response SHA-256 is
`5050345ba9b0b8178a96f25d25f2c10e02b0196e73c4f7c4e7b69e6ea30f972f`;
its revised specification is
`b4b0baeb53abfc986bf9181acb28dafd595764cec36edcc20a92fc74be0f408c`.
Fable's original substantive proposal SHA-256 is
`f86563d9620b5ad75ddced5ae99da6ca426be8f2a224912241f7a5356a85cfa6`;
its original-plus-revision record is
`fa2b4499cf7ec0660079cf3b935aefe1d6ffd42b309b09d80a02aad3f6a423e5`.
These identities bind the reviews; the substantive decisions are retained here.

Fable's initial exclusions of workers, post-build economic rejection and reuse
of correction rates conflicted with the source/authority. Its revision removed
those conflicts, but still transferred current remaining work to future full
actions, underpriced reactivated lanes and confused requested with admitted
workers. Pro's first proposal had too many forecast coefficients; its revision
uses nine pooled constants. Neither proposal establishes a convergence bound.

Independent high-effort and bounded Astra review cleared the revised
architecture subject to the explicit corrections here: caller-owned state,
disjoint timing labels, matched pre-intervention states, fixed support rules,
nonoverlapping derived random streams, successful zero-work history and accurate
classification of imported saved-effects fixtures.

## Inputs, progress and ownership

Use retained observations `n`, eliminated levels `p`, retained levels `q` and
the already stored maximum membership `d` across both effects. No policy-only
graph census, factor walk or constructor is allowed. Use original current RHS
width `r`, allocated stride `s`, live count `a` and actual admitted host workers
`W`; absence of an admitted fereg team means `W=1`.

Caller-owned demand describes the actual immutable OLS session. Fast's packed
response/regressors have only the final residual left. Low's outcome has every
generated regressor left; a block ending at `end` has `G-end` columns left.
Final residual has no later work. Aggregate full blocks, one short tail and
the final call in at most three width groups. One-off/ephemeral projections
receive no invented retained reuse.

Keep successor state outside `ReusableCategoricalMapAbsorber` and its embedded
route: their sizes enter old-path memory admission. Pass a caller-owned sidecar
through the projection boundary. Its bounded history/control shell is available
at action entry and its complete size is charged only for the successor, so
initially certified zero-step actions can commit history. Admit the two
log-proxy banks only after preserved direct selection leaves monitoring eligible.
Their payload is 16 bytes per RHS, with actual capacities charged. Release old
observation storage before a new key's direct
preflight and before construction; retain the observed peak and account for any
control shell still live during construction. A consumed
opportunity never reallocates observation banks in that key. Insufficient
optional observation admission defers; actual allocation failure remains fatal.

Observe at recurrence-local ages 0, 8, 16, ... after ordinary readiness
retirement. First decision is local age 16, then every eight steps. For each
live RHS, read the exact weighted group-moment maximum used by readiness,
including its zero-reference convention. The full original-design certificate
remains the sole acceptance authority. Poll cancellation after at most 1024
inspected entries. Do not assume a full maximum costs the same as an early-exit
readiness check.

For three positive finite proxies `g0,g1,g2`, separated by eight steps, use
`v=max((log(g0)-log(g1))/8,(log(g1)-log(g2))/8)` and
`L=log(g2)-log(target)`. If `v>0`, set each live horizon to
`clip(ceil(L/v),1,remaining_pcg_steps)`. If both slopes are nonpositive and
finite, use the remaining PCG allowance as a heuristic stagnation forecast.
Let `H` be the maximum live horizon and `D` their sum. Retired lanes contribute
zero. Negative activity sentinels remain live but make the decision Unknown.
Invalid proxies, target, histories, nonfinite values, `L<=0` on a live lane,
or overflow also mean Unknown and Defer. No infinity creates build credit.

Correction resets the local age and banks after the fresh initialization.
The preinitialization correction hook always defers. Preserve cumulative
iterations, original reference norms, reserve and attempt ledger.

Commit completed-action history once at the successful caller boundary,
including initially certified zero-step actions. Count executed shared PCG
steps plus actual subsequent MAP steps; count live RHS before each PCG step
and original width per MAP step. Errors never commit history. Compatible
history requires the same retained session, accuracy key, regressor role and
original block width. Current-action work is never in its own history.

## Cost calculation

Let `ell(x)` be the integer bit length of nonnegative `x`, equal to
`ceil(log2(1+x))`. Use checked arithmetic:

```
O = 10*n*(r+1) + 4*p*r
V = 64*q*r + (p+3*q)*s + 8*p + 12*q + 64*r
Z = q + 4*n
S = Z*(1+ell(Z))
P = n*(1+ell(d))
J = i_s*V + i_p*O/W
I = H*J
B = b_s*S + b_p*P/W
A = a_0*Z
R = r_0*(O+V)
C = t_0*(J+r*A) + t_1*(H*J+A*D)
```

`O+V` matches the existing iteration-work convention. Its decomposition is a
price predictor, not a serial/parallel work identity. `Z/S/P` do not bound
hierarchy fill, time or construction work. The nine nonnegative constants are
`i_s,i_p,b_s,b_p,a_0,r_0,t_0,t_1,kappa_F`, pooled across Mac and Linux.
Auto application remains serial, so `A` has no worker divisor. `C` predicts
complete post-startup continuation including later reactivation, correction,
MAP and certification. Startup is charged with original `r`, never `a` or `s`.

For a future block with supported compatible history, forecast its full shared
steps `Hf=ceil(sum_steps/action_count)` and live exposure
`Df=ceil(width*sum_live_exposure/sum_original_width)`. Compute its `Jf,Cf` with
the actual stride and set `df=Hf*Jf-(width*A+Cf)`. A different-width tail,
absent compatible history or final residual gets `df=-width*A`, with no
positive advance credit. Complete zero-work histories are valid observations.

Sum each group's multiplicity times `min(df,0)+floor(max(df,0)/2)` and subtract
`kappa_F*A*Nfuture`. **Nfuture counts scheduled actions**, including full-block
multiplicity, a nonempty short tail and the final call; it is not padded width
or column count. Positive history credit requires its own calibrated support.
Unsupported required future support makes the decision Unknown, including
when the current-action estimate alone would favor a build.

Build exactly when `I-C+G > 5*(B+R+r*A)/4`. Equality defers. Sunk work and any
benefit from avoiding the new observer's own overhead are excluded. Once Build
is selected, the price never becomes a work limit, timeout or rejection gate.

### Encoded arithmetic

Coefficients are unsigned Q32.32. Prices are integer nanosecond-equivalent
units, computed with checked `u128` intermediates; signed differences use
checked `i128`. Lower coefficients round down, upper coefficients/reserve up.
For a coefficient `c` and integer exposure `x`, `down(c,x)=floor(c*x/2^32)`
and `up(c,x)=ceil(c*x/2^32)`. Avoid adding a rounding bias that itself overflows.

Evaluate `J=down(i_s,V)+floor(down(i_p,O)/W)`;
`B=up(b_s,S)+ceil(up(b_p,P)/W)`; `A=up(a_0,Z)`;
`R=up(r_0,O+V)`; `C=up(t_0,J+r*A)+up(t_1,H*J+A*D)`.
The reserve is `up(kappa_F,A*Nfuture)`. Horizons/history averages round upward;
positive future gain is halved downward, negative gain is retained in full.
Compare `4*(I-C+G)>5*(B+R+r*A)` with checked signed arithmetic. No saturation.
Any unrepresentable intermediate is Unknown. Offline fitting and runtime must
use this same evaluator and expression order.

## Frozen development protocol

No measurement starts before source, diagnostic artifacts, independent
references, harness, inputs, budgets and label matching are bound. The eight
generator families are `dense`, `weak`, `high-degree-weak`, `repeated-cells`,
`disconnected`, `weight-dynamic`, `wide-rhs`, `inactive-columns`.

Calibration data seeds are **2026102201 and 2026103201**; development validation
is **2026104201**; final qualification generated data is **2026105201**.
Order seeds remain 2026102204/2026102205 for calibration/development validation.
The spaced data seeds correct the proposed consecutive-seed overlap:
disconnected fixtures use seed and seed+1; other existing transformations use
seed+1 or seed+17. Bind those derived streams and actual body hashes before
execution. A collision stops preparation; do not search replacement seeds.

Calibration: 16 inputs × Fast/Low × W1/4/8 × one Mac/one isolated Linux host ×
never/force-local16/force-local32 × one retained warmup plus two recorded
mirrored executions = **1728 calls maximum**. Development validation: eight
new inputs × Fast/Low × W1/4/8 × Mac/Linux × original-public/accepted-public/
frozen-candidate × the same three executions = **864 calls maximum**.
Total **2592**, with no sampling ladder. Forced controls use the existing sole
opportunity and preserve direct consumption, early completion and installed
reuse; they are diagnostic interventions, not alternative formulations.
The calibration runner uses one mirrored block (two recorded executions per
arm), a 120-second native-call timeout and a 14,400-second total budget per
platform. It has no continuation, retry or replacement-call path. These are
external execution budgets, never native routing inputs.

### Labels and fitting

Match repeated labels by input, platform, workers, memory mode, action/column
range, accuracy, recurrence epoch and local checkpoint. Current counterfactual
labels require identical pre-intervention scientific state; iteration alone is
insufficient. Retain deterministic state fingerprints, not raw level arrays.
An intervention's suffix starts after that same action and includes every
remaining scheduled call. Its prediction uses only information available at
the intervention, never realized future difficulty. Retain missing, censored,
refused and failed labels with reasons, without replacement observations.
Within each fixed-worker repeat, accounted memory, iteration work, routes and
non-timing policy labels stay exact. The effective memory limit from the host's
available-memory census is an observation: retain it and check each call's
accounted use against its own limit, without requiring equal census values.
Suffix cost sums the later complete projection-action intervals; intervening
QR, regressor generation and other caller work is not credited to either
projector. The state fingerprint is a source/toolchain-bound 64-bit accidental
drift witness, combined with complete input and action bindings; it is not a
cryptographic or cross-platform equality proof.

Disjoint measured intervals are: `B`, the whole constructor; `R`, handoff
reconstruction plus the first fresh initialization excluding startup apply;
startup apply, the complete gather/centering/zeroing/cycle/scatter batch;
`C`, execution after that complete initialization until current-action return.
Post-startup products and readiness belong to `R`. Fit scalar `A` using only
the complete first startup batch interval with exposure `r*Z`; do not fit a
whole-batch interval against `Z`, and do not mix in steady-state application
labels. Original `r` includes zero/component-constant columns: their cheaper
observed batch is retained without pretending they executed a cycle.
Later correction/init,
MAP and certificates belong to `C`. `I` is the matched never-build remaining
action interval. Exclude new observer/trace time from economic labels. Timers
do not affect routing. Existing cumulative Schur phase sums are insufficient.
Ordinary trace formatting is suppressed on the executing projection thread
during economic capture. Explicit checkpoint, constructor-outcome/hierarchy and
action-resource records are emitted outside their measured intervals. New
observation allocation, scans, history counters and label formatting are
excluded; their actual production overhead remains in whole-call qualification.

Fit in order `I`, then `B/A/R`, then `C`, then the future reserve. Each recorded
pair supplies its arithmetic mean; durations are integer nanoseconds and
offline calculations rational. Each input×mode×worker×platform cell has equal
objective weight, divided among its labels, with every row constraint retained.
For an upper component minimize weighted predicted cost subject to nonnegative
coefficients and prediction at least 1.1 times each label. For `I`, maximize
subject to prediction at most 0.9 times each label. Enumerate the origin,
feasible axis intersections and feasible pairwise intersections; break equal
objectives lexicographically in the coefficient order above. Require full rank
of the actual feature matrix; `C` uses already encoded `J/A` prices. Quantize
once and recheck all inequalities with the encoded evaluator. No ridge,
feature-dropping, parameter search or refitting ladder.

Fit `kappa_F` as the nonnegative maximum of
`(G_raw - observed_suffix_gain + 0.1*abs(observed_suffix_gain))/(A*Nfuture)`
over matched complete suffixes; quantize upward and check the encoded gain.
No-future rows need no reserve. Required `A<=0`, failed rank, missing labels
or failed encoded inequalities leave that component unsupported.

Freeze support as follows, using no family names or size fences. The current
support key is `(role, provided_weights, component_scaling, recurrence,
progress)` where role is packed/outcome/regressor/final, recurrence is
initial/correction, and progress is decreasing (both slopes positive), mixed
(only one positive), or stagnant (neither positive). Every live RHS must be
supported; mixed RHS classes require each class. Each supported key needs
matched successful I/B/A/R/C labels from both calibration seeds and both
platforms and all three W1/4/8 settings in each seed/platform combination
(12 combinations per key). Rank is global, not a
per-key coefficient fit. Future support adds history mode (none, compatible,
mixed-width) and requires complete matched suffixes in the same seed/platform/
worker coverage. Positive reuse requires actual first-install examples after
completed compatible regressor history. A runtime key without that support
defers. Zero histories are valid; zero-exposure fit rows are retained but do
not manufacture matrix rank. Report support and build/defer incidence even
if conservative pooling produces no builds.

Development validation cannot retune coefficients or cadence. Its purpose is
to assess the frozen implementation and forecasts before the fixed final
qualification. Empirical envelopes and 10%/25% margins are not confidence
intervals, convergence certificates or unseen-input bounds.

## Qualification and limits

Preserve the existing 42 cells, original uninjected `f35f3a4d` and accepted
uninjected `1cdb225e`/CMG`fd77`, max(5%,1 ms), Linux 12 physical hosts per cell,
three shards/two blocks, and Mac 24 blocks. Use the existing finite externally
evidenced interference recovery. Bind actual binary/request aliases and exact
public/hidden candidate equivalence. Imported fixtures are shared regression
witnesses, not fresh seeded draws: inspect actual scope, especially two-FE
`low-saved`, before deciding exact versus affected scientific comparison.

Public activation remains conditional on all science, resource, cancellation,
transaction, repeatability and performance gates. An inconclusive qualification,
scientific failure or substantive rule conflict goes back to Johannes.
Unfavorable results do not authorize more samples. Full proxy scans can cost
more than any saved work; similar dimensions can conceal very different CMG
costs; recent progress and future RHS transfer can mispredict; cross-platform
pooling can suppress useful builds. These are development risks to measure,
not claims already established by review.
