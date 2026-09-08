# Fixed-effect kernel

Status: implemented in the public 0.5.0 OLS path; exact-SHA platform and
performance qualification remains pending.

The safe core provides:

- queue-based singleton deletion to a fixed point;
- multipartite connected-component labeling;
- one-/multiway weighted projections;
- intercept, slope-only, intercept-plus-slope, and multiple-slope local blocks;
- deterministic batched symmetric MAP, genuine weighted CG, and explicit
  `within`/IT/SQUAREM/LSMR support;
- normalized absorbed-sum and saved-FE recovery; and
- fresh basis-invariant weighted FE-span projection certificates.

Each effect stores compressed level-to-observation membership. MAP allocates
and reuses bounded projection workspaces. Rank-deficient local slope blocks use
pivoted elimination; unidentified parameter directions may be normalized, but
fitted projections must remain identified.

A solver recurrence is never sufficient evidence. For each effect-level
block `B`, the final gate computes the projection of `sqrt(W) r` onto
`col(sqrt(W) B)` with scaled, reorthogonalized rank-aware QR and measures it
relative to the outcome/residual scale. Columns are normalized before this
check, making the certificate invariant to absorbed-channel units.

Hitting an iteration limit or failing the fresh certificate is fatal. Solver
threads operate only on Rust-owned storage; no Stata SPI call occurs off the
calling thread. Independent explicit-dummy fits validate small designs,
identified combinations, residuals, absorbed sums, and normalization rules.

## Comparator-equivalent MAP and CG

The comparator-equivalent transform is one complete symmetric Kaczmarz sweep.
Unaccelerated `solver(map)` applies that transform until the independent
certificate passes. `solver(cg)` uses the positive-semidefinite complement
operator `Pi(x) = x - M(x)`, where `M` is the symmetric residual sweep:

1. `r = Pi(y)` and `u = r`;
2. `v = Pi(u)`;
3. for each RHS, `alpha = <r,r>_W / <u,v>_W`;
4. update `y = y - alpha u` and `r = r - alpha v`;
5. use the Fletcher–Reeves ratio for the next search direction.

Outcome and regressor RHSs are processed as one batch but have independent
recurrences. Categorical-only designs borrow finalized codes and membership
directly; they do not copy observation codes merely to use CG. Slope-bearing
effects use the shared owned projection design.

Nonfinite curvature, invalid recurrence values, iteration exhaustion, or an
uncertified terminal value fail closed. A finite nonpositive denominator from
loss of conjugacy restarts that RHS from its true residual; a broken RHS that
already passes its own certificate is deactivated while other RHSs continue.
The recurrence stop matches reghdfe's Hestenes energy rule: each RHS tracks the
recent `alpha * <r,r>_W` improvement relative to its remaining improvement
potential. Publication requires both Hestenes convergence and the joint fresh
FE certificate. A certificate alone cannot terminate an ordinary CG path, and
the recurrence alone can never authorize publication.

Coefficient recovery uses the same recurrence but adds a stricter terminal
condition: `max(abs(target - D alpha)) / max(abs(target)) <= 1e-10` (or the
tolerance-derived stricter value). If the ordinary Hestenes condition fires
first, recovery continues from the current CG state; it does not throw that
work away and start a second solver. This additional scan and stop rule are
used only for saved-effect recovery and do not change ordinary CG estimation.

## Current automatic policy

`solver(auto)` implements the ADR-0021 categorical AKM policy with the ADR-0022
iterative-terminal and ADR-0023 direct-terminal guards, while retaining the
ADR-0009 generic/slope policy:

- categorical intercept-only graphs first select CMG, without a MAP pilot,
  when there are at most nine nuisance Schur columns, at least three
  observations per highest-cardinality level, and at least a 10:1 level-count
  ratio between the two core effects;
- an auto-eligible iterative-terminal hierarchy remains CMG only when its
  deterministic repeat-weighted work is at most four finest-level passes; a
  rejected iterative hierarchy is released before nuisance-factor/PCG work
  and the fit uses direct LSMR;
- an auto-eligible direct terminal uses the locked hierarchy/RHS/thread cost
  guard. It selects direct CG only when the broader cost score rejects CMG,
  depth-work exceeds 200 per observation or nuisance width reaches nine, and
  execution is serial or the initial RHS width fits within the thread width;
  otherwise it retains CMG;
- one FE: direct one-pass demeaning and immediate certification;
- other categorical intercept-only graphs select direct
  Schwarz-preconditioned LSMR without a MAP pilot, with internal tolerance
  `max(1e-14, requested_tolerance * 1e-2)`;
- slope-bearing and generic multiway designs retain at most four symmetric MAP
  pilot sweeps, the 0.008 contraction boundary, and genuine CG for the
  remaining cases; and
- if auto-selected LSMR returns a finite but uncertified candidate, continue
  certified CG from that candidate within the remaining iteration and memory
  budgets. Report all attempted LSMR work; do not restart from the pilot state.

The CMG selector applies in fast and low execution. It remains narrower than
explicit `solver(cmg)`, which continues to
support up to 64 non-base nuisance columns. One hierarchy and workspace pool
are retained across outcome/regressor blocks. Final OLS residuals normally
already satisfy the FE certificate, so they skip graph PCG; the normal
certified CMG path runs only when correction is required. The graph PCG systems
use one-hundredth of the requested FE tolerance because their projection feeds
the more sensitive OLS score certificate. CMG setup failure selects direct
LSMR with a warning; a solve failure marks the fit-lifetime route failed and
uses certified CG thereafter.

Neither planned hierarchy veto is a setup failure or fallback: each emits no
warning, does not set the fallback flag, and reports its actual LSMR or CG used
solver. Graph and hierarchy construction time is nevertheless charged to
solver setup time. Explicit CMG bypasses both automatic guards. The direct-CG
guard is deliberately conservative; it leaves some measured CG wins routed to
CMG rather than adding less-supported threshold clauses.

The report retains requested solver `auto` and records used solver, pilot
iterations, pilot contraction, total iterations, stop reason, setup/solve time,
fallback, and certificate norms. There is no FE-coefficient cap. The prepared
categorical route passes the already finalized codes and memberships directly
to `within`, avoiding the former observation-scale owned-code and membership
copies. Schwarz construction and LSMR reuse those prepared structures.

The direct LSMR configuration followed calibration of requested/tight tolerances,
off/diagonal/Schwarz preconditioners, forced restarted CG refinements of
4/8/16 iterations, and the prior coarse-LSMR/ordinary-CG hybrid. Direct tight
Schwarz-LSMR was the surviving lowest-runtime arm; CG refinement is therefore
not a production stage. The forced-refinement test path bypasses CG's initial
certificate return and proves that the comparison arm really executes its
registered number of restarted iterations.

## Public PPML solver families

`feppml` accepts exactly `solver(auto|map|lsmr|cmg)`. Auto selects LSMR for
exactly two or three categorical intercept effects when fast execution and the
complete ordinary/checkpoint workspace fit the generation-zero memory plan.
Other designs, low-memory execution and allocation audits select MAP. A modeled
workspace veto selects MAP before construction and is not a numerical fallback;
actual allocation errors remain fatal. Auto never selects CMG. This development
policy is admitted under ADR 0029. The PPML request records the requested family, while the
result separately reports the selected family, concrete engine, final effective
route, and checkpoint family. This policy does not alter `fereg` or its OLS
solver session.

One reusable categorical joint-incremental engine operates between canonical
checkpoints. It applies one augmented operator to the Newton correction over
active controls plus categorical FE coefficients and carries a fit-lifetime
correction warm start. A zero-active-control fit uses the same engine while
retaining the public `lsmr_fused` concrete identity; positive-rank fits report
`lsmr_joint`. Two-effect problems retain Schwarz state and rebuild it only when
recurrence or certificate evidence requires that work. Problems with three or
more effects retain one Schwarz core for the two largest retained effect
blocks and a fresh current-weight positive diagonal action for every additional
effect. A matrix-free symmetric block-factor bridge applies the current-weight
cross Gram between that core and the additional effects. Its action is
`T diag(C,D) T'`, where `C` is the retained core action and `D` is the positive
extra-effect diagonal action, so the combined preconditioner remains symmetric
positive semidefinite. The bridge uses finalized memberships directly and
retains no additional pair graph. This covers the full FE axis while avoiding
the former all-pairs Schwarz policy, whose extra factors made the augmented
recurrence dramatically slower. Exact finite and per-level normal equations
still certify the completed recurrence; the preconditioner is never evidence.

The preconditioner and reuse policy are internal implementation choices, not
additional command options. The initial rollout requires at least two
categorical intercept-only effects, the fast memory route, and no allocation
audit. Saved fixed effects are recovered once from the accepted canonical
final-Hessian state without disabling the fast ordinary-frame engine.
`memory(auto)` may select fast; explicit `memory(low)` and
other recognized but unsupported combinations return `r(498)`.

The canonical retained session still owns the first weighted-linear frame and
freezes its active basis; Mu separation remains canonical MAP. Under ADR 0027,
explicit LSMR and CMG fits with two or three categorical effects and fast memory
use a fresh current-weight batch LSMR
projection for the initial response-plus-all-controls frame as well as strict
and final-Hessian checkpoints at every positive retained sample size. The
LSMR candidate is independently re-certified by the canonical session before
the unchanged all-width rank/status decision, bread, score, objective,
reconstruction, and exact `q_i * mu_i` final-Hessian checks run. An uncertified
numerical candidate receives at most one tighter batch on untouched inputs,
with unchanged canonical tolerance and explicitly counted retry work. A second
numerical rejection falls back to MAP on the untouched authoritative inputs;
other errors remain fatal. At a strict checkpoint, the predictor, objective,
score, rank, basis, and all estimable coefficients must agree; an unidentified
normalization coordinate cannot force another IRLS frame, and the canonical
strict state replaces it before publication.

The reusable categorical MAP CG accelerator forms its symmetric `I-S` action
by summing weighted FE group contributions. It does not obtain that action
by subtracting nearly equal observation vectors: cancellation in that
subtraction can create an off-model-space component that a weak-graph CG
recurrence amplifies without failing a normal-equation check. Group means use
the existing scaled compensated reductions, and the action reuses admitted
buffers. Fresh FE certificates, strict and final-Hessian synchronization, and
publication tolerances are unchanged; no extra saved-FE recovery is imposed
on fit-only calls. The ordinary OLS projection retains its existing arithmetic.

The initial public tranche makes no allocation-free, large-scale, or
thread-scaling claim and no automatic-routing claim. The fused engine still
owns per-frame weight and projected-result storage, and either engine can spend
one tighter recurrence pass after an uncertified result. When the selected
policy owns a retained Schwarz preconditioner, it is rebuilt for that pass; the
original publication certificate tolerance is unchanged. The result wire
measures those costs separately from canonical checkpoint work.

The pinned `ppmlhdfe` comparator uses its default conjugate-gradient
acceleration of symmetric Kaczmarz for multiple effects and unaccelerated
projection for one effect; it must not be described generically as
LSMR. The development harness does not force a second comparator route. It
parses the pinned solver's verbose route line and fails if either default
technique changes. If a candidate disagrees scientifically with the timed
default fit, the harness runs one untimed fit from the same pinned closure with
`tolerance(1e-10) itolerance(1e-12)`. That strict fit keeps the same
observed default technique (unaccelerated for one effect). Agreement with the strict fit
adjudicates the stopping discrepancy while retaining the original default-fit
mismatch in the analysis. If both default and strict comparisons still differ
only in finite coefficients, the development harness may run a final untimed
public-state audit. Fresh processes save the public fitted means from MAP and
from both pinned reference fits; a separate Mata calculation recomputes every
control, constant, and FE-level score plus likelihood and deviance from the raw
data. [ADR 0026](adr/0026-ppml-comparator-objective-adjudication.md) makes each
pinned reference fit's reported `e(ll)` and `e(deviance)` the authoritative
cross-estimator objective pair, subject to finite, domain, and Poisson-identity
checks. Fereg's certified objective must be no worse than that pair within the
registered tolerance. The reference public prediction, its replayed objective,
KKT score, and model-space projection remain mandatory diagnostics. A raw
predicted state is an additional objective witness only when independent
checks certify both its control-plus-FE model-space feasibility and its full
control, constant, and FE-level stationarity; fereg must also be no worse than
every such witness. A projected state alone is not a witness. A state that does
not meet the model-space threshold remains indeterminate rather than proven
infeasible, but that indeterminacy no longer invalidates the reported reference
objective.

Fereg remains subject to the stronger self-consistency rule: MAP must satisfy
the public `1e-8` score tolerance, its published mean must replay its reported
objective and lie in the declared model space, and all solver, rank, sample,
and result certificates must pass. Fresh normal residuals, recurrence drift,
the loose-to-final fitted-vector tail, and the Poisson identity
`deviance + 2*ll` remain checked. Sample, VCE, total-DoF, route, fallback,
certificate, or non-objective failures remain fatal. The analysis retains the
reported and replayed reference objectives, all prediction diagnostics, and
all hashed public-state inputs. This development-only audit uses the pinned
comparator's observed default projection route and never forces a different
`ppmlhdfe` solver.

The version-4 development manifest derives explicit-solver eligibility from
the generated dataset metadata before timing. It excludes an unsupported
solver arm instead of treating its documented `r(498)` as a failed benchmark;
the public rejection remains covered separately. For CMG, the runner mirrors
the prepared engine's core choice: the two largest-cardinality effects are the
core graph, and the remaining non-base width must not exceed 64. This envelope
counts third-effect indicators, not the finite regressors K. Version 4 adds
local one-FE, intermediate mixing, checkpoint-boundary, nuisance-width,
finite-width, thread-scaling, large-easy, and held-out-seed profiles. Historical
version-3 manifests and receipts remain unchanged.

Under [ADR 0028](adr/0028-one-effect-ppml-initialization.md), cold one-effect
publication fits begin at the analytic group-intercept optimum for zero finite
slopes when it is representable. This only seeds the existing estimator: rank
freezing, ordinary IRLS, strict convergence, final-Hessian synchronization,
and every independent certificate remain mandatory. Unusable seeds restore
the original global initialization; warm starts and Mu exploration are unchanged.

The public CMG family means only the prepared fused-FWL engine introduced by
ADR-0023 and updated to the exact prepared-topology pin by ADR-0024. It prepares
the two core FE endpoint topology once for the fit. On each ordinary IRLS frame
it validates and assembles the current-weight numeric
graph, builds a fresh hierarchy, projects all frozen active controls and the
working response through caller-owned batch buffers, and skips the generic
observation-space CG polish. It then uses the same standardized pivoted-
Cholesky finite solve as fused FWL and requires fresh per-column projection
certificates plus a complete control-and-FE normal-equation certificate.

The PPML graph batch capacity covers the larger of the response/control width
and the nuisance Schur width, capped by the existing pool size. This lets a
narrow finite design solve independent nuisance directions across the admitted
thread team. Early memory admission fixes that capacity for initial ordinary
engine construction and any strict-basis replacement; current-weight setup,
per-RHS convergence, and every fresh certificate remain mandatory. Private
frame tracing reports the capacity as well as all setup, solve, and retry work.

Prepared PPML submits graph RHSs in batches aligned to the actual admitted
workspace-pool width. The effective width is the largest pool multiple within
the allocated capacity; only the final batch may be partial. This avoids an
underfilled execution wave at every capacity boundary without enlarging the
memory plan or changing any recurrence, reduction order within an RHS, or
certificate. The rule applies to nuisance setup and response/control solves,
including selective retries. Private tracing reports `pcg_batch_width`
separately from `batch_capacity` and retains actual RHS/batch counts. Serial
pools and the OLS owned-result route keep their original batch boundaries.

Prepared PPML retains the current frame's nuisance graph potentials while
forming its Schur factor. The correction for each projected response/control
column uses their linear combination instead of solving that combination a
second time. This is within-frame linear algebra, not cross-frame warm state:
the weights, graph, hierarchy, nuisance solves, and factor are rebuilt together.
Every completed column still passes the fresh full-FE certificate, followed by
the combined finite-and-FE certificate. A tighter nuisance retry rebuilds the
potentials with its factor. Checked admission and exact retained-memory checks
charge core-vertex count times nuisance width; no observation-by-nuisance
matrix is retained. Private tracing separates actual graph RHSs/batches from
nuisance RHSs completed by reuse. Maximum-RHS iteration counts retain their
existing units. OLS keeps its owned-result correction solve and coefficient
recovery path.

Ordinary PPML CMG can seed fresh nuisance-basis solves from the immediately
preceding fully certified frame (ADR-0024's warm-start amendment). The fit-owned
cache contains potentials only. Admission requires finite guesses, normalized
Hessian weights within a factor of two in both directions, no prior tighter
retry or extra restart, and useful freshly measured initial-residual reduction
on any prior seeded solve. Current graph, hierarchy, RHSs, recurrence residuals,
Schur factor, and all full-model certificates are still recomputed. Invalid
eligibility starts cold; failures, retries, committed checkpoints, and basis
replacement invalidate the cache. Promotion consumes the completed absorber
and swaps preflighted buffers only after full certification. Memory charges one
additional core-vertices-by-nuisance array, never an observation-by-nuisance
matrix. `phase=cmg_nuisance` tracing records warm RHS, summed nuisance iterations,
restarts, weight change, initial residual quality, and setup subphase times;
the existing public iteration counts remain maximum-RHS counts. OLS and CMG
designs without an actual nuisance block do not use nuisance guesses.

Frozen active controls have a separate fit-owned warm cache, using the same
finite-guess, factor-two current-weight transition, full-frame certification,
and own-recurrence quality rules. It stores raw core potentials before nuisance
correction, never residuals, in the frozen finite-column order. Each fresh solve
consumes eligibility before overwriting the admitted buffer; only a completely
certified frame without a tighter retry can promote it again. Failure,
checkpoint commit and basis replacement reset both seed families. Early and
retained-frame memory admission charge exactly core vertices times active
controls, with checked retained capacity; K=0 has no control-potential buffer.
The working response remains cold: its warm-start experiment triggered an
expensive three-FE certificate retry. A seeded control batch therefore stops
before the response column, which uses the original cold recurrence. Cold
frames keep the original batching. Every original-FE and combined certificate
is unchanged. `phase=cmg_projection` reports warm RHSs, warm/total summed
iterations, residual recomputations, fresh initial residual quality and
RHS/PCG timings, separately for first and retry attempts. OLS uses neither cache.

Prepared CMG frames with an actual nuisance block may retain the split heavy-edge
forest without low-effective-degree pruning. Eligibility requires every current
canonical edge's weight divided by its weighted degree at either endpoint to
be strictly below CMG's existing 1/8 threshold. An allocation-free current-graph
scan recomputes eligibility each frame; mixed/strong-edge graphs retain default
grouping. This changes only the fresh
preconditioner: it prevents nearly unreduced coarse levels from multiplying
every nuisance RHS's work. The current graph, PCG tolerances/residual checks,
and independent full-FE certificates are unchanged. The existing direct-factor,
depth and fill caps also remain unchanged; both groupings have identical
conservative memory bounds. Two-FE, exactly redundant-nuisance, and OLS routes
keep the default grouping. Setup traces report the effective grouping threshold;
hierarchy dimensions and elapsed time must accompany iteration comparisons.

If exact retained membership proves that the third effect is a function of
either selected core effect, ordinary PPML CMG projects through the two-effect
core alone. This removes an identically zero nuisance Schur block without a
numerical rank decision. The original three-effect model still governs memory
admission, the 64-indicator envelope, canonical rank/DoF/checkpoints, saved
effects, and every fresh per-FE certificate. Private frame tracing records the
one-based redundant effect position (zero when none) and the actual nuisance
solve width. Near-nesting is not sufficient. The topology proof is fit-owned
and is rebuilt with the engine; changing weights never invalidate exact span
containment. OLS is unchanged.

Only failed projection columns receive one tighter retry. If that independent
certificate demonstrates that the recurrence target was insufficient, the
next changing-weight frame begins at the learned tighter scale; the public
certificate tolerance is unchanged. Retry reuses the current hierarchy when
the nuisance solve width is zero; a nonempty nuisance block requires a rebuild
because its Schur factor depends on the original solve tolerance. A remaining
numerical failure runs certified MAP on that frame and disables CMG for the
rest of the fit.
Unsupported topology, invalid input, memory failure, interruption, and internal
errors remain visible failures. Mu frames remain MAP; initial, strict, and
final-Hessian projections follow the certified checkpoint policy above.

CMG has the same initial fast-memory requirement as LSMR but accepts exactly
two or three FE dimensions. Three-FE nuisance width is capped at 64, saved
effects use the same canonical final-Hessian recovery, and allocation audit is
disallowed. Its
retained fereg-owned buffers and the
topology-aware CMG hierarchy are charged before use. Prior-frame preconditioning
remains disabled; only the certified nuisance/control-potential guesses described above
cross ordinary frames. Caller buffers remove
owned per-batch result construction but do not imply an allocation-free IRLS
frame. The OLS CMG construction, polish, and fallback path is unchanged.

Result wire 1.2 publishes four fixed engine rows: MAP, fused LSMR, joint LSMR,
and prepared CMG. Each row repeats the route identities and records its own
ordinary/checkpoint frames, retries, fallback evidence, iterations, timings,
and certificate maxima. Numerical CMG nonconvergence records one visible
fallback, solves the same frame with certified MAP, and leaves the effective
ordinary route as MAP. Numerical checkpoint LSMR nonconvergence is separately
visible and retries the untouched checkpoint with MAP without changing the
ordinary route. A viable route that cannot fit
its checked workspace returns `r(909)`; unsupported topology or feature
combinations return `r(498)`.

## OLS saved fixed-effect recovery

This section describes `fereg` solver products. PPML saved effects are
recovered once from the accepted canonical final-Hessian state as specified
for the public PPML families above.

Saving absorbed effects is treated as an optional solver product, not as a
second estimation problem. In `memory(fast)`, categorical intercept routes use
the cheapest coefficient-space product available:

- LSMR retains the coefficient vectors it already solved for.
- CMG converts its exposed worker--firm PCG potentials to effect coefficients
  and combines them with the nuisance-effect Schur solution. A nonzero CG
  polish invalidates that product and deliberately triggers the fallback.
- CG carries compact FE coefficient vectors through the same alpha/beta
  recurrence as its observation-space directions.
- Irons--Tuck applies every accepted extrapolation to the parallel compact
  coefficient states.
- categorical `solver(auto)` retains the coefficient product of direct LSMR or
  CMG; generic pilot-routed auto adds the MAP pilot product to its selected
  LSMR or CG continuation.
- unaccelerated intercept-only MAP records the group coefficients from its
  original batched `y`/`X` sweeps, then forms
  `alpha_y - sum_j beta_j alpha_Xj` after the finite coefficients are known.

This MAP coefficient accumulation removes the recovery solve completely. The
running product uses the projection kernel's native level-major layout; large
tables add each group mean to that product inside the same parallel pass, and
the result is transposed once only after MAP certifies. When the exact DoF
result has already proved the AKM graph connected, normalization uses that fact
instead of rebuilding components. Saved-only publication also constructs the
absorbed target in its final buffer rather than allocating a separate `xb`
vector. Derived observation products execute inside the estimator's bounded
native thread team. Intercept-only retained products are scattered
observation-major, and saved-only calls do not materialize the additional
combined absorbed vector.

Publication remains independently fail-closed. Its intercept-only path
computes the reconstruction residual, weighted norms, and FE moments with
fixed deterministic chunks. The one-FE path evaluates these quantities
directly from the retained level coefficients, so it neither allocates a
second observation-length residual nor invokes the generic slope-capable
per-level solve. Multi-FE intercept recovery uses the same strict moment
definition with parallel level chunks; slope-bearing recovery retains the
general rank-revealing certificate. None of these publication kernels changes
the reconstruction threshold or FE tolerance.

Routes without a retained product, including `memory(low)` and SQUAREM, first
run one reconstruction-aware CG recovery capped at 256 iterations. If that
does not certify, intercept-only recovery performs four forward-Kaczmarz
sweeps with exact steepest-descent line searches while accumulating level
coefficients, then runs coefficient-producing LSMR only on the remaining
target. One-effect intercept recovery is direct. Slope-bearing recovery uses
strict coefficient-producing LSMR. Certified symmetric MAP remains the final
fail-closed fallback if the bounded coefficient solvers fail.

Every retained or recovered coefficient product is normalized by connected
component, scattered only for requested channels, checked against the absorbed
target in observation space, and independently subjected to the unchanged FE
moment certificate. Solver retention never relaxes the publication tolerance.

For an LSMR-routed residual, bounded fixtures must match the sealed 100-digit
oracle and large fixtures must remain within the registered envelope against a
100-times-tighter solve. Reghdfe residual distance remains a diagnostic for
this route; sample, singleton, coefficient/status, VCE, DoF, RSS, fit, FE-score,
and OLS-score contracts remain hard publication gates.

## OLS residual reuse and projection cutoff

Fast OLS always reconstructs the within-regression residual from the QR state.
That residual is submitted directly to the selected FE certificate/solver: an
already-within residual returns at iteration zero, while a failed certificate
triggers refinement. The raw and within residual vectors are not retained
simultaneously. The initial certificate reuses the column norms computed over
the same unchanged residual during setup, removing one redundant full-matrix
scan without reusing or weakening the independent FE-moment calculation.

Matched cutoff profiling at 32,768, 65,536, 131,072, and 262,144 rows selected
65,536 observations for parallel borrowed projections. It improved the
registered eight-thread standard CG median by more than 10 percent without a
measurable one-thread regression. Small batched RHS projections use a fused
per-group kernel, CG reuses its projection matrix, and large weighted dot
products use deterministic 32,768-row partial reductions. Final publication
still requires the complete independent certificate.
