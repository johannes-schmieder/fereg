# fereg behavioral specification

This document governs scientific results and failure behavior of the single
fereg runtime. [Architecture](ARCHITECTURE.md) describes current ownership;
[development](development_workflow.md) describes validation. Pinned external
packages are compatibility references; independent mathematics decides correctness.

In this document N is retained physical rows, N_eff is effective observations,
D is the complete absorbed design, X the finite design, and W positive fitting
weights. All row identities refer to the original dataset. "Tests" names
existing executable checks to carry forward, not unexecuted replacement claims.
Native test names below are relative to `crates/fereg-core` or
`crates/fereg-testkit`; Stata names are relative to `tests/stata`.

## 1. Public syntax and options

### Ownership and cell scope

The accepted ownership/cell implementation was qualified with CMG `fd77d98d`; the runtime pins
CMG `5c5b5b2b`, which adds arithmetic-preserving team dispatch of row-operator
matvecs (gated on entries times lanes, `f6a044c9`) and the 32-level hierarchy cap
(§19). The cap keeps every hierarchy of at most 32 levels bit-identical and lowers
the CMG bytes this scope admits and charges as retained. Qualified OLS `solver(auto)` with
`preconditioner(auto)`, a global intercept and exactly two categorical-intercept
FE terms selects both ownership mechanisms and eager cell construction.
Explicit developer injections retain their specified selection. Core defaults
and requests outside this scope remain unchanged. The
[acceptance record](../bench/results/ownership-qualification-final-linux-20261024.json)
and [practical Mac check](../bench/results/ownership-qualification-practical-macos-20261024.json)
record the performance decision; public/hidden integration checks preserve the
selected invocation's science, solver decisions and resource semantics.

Two independent OLS ownership experiments may omit provably unused speculative
raw moments and retain the projected matrix for inference scores instead of
copying selected columns. They preserve scientific bits, solver signatures,
sample, rank, omissions and all discrete decisions. Retained storage is charged
at its full width, including outcome and inactive columns. Admission precedes
omitting the packed copy; uncertain eligibility retains the existing path.
Preparation and FE admission cannot spend hypothetical ownership savings.

The first-occurrence cell operator experiment is restricted to eligible two-FE
OLS Auto/Auto categorical-intercept designs. Its continuous outputs and solver
iteration trajectories may change within the existing scientific tolerances;
sample, rank, omissions, factor identities and other discrete decisions remain
unchanged. Fixed-worker repetitions remain exact. Independent dense weighted
OLS, analytic and Schur witnesses and the original observation-space
certificates remain authoritative. Ownership alone remains exact even when
combined with this operator. Reuse is confined to an unchanged design, sample
and normalized-weight owner. Construction, temporary and retained storage count
toward real memory admission; refusal retains the observation operator.

CMG commitment by gate v2 (below), cutoff 5, one sampled tree, seed 0, graph
construction, small-direct selection, certification, restart and fallback
semantics remain fixed. The same original graph produces the same hierarchy
when both arms construct one. Public two-FE Auto constructs the cells once,
after the initial certificate and any direct route and before the diagonal
pilot, so the pilot's Schur products also use the cell operator; the pilot keeps
its D_f preconditioner, so only the operator's low bits change and the frozen
gate v2 stall threshold keeps its meaning. Construction runs on the host team
with the serial constructors' bits when its optional scratch is admitted. The
former deferred order (construction only after successful CMG commitment at the
existing restart, adding no setup to short solves) remains the `cells_deferred`
developer arm. Eager construction was qualified against the deferred public
order in the [round-two record](../bench/results/two-fe-round-two-20261027.json)
(Linux 123 of 126 protected comparisons within max(5%, 1 ms) after the
distinct-host waiver, 3 inconclusive; Mac 115 passed, 11 inconclusive; no
regression). The serial cell product and construction poll for Break once per
block of work, not per cell. The cell product runs on
the host team over a firm-major copy of the cells kept in increasing worker
order, which reproduces the serial product's bits at every lane count; the copy
is admitted with the kernel. There is no construction-work cap or
elapsed-time routing. See the [development protocol](development_workflow.md#compact-candidate-qualification).

Both commands accept `depvar [indepvars] [if] [in] [weight], [options]`.
The dependent variable is plain numeric; the finite list can be empty.
Stata 18 is the minimum host. The installed diagnostic is
`fereg, selftest [threads(#) verbose]`; developer actions are not public commands.
It reports installed versions, validates ado/plugin compatibility and runs bounded
installation checks. `verbose` adds build, host and per-check details; failed
checks always include their details and return a nonzero code. Available version
information is printed before compatibility or check failures. These checks cover
protocols, factor encoding, metadata and worker execution, not estimator qualification.
The former `version` and `doctor` options are removed and return r(198).

| Option | OLS (`fereg`) | PPML (`feppml`) |
|---|---|---|
| `absorb(terms)`; named saved channels | Yes | Yes |
| `keepsingletons` | Yes; default prune | Yes; default prune |
| Weights | none, aw, fw, pw | none, fw, pw |
| `vce()` | unadjusted/iid/ols, robust, cluster | robust, unadjusted mapped to robust, cluster |
| `dof()` | pairwise (default), firstpair, continuous, none | No public DoF-policy override |
| `noconstant` | Suppress reported constant | Unsupported; structural constant required |
| `residuals(newvar)` | Response residual | Formula selected by `residualtype()` |
| `solver()` | auto (default), map, lsmr, cmg, cmgschur (experimental), it, squarem | auto (default), map, lsmr, cmg |
| `cmgdegree(#)` | Experimental cmgschur only; 4, 5 (default), or 6 | Unsupported |
| `cmgsamples(#)` | Experimental cmgschur only; integer 0 (default) through 4294967295 | Unsupported |
| `cmgseed(#)` | With positive cmgsamples only; integer 0 (default) through 9007199254740991 | Unsupported |
| `threads(#)` | Nonnegative integer; 0 resolves native default | Same |
| `tolerance(#)` | Finite 0 < t < 1; default 1e-8 | Same, outer convergence tolerance |
| `iterate(#)` | Positive integer; default 16,000 | Positive integer; default 10,000 outer iterations |
| `timeit` | Runtime diagnostics | Runtime diagnostics |
| `verbose(#)` | Existing verbosity control; default 0 | No new public verbosity option |
| `offset(var)` / `exposure(var)` | Unsupported | Mutually exclusive |
| `separation(methods)` | Unsupported | Default `fe simplex ir`; optional mu or none |
| `xb()`, `d()`, `eta()`/`xbd()`, `mu()`, `dropreason()` | Through documented postestimation | Saved outputs (§12) |
| `biascorrection(exact|randomized)` | Unsupported | Optional mean correction (§15) |
| `biasunit()`, `biasfe()`, `biasprobes()`, `biasseed()`, `biasstream()` | Unsupported | Correction controls (§15) |

Preserve supported solver/map/iteration options, preconditioner(), poolsize(),
memory() and deterministic. solver(cg) remains rejected with r(198) under the
existing safety repair. Explicit LSMR/CMG and both PPML mean-bias modes are required.

Both commands load one platform-native fereg plugin. The former engine() option,
developer engine selector and engine-identity results are removed. Unsupported
engine() requests return r(198); a missing plugin returns r(601). Diagnostics
identify the native plugin and build.

The model capabilities below are preserved. IV/GMM, new categorical-continuous
or continuous-continuous RHS interactions, group outcomes with individual FEs,
HAC/Driscoll–Kraay, other GLMs and GPU execution remain unsupported.
Hidden separation tuning, allocation injection and plan overrides are test
interfaces, not additions to the public options above.

### Solver support and dispatch

`within` names the pinned modified-LSMR library, not a fourth public solver.
All supported backends must honor the same scientific model and accuracy
contract. Shared internal operations preserve explicit solver and storage support;
unsupported requests fail clearly.

| Request | OLS support to preserve | PPML support to preserve |
|---|---|---|
| auto | No FE, one/multiple intercept or slope-bearing FEs, disconnected designs | Same model grammar with changing positive IRLS weights |
| lsmr | General supported FE grammar, including slopes and multiple RHSs | At least two categorical intercept FEs, including more than three; no absorbed slopes |
| cmg | Categorical intercept graph: two largest effects as bipartite core, additional effects with at most 64 non-base nuisance columns; no slopes | Exactly two or three categorical intercept effects; third-effect nuisance width at most 64; no slopes |
| cmgschur (experimental) | Exactly two categorical intercept effects; positive observation weights, finite controls, disconnected components and saved effects | Unsupported |

The experimental `cmgschur` route eliminates the larger FE implicitly in the
outer Schur operator. By default, its CMG preconditioner eliminates levels with
at most `cmgdegree()` distinct neighbors and retains the others as auxiliary
vertices. Repeated observations form weighted cells before degree counting.
A fixed symmetric CMG cycle receives component-centered firm residuals padded
with zero auxiliary loads; its firm output is restricted and centered again.
The outer PCG recurrence uses this preconditioned residual in every energy and
direction update. It first targets half the requested FE tolerance, reconstructs
and independently certifies observation residuals at the requested tolerance.
Only a certificate miss can trigger one residual correction, targeting a
hundredth of the request, floored at 1e-15 and never looser than the request;
the correction runs only if that target is stricter than the first target.
Both attempts share the original iteration budget. Failure is explicit;
it does not silently switch solvers. The existing auto, CMG and other routes
retain their algorithms and exact scientific results.

Declared prototype scope: new-route coefficients, covariance, fit statistics,
residuals and FE coordinates may differ within section 20 tolerances; sample,
rank, omission and other discrete decisions do not change. Independent witnesses
cover dense weighted dummy OLS, sealed weak graphs, repeated cells, disconnected
components, Schur/hybrid equivalence and a multilevel cycle's linearity, symmetry
and positive energy. No default-route or PPML algorithm change is authorized.

CMG-Schur optimizations retain this numerical scope for
certify-then-refine stopping and batched fixed-cycle application. Independent
dense weighted OLS and sealed projection witnesses remain authoritative;
fixed-thread runs must repeat exactly, and sample, rank, omissions and other
discrete decisions stay unchanged. Lazy hierarchy construction, inactive-column
skipping and parallel recurrence updates preserve the explicit failure contract,
memory admission, Break handling and final requested-tolerance certificate.
Other solver routes retain exact scientific results and solver signatures.

The optional sampled extension uses `cmgsamples(k)` with an unsigned 32-bit
integer count. Zero (the default) preserves the exact retained-star hybrid.
Positive counts replace each above-cutoff worker with the average of `k`
independently seeded weighted clique trees; low-degree cliques remain exact.
`cmgseed(s)` selects an integer seed in 0..2^53-1 (default zero) and may be
explicitly supplied only when sampling is enabled. Both options require
`solver(cmgschur)` and retain its OLS-only support envelope. Repeated observations
are aggregated before sampling. A connected, positive tree spans each worker's
distinct firms, preserving the firm-component kernel. The graph is sampled once
at construction and remains fixed throughout PCG and residual correction.
The exact implicit Schur operator, observation reconstruction and certificate
are unchanged; neither observations nor the target model are sampled.

Declared sampled-extension scope: only explicitly sampled CMG-Schur fits may
change coefficients, covariance, fit statistics, residuals, FE coordinates and
solver iteration signatures within the existing prototype tolerances. Sample,
rank, omissions and other discrete decisions remain unchanged. Unrequested
sampling, including `cmgsamples(0)`, preserves existing scientific bits and
solver signatures. Independent witnesses cover exact enumeration of a small
weighted tree distribution, dense worker-clique mathematics, component kernels,
symmetry/positive energy, degree-30/60 weighted dummy OLS, fixed-seed/thread
repeatability, memory admission and Break/rollback. Every sampler allocation
is admitted before growth; edge-count overflow, nonfinite or underflowed edge
weights fail explicitly without silently changing the requested sample count.
Unbiasedness in exact arithmetic is not a spectral-quality guarantee.

The OLS CMG adapter also handles zero/one categorical effects by exact
no-work/direct projection; preserve this degenerate support and identify that
no graph iteration occurred. PPML explicit requests outside its supported
envelope return r(498), including zero/one FE. A valid graph formulation,
component anchoring and certified nuisance elimination are required for CMG;
the 64-column bound is an explicit adapter support limit, not an auto-routing
threshold. Retain all saved-output and bias-correction capabilities on supported
solver/model combinations. Tests force each supported backend independently.

Explicit OLS CMG polishing must resolve small remaining FE components even when
their moment certificate already passes. Its observation-space CG uses the
actual Hestenes decrement and requires relative recurrence convergence at the
requested tolerance, followed by the independent original-design certificate.
Nonzero final residual candidates undergo the same verification. An entrywise
zero computed projection may return without a CG update after its workspace
and action have been accounted; a certificate alone cannot justify that exit.
A zero residual energy with nonzero entries fails explicitly rather than
certifying an underflowed norm. The original iteration limit remains binding.

Only this CMG polishing action preserves the high/low parts of each subtraction
and the FMA product errors in compensated group reductions, with binary value
scaling. Ordinary CG, saved-model recovery and PPML retain their established
arithmetic. Strict polishing additionally carries its residual, direction and
operator product through FE coefficients and evaluates each updated product
with compensated summation. This preserves the mathematical FE-range
constraint instead of allowing rowwise recurrence roundoff to accumulate in
the operator's null space. It does not replace the Hestenes, recurrence or
original-design certificate gates. Three internal coefficient banks are
mandatory even when no FE coefficients are returned; a returned correction
uses a fourth bank. These banks must fit the existing joint memory allowance.
This correctness repair declares affected strict OLS CMG polishing/fallback/
final-residual outputs, iteration signatures and workspace/admission; it does
not claim historical bit identity for those affected results. Model, sample,
rank, omission and separation semantics do not change.
Successful bulk projections that retain FE coefficients also retain
and add the polishing coefficients. Joint admission includes the live graph,
norms, bulk coefficients and all CG correction banks at the actual RHS width;
final correction releases the graph before admitting its workspace. Requested
CMG remains explicit: a Fast final CG verification reports used CG and its
verification/correction warning, including when the verified action needs zero
CG updates. Sample, rank, omission and separation semantics are unchanged.

Ordinary `solver(auto)` retains its qualified automatic policy, iteration
budgets, support checks and numerical fallbacks. The optional serial routing
experiments below require sanctioned developer injections; they are not a
promoted default or new public syntax. Future default changes need comparative
evidence; no new N/width/thread/weight/memory fences may be introduced to rescue
individual performance cells.
Preserve requested/used solver, work accounting and existing reason diagnostics;
unsupported explicit requests must not silently become auto.

### Qualified Auto routing and developer comparisons

Public OLS Auto/Auto uses the many-FE geometry policy
(`routing_many_geometry_v1`) for eligible designs with at least three
categorical-intercept FEs. Eligible two-FE designs use
[structural CMG gate v2](two_fe_auto_gates.md) (`routing_two_gates_v2`),
with ownership and eager cells in the narrower global-intercept scope above. Zero/one FE,
slopes and explicit choices retain their specified behavior; PPML Auto adds only
the two-FE CMG reuse below. Activation and performance evidence for both OLS
families are complete; the many-FE policy carries one documented
near-confounded timing exception, see [VALIDATION](VALIDATION.md).

PPML Auto with exactly two categorical intercept FEs, no slopes and a
publication fit reuses one bounded CMG Schur hierarchy across IRLS frames as
historical solve acceleration (§13). Ordinary frames solve the joint FE and
finite WLS direction by PCG with a diagonal retained-effect block. A frame
whose recurrence reaches step 64 may build a missing hierarchy, or rebuild one
from an earlier weight generation, on its own weights and restarts from its
current iterate. An installed hierarchy `B` applies to the retained block of
later frames as `D^1/2 B D^1/2`, with the current frame's level weight sums
`D`. The canonical products' reduced Schur solve applies it to implicit
components of at least 64 levels as `R G' B G R`, with the Jacobi roots `R`
and `G` placing the component's grounded anchor. Before such a solve, a
hierarchy from an earlier weight generation is rebuilt on current weights;
without one, a plain recurrence of the first column on the largest component
that exceeds 32 steps builds one. A probe that finishes within 32 steps and
within that column's iteration cap is the column's solve on that component:
the column keeps the probe's coordinates and count instead of repeating the
identical recurrence. Three serial sections of the joint
recurrence run as exact parallel sections from 65,536 element operations
(fewer stay on the calling thread): the merge of helper partial products, the finite cross block
and each frame's level masses `D`. Each splits per-element or per-column work
into fixed chunks and keeps every element's serial operation order, so results
are bitwise identical to the serial form at every worker count, and each chunk
polls cancellation.
Categorical reprojections, used when strict canonical products decline, use
the two-FE Schur recurrence with the h16/k1 build:
without a hierarchy one may be built at step 16, and a hierarchy from an
earlier weight generation is set aside so the reprojection can rebuild it on
current weights, then reinstated unless replaced. Construction keeps cutoff 5,
one sampled tree, seed zero and the two-FE Schur hierarchy options, polls
cancellation and admits itself against
the fit's solver memory budget with any set-aside hierarchy charged as
resident; refusal leaves the diagonal recurrence unchanged. An estimation
makes at most three construction attempts, refusals included, and no
elapsed-time decision. Operators, stopping targets, reconstruction and every
certificate use the current frame; the hierarchy carries no certificate, rank
or Hessian authority. Explicit solvers, the separation phase, slopes and other
FE counts retain their behavior.

Canonical products solve implicit components of at most 16 free coordinates
as dense Cholesky systems of their applied unit columns. Beyond `size`
recurrence steps, components below 64 free coordinates get `size + 8` more
and larger ones two. A dense reduced matrix without an affordable
factorization is not assembled when assembly costs more than 64 applications
of the implicit incidence operator, which serves instead.

Declared PPML scope: eligible PPML Auto fits may change coefficients,
covariance, fit statistics, residuals, FE coordinates and solver iteration
signatures within §20 tolerances; through the strict entry rule (§13) and the
canonical-product solves, so may every PPML publication fit and every PPML
fit with categorical FEs. Sample, rank, omission, separation and other
discrete decisions remain unchanged. Witnesses cover the dense
Poisson oracle on weak chain and AKM graphs that exercise reuse, exact
refusal, the scaled block against the hierarchy, set-aside and attempt rules,
admission refusal, Break during construction, fixed-worker repeatability and
cross-worker agreement.

The many-FE geometry policy keeps retained unseeded LSMR preparation and adds
three parts. First, with exactly three unit-weight symmetric FEs, every
projection action without a prepared LSMR solver, including the final-residual
action, may run the bounded geometry-qualified MAP pilot described in
[ARCHITECTURE](ARCHITECTURE.md). Second, the retained LSMR solver starts as a
Jacobi (diagonal) trial. From its third iteration each RHS projects the
iterations it still needs from the geometric mean of its last two
normal-equation residual contractions toward the recurrence target. Once the
projected total exceeds 40 iterations, about the cost of building the additive
Schwarz preconditioner and running its usual iterations, the trial solver is
released, the additive configuration that the slot rule selects is built once,
and each escalated RHS continues from its iterate with its remaining budget.
The trial solver and its escalation share one prepared pinned-solver design
(observation frame, locality sort and layout), so escalation builds only the
additive solver state; this is arithmetic-preserving, and the kept design stays
within the additive solver frame that admission charges.
Later calls on the binding reuse the solver it holds; reports name the
preconditioner that finished. Admission charges the additive bound throughout.
Third, every successful exit (pilot, LSMR or MAP fallback) is accepted only by
the independent original-design certificate at the requested tolerance, as
with the pilot and the hidden prefix arms. The LSMR recurrence stops one
decimal order below the request, floored at 1e-14 and never looser than the
request, as two-FE Auto stops at half of it. A certificate miss permits one
tighter correction at Auto's private hundredth when that target is strictly
tighter: a categorical seeded LSMR session continues from the rejected
coefficients with the original response, weights and RHS normalization, and
only the remaining LSMR budget. The rejected solve keeps its coefficients
through certification only when the joint memory plan admits them, and the
correction starts only when its seed and warm-session banks fit; otherwise, or
after a second miss, the existing original-input MAP fallback runs on the
shared budget. A corrected exit reports the combined work without a fallback
warning. Native iteration caps, memory admission and cancellation are
unchanged.

Declared scope of the geometry policy: eligible public OLS Auto/Auto may change
coefficients, covariance, fit statistics, residuals, FE coordinates and solver
iteration signatures and diagnostics within §20 tolerances. Unit-weight
three-FE Auto with an explicit preconditioner reaches the same pilot and may
change through its earlier certificate checkpoint. Sample, rank, omission,
original factor slots/base flags and other discrete decisions remain unchanged.
Hidden `routing_incremental_many` keeps the previous public policy exactly. The
parallel copies, RHS transport, pair moments and session starts, the pilot's
norm handoff and its fused sweep passes (beta.10; optional admitted scale
tables, see [ARCHITECTURE](ARCHITECTURE.md)) are arithmetic-preserving. Independent witnesses cover pilot
acceptance one sweep early with identical bits, forced certificate misses
answered by one seeded correction (with and without retained coefficients,
repeated bit-for-bit) or by the MAP fallback after a second miss, the norm handoff against the
LSMR kernel and an independent sum, exact split pair moments for every lane
count, Jacobi completion on mixing panels, escalation near confounding with
agreement against the strict reference and FE-moment reconstruction, and
fixed-worker repeatability.

Additive-Schwarz admission charges `T + R` level-sized worker banks per
concurrent RHS apply, where `R` bounds the factor-pair components whose local
solve may yield into nested work (a block of more than 10,000 levels). The
level-count bound `R = levels * (effects - 1) / 10,002` grows with the levels,
so the charge is quadratic in them. When it refuses the additive LSMR attempt
of the general categorical OLS Auto route (many-FE designs, and two-FE designs
whose graph and compact routes were refused), fereg counts the structural pair
components with a block of at least 10,000 levels exactly from the codes,
once per design, after admitting that count's O(levels) scratch, and admits
the attempt against the same bound with the smaller `R`. Within's subdomains
are the bipartite components of each pair's cross-tab, and with the route's
strictly positive weights those are the structural pair components. An estimate that fits, an unbudgeted call,
an explicit non-additive preconditioner or an unadmitted count scratch keeps
the level-count estimate and computes nothing new.

Declared scope of the exact count: only fits whose additive attempt the
level-count bound refused change, from the original-input MAP fallback to the
unchanged LSMR attempt of their route (for many-FE Auto the geometry policy:
Jacobi trial, additive escalation, requested certificate, correction and MAP
fallback). They may change coefficients, covariance, fit statistics, residuals,
FE coordinates, solver iteration signatures, reported solver, workspace peak
and fallback diagnostics within §20 tolerances. Sample, rank, omission,
original factor slots/base flags and other discrete decisions remain
unchanged. Every fit the level-count bound admits keeps its bits, route and
memory accounting. Witnesses: the count against an
independent breadth-first search; the beta.8 campaign's sparse three-FE
parameters (level bound about 1 TB at ten million rows, exact count 3, linear
growth); and a one-million-row campaign topology under a budget one byte
below the level bound, which admits the additive route with the unbudgeted
fit's bits at one and four workers, repeated, agreeing across workers and
reconstructing the input from its FE coefficients, where the level bound ran
the MAP fallback.

The same refinement, under the same rule (only after the level-count bound
refused; the exact `R` is never larger), covers the other additive-Schwarz
LSMR admissions. Explicit OLS `solver(lsmr)` on a borrowed categorical design
refines its call admission. Owned designs refine the generic LSMR admission of
explicit `solver(lsmr)` and of the automatic owned route when every term is one
intercept channel and every weight is strictly positive; designs with slopes
keep the level-count bound, because their subdomains are not the code
components. The PPML preflight (explicit LSMR's joint Schwarz factor over the
core pair or both effects, and the checkpoint accelerator over every pair)
recounts once when its plan refused, with the strictly positive PPML working
weights; a plan that still does not fit reports the original refusal. Each
count is taken once per design, after its O(levels) scratch is admitted, and a
count whose scratch or allocation is refused keeps the refusal. Declared scope:
only fits the level-count bound refused change. Explicit LSMR fits move from
an `r(909)` allocation refusal to their unchanged LSMR solve; the automatic
owned route moves from its MAP fallback to its unchanged LSMR attempt; refused
PPML plans move from the preflight refusal to their unchanged fit. They may
change coefficients, covariance, fit statistics, residuals, FE coordinates,
iteration signatures, reported solver, workspace peaks and fallback
diagnostics within §20 tolerances; discrete decisions are unchanged.
Witnesses: the per-pair cache against the independent search on every pair
subset; explicit borrowed LSMR, explicit and automatic owned LSMR under a
budget one byte below the level bound with the unbudgeted solve's bits,
repeated at one and four workers and agreeing across them, with a budget below
the exact bound still refused and a sloped design still refused; and PPML
explicit LSMR on the sparse three-FE topology, whose refused plan is admitted,
repeats bit for bit, agrees with the unbudgeted fit and refuses below the exact
bound with the original message.

Candidate bounded LSMR session windows (hidden `routing_many_windows_v1`, not
public): the many-FE geometry policy unchanged except that its unseeded and
seeded LSMR solves keep at most one live session per worker instead of every
RHS of a call (at most 64, or four per worker). RHSs run in fixed windows of
consecutive columns; each window starts its sessions, runs them to completion,
finishes and releases them before the next window starts. The Jacobi trial's
escalation barrier falls at the end of each window: if a window's trial
projects beyond its budget, that window's escalated RHSs continue from their
iterates on the additive solver built once from the shared design, and every
later window of the call, and later calls on the binding, start on the
additive solver without a trial. A window whose RHSs all finish on the trial
leaves the trial retained for the next window. Without any escalation every
RHS runs the same independent recurrence as in one window, so results are
bit-identical to the public policy; with an escalation, RHSs after the
escalating window may finish in fewer iterations with different low bits.
Admission still charges the established window bound, so the arm admits at
least what the public policy admits; only the reported workspace peak's
warm-session term follows the smaller window. Two-FE and ineligible designs
fall back to exactly the public policy, including ownership/cell activation.
Declared scope of the arm: injected many-FE OLS Auto fits with an escalating
Jacobi trial may change coefficients, covariance, fit statistics, residuals,
FE coordinates and solver iteration signatures within §20 tolerances;
discrete decisions are unchanged. Public activation is a separate decision
after qualification. Witnesses: dense explicit-dummy QR at one, two, four and
eight workers on a finishing and an escalating panel, fixed-worker repeats,
exact public agreement where one window holds every RHS or no RHS escalates,
and the four-, eight-, sixteen-FE, small-third and near-confounded routing
families against the public arm.

The CMG builder borrows the host team, polls cancellation at fixed logical chunks
and admits construction against real memory, without a construction-work cap or
budget-dependent factor screen. Preserve cutoff 5, one sampled
tree, seed zero, eligibility, small-direct selection, certification, restart and
fallback semantics. See the [execution contract](ARCHITECTURE.md#cmg-host-team-and-stage-admission).

Every two-FE Schur hierarchy (OLS Auto, explicit `cmgschur` and PPML reuse) is
built with CMG's options except a zero low-effective-degree threshold: on
expander-like firm Schur graphs CMG's correction detaches most firms into
singleton aggregates, so coarsening stagnated. The options change only the
preconditioner. CMG's default depth limit of 32 levels is the hard stop of each
such hierarchy and the level count of every CMG bound fereg admits (§19). OLS Auto installs its hierarchy on the host team: CMG's
row-operator plan and up to four interleaved lanes, each only when its bound
is admitted after the one-lane apply storage, otherwise the scalar
application of the same hierarchy, so the team installation never refuses a
build the scalar one admits. Its arithmetic is the scalar cycle's except
CMG's fixed-chunk centering of one coarse component of at least 131,072
entries. PPML reuse keeps the team-free scalar cycle.

Developer selectors below are supported comparison contracts; public defaults
are specified above.

Hidden OLS `inject=routing_h{4|8|16|32}_k{0|1|2}` selects a fixed iteration
checkpoint and high-degree CMG sample count. Eligible two-FE problems start the
existing diagonal-preconditioned Schur recurrence and consider one optional
CMG build if work remains at the checkpoint. The general categorical
more-than-two-FE pair-upgrade interpretation of these selectors and
`routing_builder_break` is retired with Johannes's approval. Those explicit
requests return InvalidArgument on more-than-two-FE categorical designs; the
two-FE selectors and their builder-Break witness remain supported. Zero/one-FE
and slope routes retain their existing behavior. This retirement does not change
public many-FE Auto's Jacobi-to-additive escalation or its supported prefix,
portfolio, geometry and bounded-window comparison policies.

Construction runs serially at the numerical checkpoint, without elapsed-time or
background-completion decisions. Refusal leaves the live recurrence unchanged;
acceptance restarts from the current iterate with a fresh correction residual
and original RHS normalization. Checkpoints use recurrence readiness; final
acceptance still requires the original-design certificate at the requested
tolerance. Pilots, restarts, corrections and fallbacks share the existing
per-projection-action native cap, not an estimation-wide cap across Low chunks.

Setup and a built inverse are shared by the immutable sample/topology/weight
binding; one optional attempt belongs to each accuracy key, and a new
accuracy/final-residual phase may reuse the inverse. Joint memory admission
covers resident hierarchies, live recurrences, restart buffers and application
scratch, and every allocation is fallible. Optional construction is cancellable
at bounded logical chunks. A memory or resource refusal before installation
leaves the live recurrence unchanged and resumes the incumbent. Cancellation
(Break) and fatal errors are never swallowed: after all dispatched tasks join,
they propagate as an interrupted or failed native estimation with atomic
rollback; no PCG/MAP resumption and no publication follows a cancellation.
Existing preconditioner-breakdown fallback uses only the remaining native
budget.

The synchronous two-FE observation builder may retain its first-pass canonical
cells and already-compensated worker totals for edge emission. This preserves
the original canonical values, order and sampler input bits. Before allocating
the cache, a checked conservative envelope admits the cache together with cell
scratch, sampling, graph construction, components, hierarchy, scalar workspace
and final application banks, net of live parent/execution storage. Overflow or
insufficient memory chooses the original two-pass path before allocation, with
no second attempt. Cache bytes remain reserved through finish. Cached and uncached builds
require identical graph, hierarchy and application results. This optimization
does not activate public routing or establish a speed claim.

Three separate hidden two-FE experiments isolate structural choices:
`routing_diagonal` uses the current-weight Schur diagonal with the observation
operator; `routing_cells` retains the incumbent `D_f` preconditioner and applies
the operator through deterministic aggregated weighted cells;
`routing_direct` considers componentwise grounded
Schur factorization in either orientation. Its structural noncertificate work
`A` must satisfy `A <= 4 W_ref`: `A` includes sunk census/profiling work for both
orientations, assembly, factorization, solves for the currently committed RHSs,
reconstruction and direct-specific checks. `W_ref` prices one incumbent iteration
in the incumbent orientation with the current active RHSs packed, without credit
for inactive capacity. The complete attempt must fit the joint memory budget;
the protected fallback certificate/output reserve is unchanged. There is no
construction-work allowance. The certificate is common only when its kernel,
scales and acceptance contract match the incumbent's; direct-only preparation
remains in `A`. Checked overflow declines the proposal. There is no second
candidate after factorization or certificate failure; the incumbent remains
available. The four-iteration threshold applies only to
structural work and carries no total-overhead or speed guarantee. Grounding and
zero-diagonal preconditioning safeguards are not numerical-rank authorities.
Zero/one-FE and slope routes retain their existing behavior. Separate prefix
and PPML topology experiments are described below. Exact hidden tokens
and validation are specified in [the plugin protocol](plugin_protocol.md#developer-injections-hidden-adapters-only);
implementation boundaries are [routing presets and the attempt record](../crates/fereg-core/src/fe/routing.rs),
[many-FE controlled LSMR](../crates/fereg-core/src/fe/borrowed_map/routing_lsmr.rs),
[two-FE kernels](../crates/fereg-core/src/fe/routing_kernel.rs) and
[direct proposals](../crates/fereg-core/src/fe/routing_direct.rs).

Declared routing scope: only explicitly injected OLS routes may change
coefficients, covariance, fit statistics, residuals, FE coordinates and solver
iteration signatures within §20 tolerances. Sample, rank, omission, original
factor slots/base flags and other discrete model decisions remain unchanged;
uninjected routes retain exact scientific results and solver signatures.
Independent witnesses cover weighted rectangle and dense observation-elimination
identities, disconnected direct solves, exact two-FE recurrence continuation after
refusal, and original FE-moment and coefficient
reconstruction checks. No performance or default-promotion claim follows from
these contracts.

Explicit OLS `solver(it)` retains its support envelope and incumbent
observation-space behavior. Hidden `inject=it_coefficients` independently tests
a categorical intercept challenger; it compares complete simultaneous memory
plans before allocation: compact
acceleration owns three coefficient banks and one candidate residual panel;
observation-space acceleration owns three residual snapshots and, only when
retaining FE coefficients, three coefficient banks. Each plan includes its
coefficient layout metadata and the fully reserved sweep/certificate workspace;
caller-owned input storage remains in execution memory. The smaller plan is selected, with
ties favoring coefficient space; failure to admit it is an allocation error.
No IT prefix is promoted into ordinary automatic routing, and generic
slope-bearing IT retains its existing implementation.

Compact IT computes its extrapolation parameter in the Euclidean coefficient
metric, preserving the original-slot sweep gauge. It reconstructs the proposal
in observation space and accepts it only when its independent original-weight
certificate's relative two-norm strictly improves on the ordinary two-sweep
baseline. An unusable or rejected proposal leaves that baseline and its
coefficients intact. A passing iterative
certificate receives a confirming sweep when iterations remain; all ordinary
and confirming sweeps share the original cap; initially certified input returns
at zero sweeps. Exhaustion without certification is explicit. The changed metric
may change injected categorical IT coefficients, covariance, fit statistics,
residuals, FE coordinates and iteration signatures
within §20 tolerances; discrete decisions and unaffected solver routes remain
unchanged. Witnesses include weighted disconnected rectangles, exact memory
boundaries, exhaustion, and coefficient reconstruction in
[categorical IT tests](../crates/fereg-core/src/fe/borrowed_map/irons_tuck.rs), plus
[independent dense-QR OLS comparisons](../crates/fereg-oracle/tests/irons_tuck.rs)
across two, three and five FEs, Fast/Low modes and worker counts, with saved effects
and an unaffected slope-IT check.

The isolated categorical prefix arms replace the existing three-FE shortcut;
they never stack with it. `routing_prefix_none` omits that shortcut,
`routing_prefix_map4` permits four native MAP sweeps, and
`routing_prefix_it4` permits four compact IT cycles, priced as at most eight
native sweeps including confirmation. A prefix starts only when its complete
memory plan fits and its nominal sweep allowance leaves a native LSMR step.
Every executed sweep consumes the existing action cap. An unfinished prefix
passes original-slot coefficients to a categorical seeded LSMR session with the
original response, weights and normalization; its result already includes the
seed. All three hidden prefix arms certify the original problem at the requested
MAP tolerance, both before and after LSMR correction; the stricter private LSMR
recurrence tolerance and native iteration caps remain unchanged. The seed and enlarged warm-session banks are admitted on every solver and
preconditioner-fallback path. Each Low chunk and final-residual action retains
its own cap. The backend's presence does not suppress an explicitly requested
prefix. An early certificate reports the concrete MAP or IT family. Low reports
combine those prefixes with actual LSMR metadata when a chunk needs LSMR, while
preserving any real fallback; an intentional prefix alone is not a fallback.
Uninjected Auto uses the geometry pilot instead.

The separate `routing_portfolio_v1` qualification arm uses one fixed structural
portfolio. With exactly two categorical FEs it retains the incumbent recurrence
and first prices an allocation-free direct-or-CMG choice once per accuracy key,
at the first uncertified action without a retained inverse. A checked
dimension-only upper bound must admit direct census, both orientation profiles,
assembly, factorization and outputs within four packed current-RHS incumbent
iterations, including the constant 1,024-unit preflight envelope, and the
complete attempt must fit the simultaneous memory budget. A selected direct
attempt consumes the sole optional opportunity before census; numerical,
geometry or certificate refusal cannot enable CMG afterward. A declined
preflight retains the one CMG opportunity across Low chunks. Previously
retained CMG inverses remain reusable.

The incumbent-CMG choice considers one synchronous build at iteration 16 (or at
gate v2's earlier stall decision), using cutoff 5, one
sampled tree per high-degree worker and seed zero. Sampling changes only the
preconditioner; the original Schur operator and certificate remain authoritative.
The canonical cache uses the complete admission envelope above. These are
prospective engineering constants, not a policy selected by the earlier failed
Mac panel. There is no sampling ladder, resampling or second optional attempt.

The bounded build's terminal factor follows the hierarchy options (direct
threshold and stopping rules) and memory admission only; no construction-work
screen redirects a direct terminal to coarsening. Compatible Laplacian action
witnesses and exact disabled-path replay precede qualification.

With three or more categorical FEs, this arm tries at most four categorical MAP
sweeps before seeded LSMR. It replaces the three-FE shortcut,
including when prefix admission declines it. Both prefix acceptance and the
original-design certificate after correction or MAP fallback use Auto's private
`direct_lsmr_tolerance`, rather than the requested MAP tolerance used by the
isolated prefix arms. The original response, weights, normalization and factor
slots remain authoritative: an unfinished prefix passes coefficients only, and
the seeded LSMR result includes that seed. Actual prefix sweeps consume the
existing action cap; the correction receives only its remainder. This first
portfolio has no general-FE CMG checkpoint and does not stack direct or diagonal
attempts before CMG. Zero/one-FE and sloped paths, explicit solvers and PPML
retain their behavior. This new injected arm may change eligible OLS numerical
results and solver diagnostics within the existing scientific tolerances;
model and discrete decisions remain unchanged. It is
experimental and does not change public Auto or establish performance evidence.

Break during optional construction is exercised on the synchronous builder through the
hidden `routing_builder_break` arm: the h4/k1 two-FE build parks at its first
cancellation poll after executed construction work, Stata's main thread polls
Break, and the requested cancellation propagates as an interrupted native
estimation with atomic rollback after all dispatched work joins.

Hidden PPML `inject=ppml_topology` shares only component labels in the already
admitted categorical checkpoint path. It retains no weighted numerical factors,
maxima or normalized weights. Accepted and pending topology have separate
publication ownership; acceptance and rollback transitions allocate and log
nothing. Rejected trials preserve accepted state, while support or incompatible
generation changes invalidate reuse. Weighted data is rebuilt for each current
frame. The default cache is disabled, and this experiment does not change outer
engines, strict checkpoints, separation, sample/rank decisions or bias modes.

Explicit OLS `solver(lsmr)` stops its recurrence at a hundredth of
`tolerance()`, floored at 1e-15 and never looser than `tolerance()`, and
certifies the FE moments at `tolerance()`. Johannes chose these scoped changes
on 2026-10-10: an equal-tolerance stop could leave a certificate just above the
request, both in the main projection
(`development/ols_three_fe_batch_oracle.do`) and in the 1e-14 final residual
projection used for saved outputs. Unlike automatic LSMR (floor 1e-14, with a
MAP fallback), explicit LSMR has no fallback, so it keeps that margin. Explicit
PPML LSMR keeps its scheduled certificate and one tighter continuation.

Ordinary automatic two-FE OLS first targets half of `tolerance()` in its
recurrence and certifies at `tolerance()`. Only a failed certificate permits
one tighter correction solve on the reconstructed residual, using the automatic
LSMR target when strictly tighter. Both attempts retain the original RHS
normalization and share the original PCG budget and MAP reserve. A passing
certificate is accepted even at recurrence exhaustion. This scoped adaptive
policy does not change explicit solvers, PPML or final-residual projection.

Automatic routes can differ at roughly the 1e-5 level in derived statistics
on weakly connected graphs, despite passing their requested FE-moment certificate.

Tests: `tests/model_wire_{roundtrip,adversarial}.rs`, `parser/`,
`parity_ppmlhdfe/feppml_command_contract.do`,
`development/ols_cmg_raw_residual_oracle.do`, `tests/prepared_fit_oracle.rs`.
Retain mapped support/rejection, exhaustion and fallback checks.

## 2. Sample construction, missingness and restrictions

Stata evaluates `if`/`in` once. Candidate exclusion precedes scientific domain
validation: excluded invalid values cannot cause a data-domain failure.
On candidates, apply outcome, finite regressors, weights, FE sources, cluster
sources, then PPML adjustment availability/domain rules in that order. Stata
numeric missing values, including extended missings, and string missingness
retain their host meaning. Missing required sources exclude rows; fatal domain
errors are not converted to exclusions. Cluster availability precedes singleton
pruning; cluster counts never define FE singletons.

Ordinary factor values are validated on the resulting available sample, after
these exclusions but before singleton pruning, separation or rank decisions.
They must be nonnegative integers; otherwise fail with r(452), including for
omitted/base terms. Invalid factor values on excluded rows do not cause an
error. This follows built-in Stata `regress`; it intentionally supersedes the
former candidate-wide ordinary-factor check and differs from pinned reghdfe
6.14.1 in excluded-row cases. FE and cluster identifiers
retain their existing numeric/string domains.

Rust owns classification, collision-safe factorization and stable compaction.
The same original-row map compacts every source, recipe, weight, offset and
topology. After deletions, rebuild final codes and factor recipes; preserve
canonical coefficient slots when levels vanish. Dataset order never changes.
Time-series materialization respects the full declared panel/time settings;
a retained lag/lead can use a source outside `if`/`in`, and gaps remain missing.
Empty or insufficient final samples fail, with no public sample posted.

Tests: `tests/numeric_sample_oracle.rs`,
`tests/categorical_final_sample_rebase_oracle.rs`,
`parity_ppmlhdfe/feppml_native_preparation.do`, `feppml_time_series.do`.
Replacement checks: `tests/categorical_cluster_preparation.rs` and
`tests/stata/development/reference_factor_precedence.do`.

## 3. Weights

| Kind | Candidate rule | N_eff | Reference objective weight |
|---|---|---|---|
| none | Structural unit weight; no pseudo-column | N | 1 |
| aw, OLS only | Zero/missing excludes; negative r(402); otherwise finite positive | N | raw × N / sum(raw) |
| fw | Missing excludes; otherwise positive exact supported integer; invalid r(401) | Checked exact frequency sum | raw multiplicity |
| pw | Missing excludes; otherwise finite positive; invalid r(459) | N | raw × N / sum(raw) |
| iw | r(101) | — | — |

PPML also rejects aw with r(101). Each fweight must be an integer in
1 through 2^32−1. Accumulate effective N with checked integer arithmetic
within the platform's supported index range; reference-statistic conversions
also enforce exact floating-point integer representability. Rounding must
not silently admit an out-of-domain multiplicity.
Raw weight expression/sum remain metadata. Common aw/pw rescaling leaves
estimates, reference-scale bread, covariance and fit statistics unchanged.
Internal common rescaling is permitted only with explicit conversion back to
reference scale. Recompute normalization after sample mutation. Fweights match
literal row replication for objective, singleton multiplicity, N_eff, DoF,
scores and covariance; repeats do not create independent bias units.

OLS pw selects robust when VCE is omitted; explicit IID is rejected. HC1 meat
uses squared normalized aw/pw weights, but fw multiplicity rather than fw².
PPML prior weights q follow this table, while Hessian weights are q×mu.

Tests: `tests/numeric_sample_oracle.rs`, weighted `fereg-oracle/tests/ols.rs`,
`tests/prepared_robust_oracle.rs`,
`parity_ppmlhdfe/feppml_weights_inference.do`, frequency-expansion fixtures.

## 4. Singletons

Prune FE singletons to a fixed point unless `keepsingletons` is requested.
FW uses checked integer multiplicity; other weights use physical membership.
Removal counts remain physical rows. Apply the rule to the final available
model sources and all absorbed partitions, with deterministic original-row
identity. Separation can start another singleton cascade (§14). Singleton and
separated counts/reasons remain distinct. `keepsingletons` does not exempt a
model from positive DoF, finite-rank or convergence requirements.

Tests: `src/fe/singleton.rs`, weighted singleton-cascade Stata fixtures,
`parity_ppmlhdfe/feppml_separation_outputs.do`.

## 5. Absorbed terms and saved normalization

Support numeric/fixed-width-string categorical sources; `g h` for separate
effects; `g#h` for observed-tuple interactions; `g#c.x` and `g#c.(x z)` for
slope-only channels; `g##c.x` and `g##c.(x z)` for intercept plus slopes.
Repeated sources share coding without changing declared term order. `strL`
and categorical `##` expansion inside absorb are unsupported. Disconnected
components, duplicate terms and locally rank-deficient slope channels retain
their exact span and reported redundancy; do not manufacture extra rank.

`name=term` saves normalized observation contributions. An intercept uses
`name`; slope channels use `name_Slope1`, `name_Slope2`, etc. Slope-only terms
have only suffixed channels. These are channel contributions (coefficient
times slope), not bare slope coefficients. Names must be new and distinct
from every other requested output and input binding. `savefes()` is not an alias.
Within each connected component, center every intercept effect after the
first in declared absorb order to weighted mean zero, transferring each
removed mean to the first intercept effect so the total is unchanged. Use
OLS fitting weights or PPML's accepted final Hessian weights q×mu. With zero
or one intercept effect there is no between-effect centering transfer. Slope
channels retain their rank-aware representative; arbitrary nonidentified
contrasts are not accuracy targets. Preserve the reported constant gauge and
compare nonunique FE solutions by fitted sums and identified contrasts. Apply
the same normalization to all backends. The distinct bias anchor in §15 must
not replace this ordinary saved-FE gauge.

Output recovery reconstructs the declared FE sum from coefficients, checks
each row, and certifies at the requested tolerance. Output recovery and automatic
tighter re-solves require declared affected scope before changes. Independent tighter test oracles remain required; reducing output work cannot relax their accuracy
envelopes. No output request may change the sample, rank, VCE or coefficient
status. Unsupported explicit backend structures fail before publication.

Tests: `tests/intercept_slope_oracle.rs`, `tests/fe_certificate_oracle.rs`,
`parity_reghdfe/estimate_ols_saved_outputs_parity.do`,
`parity_ppmlhdfe/feppml_saved_slopes.do`, `bias/final_recovery_precision.do`.

## 6. Regressors and canonical status

Support numeric continuous columns, numeric factor main effects,
all-categorical factor interactions, and Stata lag/lead/difference operators
on continuous variables. Stata supplies authoritative base/omission flags and
canonical names through `fvexpand`/`_ms_parse_parts`; Rust must not infer these
flags from spelling. Do not broadly expand design columns in ado/Mata.
Explicit RHS base overrides (`ib#`, `fvset`) with interacted or sloped
absorption remain unsupported until a separately approved extension.

Preserve input-order slots and stripes for kept, base, explicitly omitted,
FE-collinear and numerically rank-deficient columns. A vanished level becomes
an all-zero/omitted recipe without shifting subsequent slots. Omitted slots
and covariance coordinates remain consistently zero. No-regressor models use
an explicit empty graph, never the dataset's implicit full variable list.
Unknown operators, orphaned/gapped metadata, invalid flags, nonzero reserved
fields and pending time-series work are errors before observation access.

Tests: `parser/ordinary_factor_terms.do`, `parser/model_wire_v2_capture.do`,
`tests/model_wire_adversarial.rs`, final-sample rebase and TS dummy oracles.

## 7. OLS estimation

Minimize `(y-Xb-Dalpha)'W(y-Xb-Dalpha)` on the retained sample. Project y
and X using the same FE span/weights, solve finite WLS on the identified
coordinates, and reconstruct residuals from original sources. Independently
certify FE orthogonality, finite score, bread/inverse, covariance and fit.
No iterative recurrence status alone permits publication.

A Fast fit that requests no residual, xbd, d or saved-FE output may keep its
within residual `r = y~ - X~b` instead of projecting it again, when the derived
certificate of §16 passes; its residual solver row reports the absorption's
method, zero iterations, zero pilot work and stop code 5. The derived residual
serves the two-FE categorical owner (decided by Johannes, 2026-10-27) and, for
beta.10, the direct one-FE Auto route and the many-FE Auto geometry policy,
whose MAP pilot, LSMR session and MAP fallback all certify at the requested
tolerance (decided by Johannes, 2026-10-28). Requested outputs, Low memory,
explicit CMG, LSMR, MAP, CG, IT or SQUAREM sessions, many-FE developer routing
arms outside the geometry policy, designs without an absorbed effect and a
failing derived certificate keep the certified final projection. Residuals,
fitted values, residual-based fit statistics and every covariance (iid through
its residual sum of squares, robust/cluster through the scores) may differ from
the projected residual in low bits within the certified tolerance, wherever the
projection would have changed the within residual; coefficients, the bread,
sample, rank, omissions and discrete decisions do not.

Where an absorbed intercept exists, the reported constant is
`mean_w(y) - mean_w(X)'b`. It is a normalization coordinate, not an extra
finite/absorbed rank direction. With within bread B its extended bread has
side `-B mean_w(X)` and corner `1/N_ref + mean_w(X)'B mean_w(X)`, using the
reference weights and their N_ref=N_eff. Pure slope-only absorbed models do
not automatically have a structural OLS constant. No-FE models retain the
ordinary constant unless suppressed. `noconstant` preserves its accepted
model/reporting semantics; it does not delete an intercept already in D.

Tests: `fereg-oracle/tests/ols.rs`, `tests/prepared_fit_oracle.rs`,
`tests/categorical_iid_oracle.rs`, normalized-intercept Stata parity fixtures.

## 8. Rank and collinearity

Use deterministic scaled rank decisions and canonical tie breaking. Guarded
diagonal-pivoted Cholesky is permitted only when pivots clear the stronger
square-root rank guard and independent score/inverse checks pass. Otherwise
scaled column-pivoted Householder QR owns omission decisions; never invert
an unpivoted singular cross-product. Solve only identified coordinates.
Local FE block rank is the weighted span rank, including constant/zero or
dependent slope channels; a singular local Gram block is not automatically
an invalid model. Weak-graph tests retain identified combinations even where
individual FE coordinates are not unique.

PPML freezes canonical finite rank/status after the final separation/singleton
sample. Positive changing IRLS weights cannot silently change public omission
decisions. Revalidate that basis against the accepted final Hessian; a final
rank loss or inconsistent rank decision is fatal. Starts, warm states and
ordinary iteration-specific factorizations have no canonical-rank authority.

Tests: `src/ols/finite_wls_kernel.rs`,
`tests/intercept_slope_oracle.rs`, `tests/prepared_fit_oracle.rs`,
`parity_ppmlhdfe/feppml_scientific_contract.do`.

## 9. Degrees of freedom and nesting

Preserve the pinned reference's absorbed-category/redundancy rules, not a
generic count of FE labels. `pairwise` applies pairwise mobility reductions;
`firstpair` adjusts only the first eligible pair; `continuous` applies slope
redundancy without categorical mobility searches; `none` disables those
adjustments but still charges absorbed coefficients. Two-way graph components
give exact rank; multiway approximations remain explicitly marked. Constant
and locally redundant slope channels follow the accepted continuous rules.
Report categories, redundancy, rank, exactness/approximation and nesting per
declared channel. Nesting uses complete collision-safe final cluster tuples.

For OLS let K_f be finite model DoF and A the nesting-adjusted absorbed rank.
Unclustered residual DoF is N_eff−K_f−A and must be positive. With clusters,
at least two groups per dimension are required, reported residual DoF is
`min(N_eff−K_f−A, G_min−1)`, fit-statistic DoF subtracts the nested absorbed
category count as well, and the covariance correction denominator is
`N_eff−K_f−A−1{nested categories > 0}`. Each required denominator must be
positive. The normalized absorbed constant is not charged twice. Retain the
48-row positive-DoF slope fixture and the original 24-row failure regression;
negative IID variance is never a successful result.

PPML has its own policy: structural DoF is N_eff minus finite within rank
minus absorbed rank; clustered reported DoF additionally caps at G_min−1.
Positive reported DoF and at least two groups per declared cluster dimension
are required. PPML does not borrow OLS finite-sample corrections or t/F laws.

Tests: `src/ols/{topology_dof,prepared_dof}.rs`,
`tests/categorical_clustered_oracle.rs`, positive-DoF slope Stata fixtures,
`parity_ppmlhdfe/feppml_weights_inference.do`.

## 10. Covariance

Let B be the identified bread and s_i the within-regressor score. OLS IID is
`RSS/df_r × B`; HC1 is `N_eff/df_r × B meat B`. OLS cluster scores are
`sum_g omega_i r_i x_tilde_i`; combine every nonempty declared-cluster subset
by inclusion–exclusion and apply one common correction
`(N_eff−1)/correction_df × G_min/(G_min−1)` from §9. Support up to ten
OLS cluster dimensions. `#` defines one interaction dimension; `##` is invalid.
FW robust meat uses multiplicity, while cluster scores use full weighted sums.

PPML supports one through five cluster dimensions. Its bread is the inverse
of `sum q_i mu_i x_tilde_i x_tilde_i'` at the accepted Hessian.
Robust scores use `q_i(y_i−mu_i)x_tilde_i`; FW meat uses f_i times the
unweighted score outer product, not f_i². Robust correction is
`N_eff/(N_eff−1)`. Cluster scores sum the weighted scores within groups;
one common `G_min/(G_min−1)` multiplies the complete inclusion–exclusion
meat. No OLS residual-DoF or subset-specific multiplier applies.

Grouping is collision-safe and deterministic. Apply the registered symmetric
CGM PSD repair only after the complete multiway covariance has been formed;
retain repair diagnostics, symmetry/finite/diagonal checks and independent
covariance reconstruction. Johannes approved full-matrix OLS repair including
the reported constant; never restore a separately clipped slope block afterward.
This intentional difference from pinned reghdfe has an independent regression. Multiway repair sets negative covariance
eigenvalues to zero and independently checks the eigensystem and reconstructed
matrix. Robust and one-way clustered covariance must already satisfy the
strict PSD policy; they do not receive that multiway repair. Do not clip individual scores, coefficients or
subset meats. Wald rank and missing/undefined test statistics preserve the
canonical identified coordinates. OLS uses t/F; PPML uses z/Wald chi-squared.
The PPML model Wald leaves absorbed effects and the structural constant
unrestricted: it tests whether `eta−offset` lies in `span(D,1)`. Its null and
degrees of freedom are invariant to which identified recipes represent that
constant, including when the explicit `_cons` slot is numerically omitted.
Preserve original coefficient slots, base/omission flags and the accepted basis;
use identified contrasts when a nuisance constant spans several kept columns.

Tests: `tests/prepared_{robust,clustered}_oracle.rs`,
`tests/categorical_{iid,robust,clustered}_oracle.rs`, `src/ppml/psd.rs`,
`tests/python/ppml_multiway_cluster_oracle.py`, Stata weight/VCE fixtures.

## 11. Fit statistics and posted results

OLS RSS is the reference-weighted residual sum of squares; TSS uses centered
y when a structural intercept exists and raw y otherwise. Within TSS is the
FE-projected outcome norm. R² and within R² are `1−RSS/TSS` using their
respective denominators. Adjusted R² is
`1−(RSS/fit_df)/(TSS/(N_eff−1{structural intercept}))`; adjusted within R²
uses within TSS and baseline DoF `fit_df+K_f`. RMSE is `sqrt(RSS/fit_df)`.
Preserve the corresponding Gaussian ll/ll_0, model F/Wald, MSS and undefined
statistic behavior; zero denominators yield documented missing/absent fields,
never invented zeros or infinities. The cluster fit DoF is distinct from the
reported test DoF (§9). The OLS model F keeps two Cholesky pivot policies on
purpose. IID covariance is `s²B` on rank-certified coordinates, so a
nonpositive slope pivot contradicts the rank certificate and is an error.
Robust and clustered covariance may be legitimately singular (for example
G < K); a pivot within the certificate tolerance of the largest slope variance
leaves F missing, and a clearly negative pivot is an error.

PPML deviance is `2 sum q_i[mu_i−y_i+y_i log(y_i/mu_i)]`, with zero-outcome
contribution `2q_i mu_i`. Log likelihood is
`sum q_i[y_i eta_i−mu_i−lgamma(y_i+1)]`, including fractional outcomes.
The reference null mean is `sum(qy)/sum(q)` irrespective of FE/offset;
ll_0 uses that mean and pseudo-R² is `1−ll/ll_0` when defined. Publish outer
and inner counts, halvings and convergence. Do not post OLS RSS/TSS/MSS,
R²/adjusted R², RMSE, F or e(df_r) for PPML.

Both estimators preserve e(b), e(V), e(sample), coefficient/equation stripes,
model/command/weight/cluster metadata, effective and physical N, candidate/full
counts, ranks, absorbed metadata, singleton accounting, prediction binding
and requested output names. Sample/status/count/DoF fields are exact.
Required OLS families include `fereg_status`, `fereg_dof`, `fereg_fe`,
`fereg_cluster`, `fereg_subset`, `fereg_solver`, `fereg_certificate` and
`fereg_runtime`. The OLS solver matrix retains its 2x16 layout and numeric codes.
Rows `absorption` and `residual` describe outcome/regressor absorption (aggregated
across Low blocks) and final residual projection. The `used` column reports the
actual certifying method: automatic two-FE Schur PCG is code 4, and actual CMG
application on that path is code 8. A MAP continuation is code 2 with fallback
set; already-certified zero-work inputs retain code 2 with zero iterations.
Low aggregation preserves CMG use across blocks unless a certified fallback
takes precedence. Requested solver remains unchanged. The public macros
`fereg_solver_used` and `fereg_solver_residual` expose the method names;
`fereg_solver_fallback` is the OR of the two fallback fields. This reporting
correction changes only method/fallback telemetry, not arithmetic, route
decisions, iteration budgets, science or the PPML solver contract.

Required PPML information includes:

- N_physical, N_effective, N_candidate, N_full; rank, feppml_model_rank,
  df_m, df_resid, df_structural, df_a, N_hdfe, N_hdfe_extended;
- deviance, ll, ll_0, r2_p, mu_0, sum_y, ic, inner_iterations,
  step_halvings, converged; chi2/df_chi2 when defined;
- num_singletons, drop_singletons, separated, sumweights and applicable
  raw_sumweights/weight_multiplier and logarithms; N_clustervars,
  N_clust1 through N_clust5, N_clust and G_min when clustered;
- feppml_status, fit, dof, fe, cluster, subset, separation, inference,
  drop_counts, separation_sequence, certificate, solver, runtime and receipt
  matrices (all names after the first have the `feppml_` prefix);
- `dof_table` when applicable; requested/selected/effective/checkpoint solver
  macros, concrete engine identity, fallback count and reason;
- `cmd=feppml`, `model=poisson`, command line, depvar, indepvars, vce, absvars,
  separation, adjustment, predict=feppml_p, requested output bindings and
  bias-correction results when requested (§15).

Retain the existing result layouts and native protocol. Any
telemetry simplification needs a scoped compatibility change; scientific fields
and solver identity/failure evidence remain required. Preserve runtime input/native/total measurements
needed by the benchmark suite, with defined boundaries. Undefined, unavailable
and absent remain distinct; validators reject stale/malformed matrices.

Tests: `parity_reghdfe/`, `parity_ppmlhdfe/feppml_scientific_contract.do`,
`estimate_ppml_runtime_validator.do`, native result-wire and statistic tests.

## 12. Outputs and prediction

Requested observation outputs use original-row identity and are missing
outside e(sample), except `dropreason()` which covers the whole dataset.
Do not retain hidden O(N) prediction caches when outputs are not requested.
All outputs belong to the same estimation transaction.

OLS predict defaults to xb. xbd, d and residuals require the estimation's
retained residual variable: xbd=y−r and d=xbd−xb. Missing retained state is
r(498); deletion of its named variable is r(111). `test` uses posted b/V;
`estat summarize` uses the estimation sample.

PPML xb includes the current finite predictor and offset/log exposure;
eta=xb+d and mu=exp(eta). Predict defaults to mu and intersects its own
if/in with e(sample). No-FE d is zero and all predictions are reconstructible.
With FEs, d/absorbed requires retained d. Eta/xbd, mu and residual predictions
require compatible retained d, eta or mu, otherwise r(498) before target
creation. Retained d is authoritative and combines with current xb; absent d,
eta then mu are snapshot fallbacks and do not track edited covariates.
Deleting the authoritative variable is r(111). Duplicate alias pairs or
multiple statistics are r(198). Estimation eta()/xbd() are aliases too.

| PPML residual type | Unweighted per-observation value |
|---|---|
| response, score | y−mu |
| pearson | (y−mu)/sqrt(mu) |
| working | (y−mu)/mu |
| deviance | 2[mu−y+y log(y/mu)], with zero-y limit 2mu; nonnegative contribution |
| anscombe | 1.5(y^(2/3)−mu^(2/3))/mu^(1/6) |

`residualtype()` requires `residuals()` and defaults to response.
Full-axis drop codes are 0 retained, 1 if/in, 2 missing outcome, 3 regressor,
4 weight availability, 5 FE, 6 cluster, 7 adjustment, 8 singleton,
9 FE separation, 10 simplex, 11 IR, 12 Mu, 13 other model exclusion.
Invalid retained domains cause errors, not successful drop codes. Report all
14 counts; separated sums 9–12. No dropped output can masquerade as a fitted
zero. Named FE channels follow §5; bias channels follow §15.

Tests: `postestimation/fail_closed.do`,
`parity_reghdfe/estimate_ols_postestimation_parity.do`,
`parity_ppmlhdfe/feppml_separation_outputs.do`, saved-output/alias fixtures.

## 13. PPML family, starts and IRLS

The model is eta=offset+Xb+Dalpha, mu=exp(eta), score q(y−mu).
Nonnegative fractional outcomes are valid; negative finite candidates fail
r(459). Empty/all-zero weighted final outcomes fail r(2001). PPML always has
a structural constant; an absorbed constant is not counted twice.
Offset/exposure are mutually exclusive (r(198)); missing excludes, finite
offset is required, and exposure must be finite positive (r(459)). Log
exposure exactly once. Adjustments affect fit/prediction but not the
homogeneous separation cone.

Start from `c0=log(sum qy)−log(sum q exp(offset))` using stable logarithmic
sums. Analytic/group/coordinate/slope start policies may improve this initial state. A declined
or invalid start proposal restores a finite global state. Starts never certify
rank, optimality or the final Hessian. Do not clip eta/mu to artificial bounds.

At an accepted state h=qmu and z=eta−offset+(y−mu)/mu, with exact zero-y
expression eta−offset−1. Stable evaluation may use exp(log(y)−eta).
Full Fisher steps are tried first; halve coefficient/predictor steps together
until values are representable and deviance nonincreasing within registered
roundoff slack. Exhausted steps or iteration budgets are fatal.
Relative deviance change is `abs(D_new−D_old)/max(min(D_new,D_old),0.1)`.
Require two consecutive accepted passes satisfying this and independently
recomputed normalized finite and FE scores. A small objective change alone
is insufficient, particularly near small coefficients.

Apparent convergence triggers fresh strict WLS at
`min(1e-12,0.1×outer_tolerance)`. A publication fit first waits while its
contracting predictor is predicted to keep moving: with the strict profile
envelope `min(outer_tolerance, score_tolerance, 1e-8)` and the relative eta and
coefficient movements `m_k` of the last two accepted iterations, it enters
once `m_k^2/m_(k-1)` is at most the envelope. A contraction ratio
above one half, fewer than three remaining iterations or eight such
deferrals enter at once. Weakly identified FE directions can pass the deviance
and score tests while eta still moves; the wait changes only when the
unchanged strict check runs. If beta, eta, basis/rank or objective moves
beyond the registered publication envelope, resume bounded IRLS. Publish
only after exact accepted-h=qmu Hessian/bread, basis, projection, score,
objective replay and eta reconstruction checks. Refresh/invalidate all
current-weight operators, products and proofs whenever weights change.
Separately typed historical preconditioner state may survive solely as solve
acceleration, with its build identity retained and compatibility, memory and
work limits checked. It carries no current-weight, certificate, canonical-rank
or Hessian authority; the operator, RHS, stopping target and independent
certification still use the current problem. The two-FE Auto CMG hierarchy
(§1) is such state. Sample mutation invalidates all reuse. Warm starts are guesses, never certificates. Final inference and correction
use this same accepted state. Output tolerance (§5) does not remove the
strict PPML/final-Hessian requirements.

Tests: `src/ppml/family.rs`, `fereg-oracle/tests/seeded_dense_ppml_differential.rs`,
`tests/python/ppml_explicit_dummy_oracle.py`,
`parity_ppmlhdfe/feppml_one_fe_dummy_oracle.do`,
`feppml_multipartition_slope_oracle.do`, final-Hessian/line-search fixtures.

## 14. PPML separation and ledger

The ledger alone owns deletion. Detectors read immutable generation views
and propose sorted unique original-row IDs plus method/status/counts and a
certificate. Validate bounds, membership, uniqueness, eligibility and all
certificate fields before accepting any row; one invalid row rejects the
whole proposal. Detector traversal order must not choose public drop reasons.

A numerical recession direction z lies in the finite-plus-FE span, is zero
on positive outcomes, nonpositive on zeros, and strictly negative on every
proposed row. Independently check span distance, normalized equality/sign
margins, nonzero scale and finiteness. Exact zero-only intercept-FE levels
use a combinatorial certificate. Slope-only channels do not prove that rule.
Tiny fitted means, extreme eta and stagnation are not deletion evidence.

Accept nonrepeated method subsets in canonical `fe simplex ir mu` order;
default is `fe simplex ir`, mu is opt-in, and `none` alone disables proactive
separation only. Each round prunes singletons, applies requested exact FE
zero-level pruning to a fixed point, then simplex, IR, optional Mu. After a
certified deletion, apply its singleton cascade and requested FE fixed point,
commit the union as one generation, compact all sources together, rebase
topology once and restart the chain. First certified reason in canonical
method order wins; singleton counts remain separate.

Full-model simplex includes finite columns, every FE level and slope channel
in its explicit width; checked capacity includes design, null-space/tableau,
topology and certificates. A capacity skip is permitted only if later requested
comprehensive IR on the same generation certifies no unhandled separation.
A simplex-only or unresolved over-cap request fails r(909). An IR drop
restarts the chain and cannot certify a prior generation's skipped simplex.
Requested/default IR nonconvergence or uncertified output is fatal. Mu needs
a fitted start and the same independent cone certificate; accepted Mu drops
discard that entire fitted session.

An admitted detector projection on exactly two categorical intercept effects
is solved with the OLS two-FE Auto route (Johannes, 2026-10-28, beta.10);
other admitted projections, and the developer comparison arm
`inject=ppml_separation_cg`, use retained-product CG. The FE tolerance, the
finite score tolerance, the rank, FE-moment, score and reconstruction
certificates, the memory budget and every detector weight, recurrence,
iteration limit, sign and threshold rule are the same on both routes. An
Auto-routed step that fails a numerical certificate takes no two-FE PCG MAP
continuation and runs once more with retained CG under the same admitted
budget; from there it is exactly a retained-CG step (below). When that fails
as well (its LSMR re-execution included), the Auto step runs once more, and
this time a two-FE PCG certificate miss re-certifies its rejected candidate
with each FE level's mean formed about the level's largest-weight member
instead of its first member, at the same scales and tolerance, publishing the
candidate if that passes (Johannes, 2026-10-28; seeded census seed 3, where a
zero-outcome first member of weight 1e-18 and residual of order one floors the
shared certificate near the tolerance while the exact moments are 1e-18 or
below). Otherwise the retained-CG step's error is reported. Numerical scope of
the re-check: only Auto detector steps whose every attempt failed a numerical
certificate can change, from that error to a certified result; every fit that
completed or failed differently before is bit-identical, and the shared FE
certificate, the retained-CG arm and every other solver are unchanged.
Numerical scope of the activation: the flagged separated set, the final sample, coefficients and VCE
are unchanged on every witness; the rectifier may certify its terminal
fixed point at an earlier iteration, so `RSEP` iteration counts (column 5)
and certificate magnitudes (columns 9-12, zero-based 8-11) and `RSEQ` total
iterations may change, and with them the separation trace digest (`RREC`
zero-based positions 28-29).

A retained-CG projection solves the weighted normal equations. When a
detector step that used it fails a
numerical certificate (NonConvergence or InvalidData: a simplex deletion's
certification, the rectifier's detection and deletion certification, or the
Mu candidate and its certification), the step runs once more with LSMR, the
public `lsmr` solver family and preconditioner policy, under the same
weights, tolerances, iteration limit, decision rules and certificates
(Johannes, 2026-10-27: fix the public detector failures). CG squares the
rectifier's zero-row weight ratio (`tolerance^2 / zeros`), so on weakly
connected or heavily zero designs its zero-row fitted values can leave the
model span; LSMR works on the weighted operator itself. Its workspace is
bounded on generation zero and charged to the memory plan only when the step
is re-executed; a step that certifies on its first attempt is never
re-executed. If the LSMR workspace does not fit the plan or the re-execution
also fails a numerical certificate, the first error is reported; Break,
allocation and internal errors of the re-execution propagate. A terminal
no-drop result is replayed with the solver that produced it. Numerical scope:
only fits that previously failed with r(430) or r(459) in such a step can
change, from failure to a certified result; every fit that completed before
is bit-identical.

A terminal no-drop result is checked before it is reported. For an IR
fixed point (Johannes, 2026-10-09) the check uses the stored recurrence and
projection residual: both are finite; the recurrence is nonnegative, zero
on positive outcomes and unit-scaled (or identically zero with a zero
residual, the vanishing projection); the stored fitted values reproduce the
reported sign certificate exactly; the reported iteration count is in range
and carries no exact-FE certificate; and every kept regressor and FE
intercept or slope channel has a zero weighted residual moment, relative to
the channel norm and the larger recurrence or residual norm, within the
certificate tolerance. These denominators are at least as large as the
projection's own certificates use, so this is an independent recomputation,
not a stricter test. If
any part fails, the detector is re-executed deterministically, as it is for
simplex and Mu no-drop results. Like the projection's own certificates, this
weighted check only weakly constrains fitted values on zero-outcome rows.

No prior-generation projected design, warm state, rank, cluster subset,
bread or output is reusable. Recompute canonical finite rank on the final
sample and verify positive weighted outcome. Report requested/executed
sequence, method status/iterations, generation identities and deterministic
trace digest independently of timing. Bound public diagnostic tables; no
observation-sized trace crosses the result wire. Separation proves a sample
decision, not estimator convergence.

Tests: `src/ppml/prepared_separation.rs` (re-execution: `prepared_separation/retry`,
re-certification: `prepared_separation/recertify`),
`fereg-oracle/tests/ppml_separation_primers.rs`, `fereg-oracle/tests/ppml_separation_routing.rs`,
`tests/python/test_ppml_recession_cone_primers.py`,
`tests/python/ppml_recession_cone_oracle.py`,
`parity_ppmlhdfe/feppml_separation_methods.do`,
`feppml_separation_outputs.do`, full-model simplex capacity/oracle fixtures.

## 15. PPML coefficient mean bias correction

Retain `biascorrection(exact|randomized)` as an optional calculation on the
unchanged accepted fit. Require exactly two categorical intercept FE sets,
ordinary numeric controls, explicit numeric `biasunit()`, and none/fw/pw
weights; offset/exposure are allowed. Slopes, finite factors, other FE
topologies and multiway bias-unit dependence are unsupported. The score unit
comes from the declared dependence experiment, not VCE cluster settings.

Every retained level of both FE sets must contain at least two distinct units,
even without `biasfe()`. Count actual IDs, never FW copies. A missing unit on
a retained row is an error, not a sample exclusion. Any outcome-dependent
separation deletion rejects the entire correction transaction. Unit relabeling
does not change support; restrictions can change it. More probes, exact
calculation or splitting dependent units cannot establish valid independence.
This necessary support screen does not certify statistical adequacy.

For complete identified FE-plus-control row x, fitting weight a, accepted mu
and independent unit g, compute

```text
S_g = sum_g a x (y−mu)        J_g = sum_g a mu x x'
J = sum_g J_g                K = sum_g S_g S_g'
V = J^-1 K J^-1
c_D = J^-1 sum_g J_g J^-1 S_g
c_C = (1/2) J^-1 sum_rows a mu x (x' V x)
beta_bc = beta_hat + c_D + c_C
```

Both increments include all identified nuisance coordinates. Do not substitute
the public VCE, add HC1/DoF/PSD corrections or cap coefficients. This empirical
second-order mean correction is an approximation for interior fits; it is not
a correction for confidence intervals, predicted rates or effect variances.
Ordinary e(b), e(V), objective, sample, means and predict remain unchanged.
No leave-one-unit-out refits or observation selection enter this calculation.

Exact mode streams units without storing K, V or all scores; retain limits of
10,000 units and one billion observation-unit contributions. Randomized mode
has a fixed prespecified B≥2 (default 256), seed 739391 and stream 0;
seed/stream/probe controls are unsigned 32-bit integers. Exact mode rejects
explicit randomization controls. Ancillary bias options require correction.

Use Philox4x32-10 with key=(seed,stream), counter=(low/high IEEE-754 numeric
unit bits, low/high zero-based probe index). Normalize −0 to +0. The first
output word's low bit maps to −1/+1. One sign per unit/probe is shared across
both increments; no Stata RNG mutation. Signs are stable under row ordering,
grouping, worker count and route; renumbering units changes randomness.
Retain pinned Random123 known-answer vectors from the source contract.
Their source is commit `d8a0c25e49992685c0b9d91b553c1d04f81b6b4b`,
`DEShawResearch/random123/tests/kat_vectors`, SHA-256
`aab5ebabf40003f63d6d87b24cbd2c8a02652e00cf8bad64226fd50586929183`.

Point calculation uses B probe solves plus two final solves. Up to eight
predetermined independent groups use the same probes and probe-count weights;
at most fourteen extra final solves estimate numerical variation. For group
sizes m_j and increments d_j,c_j, covariance of the full means is
`sum m_j(d_j−dbar)(c_j−cbar) / [B(groups−1)]`. Analogous squares give each
variance; total variance includes twice covariance. These are conditional
Monte Carlo diagnostics, not sampling standard errors. Exact mode reports
zero probe uncertainty, not zero solver error.

Anchor the smallest canonical second-FE level to zero in each component and
put the component constant in the first FE. Apply that same linear map to raw,
derivative, curvature and corrected channels; do not Hessian-center each
channel differently. Recover raw effects from accepted eta minus offset and
unchanged finite contribution. Independently check row reconstruction,
including tiny means and disconnected branches. Ordinary saved FEs retain
their separate existing normalization.

Publish ordinary-stripe matrices `b_bias_raw`, `b_bias_d`, `b_bias_c`,
`b_bias_total`, `b_bc`, `bias_mcse_d`, `bias_mcse_c`, `bias_cov_dc`,
`bias_mcse_total` under e(). Omitted and normalized absorbed-constant slots
are zero. `biasfe(stub)` creates `stub_1_*` and `stub_2_*` with suffixes
raw,d,c,total,bc,sed,sec,covdc,setotal, missing outside e(sample).
Numerical SE channels are tied to this anchor; omitted cross-level covariance
prevents arbitrary SE renormalization.

Retain the 21×1 `e(bias_diagnostics)` named fields: method, units, probes,
seed, stream, independent groups, solves, maximum solve score, maximum FE
reconstruction error, seconds, workspace bound, maximum absolute total
increment, minimum/maximum mu, minimum/maximum rows per unit, route, validity,
certificate tolerance, minimum level information and maximum FE-leverage
lower bound. The last is max row h divided by its FE-level information, across
both effects; it is not the full leverage or unit-information concentration.

`e(bias_support)` has rows fe1/fe2 in absorb order and columns levels,
min_units,p25_units,median_units,p75_units,max_units,single_unit_levels,
single_unit_fraction,min_info_units,median_info_units,max_unit_info_share.
Quantiles are unweighted nearest rank across levels. With
I_level,g=sum a×mu and p_g=I_level,g/sum_g I_level,g, information equivalents
are `1/sum p_g²`; these are not certified effective sample sizes. Successful
single-unit counts/fractions are zero. `bias_validity=interior-retained-sample`
and numeric valid=1 mean necessary support and numerical checks only.

All solves use the accepted full-system basis and final a×mu, with independent
normal-equation and reconstruction certificates. Reuse bounded workspaces and
one worker team; count all probe/recovery work. Option-off allocates/solves none
of this. Missing IDs, support, rank, allocation, solve or Break failures roll
back the whole estimate. Error diagnostics identify channel, block/probe and
recovery discrepancy without posting partial matrices.

Tests: `tests/python/test_ppml_bias_oracle.py`, `src/ppml/bias*.rs`,
`parity_ppmlhdfe/feppml_bias_correction.do`,
`bias/final_recovery_precision.do`, duration reconstruction and Philox vectors.
Retain both increments, exact/randomized and support-refusal tests.

## 16. Certificates and errors

Every FE backend reconstructs r=v−Dalpha explicitly. Independently validate
finite coefficients, row reconstruction and D'Wr. Use the invariant
weighted local-span certificate: for each level's orthonormal basis Q_g of
sqrt(W)D_g, measure `||Q_g' sqrt(W) r_g|| / s`, where
`s=max(||sqrt(W)v||,||sqrt(W)r||)` for that RHS. A zero scale accepts only
zero projections. The maximum across effects/levels/RHS must meet tolerance;
retain raw moment and aggregate norm diagnostics. Recompute from declared
design/weights, not a recurrence's own residual or stale weighted factor.
Local rank-aware bases prevent slope scaling/rotation from changing the span
test. Residual reconstruction and small D'Wr are necessary, not a universal
forward-accuracy bound; §20 independently tests weak graphs.

A derived certificate applies to a residual formed as a linear combination of
certified projections, `r = y~ - X~b`. The moments are recomputed from the
declared design and weights as above, with `s = s_y + sum |b_j| s_j`, each term
the scale at which its column was certified (the larger of its raw and within
weighted norms). By linearity a certified projection cannot exceed this bound,
so the check verifies the combination without a second solve. The two-FE owner
recomputes the moments on its retained workspace; the one-FE and many-FE
binding routes, which retain no owner, recompute them for every absorbed effect
on a one-RHS workspace over the same borrowed design, admitted against the
solver memory budget. Neither reads a solver's own residual norms.

OLS additionally certifies finite scores, bread/inverse, fit and covariance.
PPML additionally certifies model-space feasibility, scores, accepted objective,
final Hessian/rank, output reconstruction and any bias calculation. Finite but
uncertified candidates cannot be posted or relabeled as convergence.

| Failure class | Return code |
|---|---:|
| Syntax/removed option or duplicate incompatible syntax | 198 |
| Unsupported/invalid native argument or unavailable prediction state | 498 |
| Protocol mismatch | 499 |
| Unsupported weight | 101 |
| Invalid FW / negative AW | 401 / 402 |
| Invalid ordinary factor on the available sample | 452 |
| Empty/insufficient observations or required residual DoF | 2001 |
| Invalid data | 459 |
| Numerical nonconvergence | 430 |
| Allocation/checked size overflow | 909 |
| User Break | 1 |
| Self-test failure | 3498 |
| Internal failure/caught panic | 3499 |

Preserve more specific Stata binding errors (for example deleted variable 111)
and pinned fixture error precedence; a failed covariance validity check is
not successful missing inference. No errors are converted to a different
estimator, different sample or silently weaker tolerance.

Tests: `tests/fe_certificate_oracle.rs`, `tests/weak_graph_decimal_oracle.rs`,
`tests/prepared_fit_oracle.rs`, native error/wire tests, public rollback fixtures.

## 17. Transaction, transport and Break

One substantive plugin call owns estimation. Stata parses syntax/metadata,
holds prior estimates, creates private shadows and publishes only after native
completion and independent bounded transport validation. Rust loads observations,
prepares the sample, estimates, certifies and validates all N/K/K² products
before the first SPI store. All source/output identities, dimensions and
nonaliasing bindings are checked before observations. SPI calls and Break
polling occur only on Stata's main thread; workers touch Rust-owned memory.
Native work from preparation through derived outputs and saved-FE recovery
runs under Break polling, and long serial row passes checkpoint at a fixed
row stride.
Catch Rust panics at FFI; no unwind crosses C. Safe crates forbid unsafe code.

COMPLETE is the last native operation; no allocation, polling, display,
computation or store follows it. A partial physical SPI write has no logical
meaning. The final ado commit protects posting and public output renames.
Any error, Break, store failure, bad receipt or missing helper restores prior
e(), named/null estimates, data, order and output names. No correction or
postestimation path makes an extra substantive estimation call.

Retain independently versioned exact estimation handshakes. Current
versions are action 1.6, model 2.1, OLS result 1.5, PPML request 1.8, PPML
result 1.5, ado identity 0.5.0 and SPI 3.0. Diagnostics accept same-major,
not-newer-minor action callers. Model 2.0 is readable only for the bounded
no-absorbed-metadata shape;
unknown/newer identities and malformed framing fail before observation access.

Use canonical byte-length/count framing inline through 4,000 payload bytes;
larger requests use only `bundle_local=<byte-count>` and fixed caller-local
`fereg_request_bundle` (SPI name `_fereg_request_bundle`). No arbitrary macro
lookup. Cap native metadata at 16 MiB, reserve extra content and NUL bytes,
and validate exact NUL position, UTF-8 and closed decoder before observations.
Drop transport storage after decoding. Metadata-only Mata framing preserves
UTF-8 byte lengths and cleans all temporary externals independently on error.

Bounded compiled Mata validation/publication may remain in the control plane;
no observation-scale design/estimation work enters Mata. If retained, ship
objects compiled by Stata 18 and verify source/object identity. A `version 18`
statement in a newer compiler does not suffice. Missing/stale helpers fail
atomically; no runtime source-compilation fallback.

Tests: `tests/mock_spi/`, native FFI transaction tests, `tests/licensed_stata/`,
`parity_ppmlhdfe/estimate_ppml_hidden_transaction.do`,
`estimate_ppml_runtime_validator.do`, installation and framing failure fixtures.

## 18. Threads and repeatability

Honor threads(); 0 resolves the existing native default. Preserve repeatability
at a fixed thread count with deterministic and fixed random seed/stream.
Across worker counts require scientific agreement under §20 and stable sample,
omission, separation and other discrete model decisions. Bitwise identity
across different worker counts, platforms or processor features is not required.
Processor-dependent fast paths, including runtime FMA dispatch, may select the
fastest supported arithmetic when the same independent certificates and §20
scientific witnesses accept their results and discrete decisions stay stable.
The portable path remains supported and tested. Same-machine repeatability at
a fixed worker count and seed remains required. The deterministic option
remains supported. Low memory mode is one such case: when its scratch moments
are split by observations, the partial Gram matrices follow the planned worker
count, so the last bits of Low results may differ across worker counts while
Fast results do not (accepted by Johannes, 2026-10-09).

threads(n) is an available worker budget. Stages may use fewer workers under
general work/resource rules, with transparent requested, resolved-budget and
effective concurrency. No-numerical-change commits preserve scientific bits and
available solver/route signatures against the parent under equal build settings.
Numerical fixes declare affected cases/fields; unaffected science remains exact.
Exclude timing, build identity and environmental resource observations only.
A tolerance pass cannot silently waive an unexplained exact-change failure.
Preserve Stata processor restoration on success, error and Break.

Every additive-Schwarz preconditioner fereg builds in the pinned solver (OLS
borrowed and owned LSMR, the many-FE Jacobi trial's escalation, explicit
`preconditioner(schwarz)` and the PPML joint Schwarz factor) uses the Ordered
reduction, which commits subdomain contributions in subdomain order, in every
mode; the pinned Auto reduction may resolve to an atomic scatter whose
summation order follows thread timing. Before beta.10 only `deterministic`
selected it. Declared scope: default-mode fits whose LSMR runs on the additive
preconditioner (many-FE OLS Auto fits whose Jacobi trial escalates, explicit
Schwarz LSMR and PPML LSMR) may change coefficients, covariance, fit
statistics, residuals, FE coordinates and iteration signatures in low bits
within §20; `deterministic` fits and discrete decisions are unchanged.
Witness: an escalating default-mode many-FE fit repeats bit for bit at one,
two, four and eight workers.

The two-FE OLS Auto team installation, the parallel cell construction and the parallel cell product
follow this rule: their results do not depend on the worker count, apart from
CMG's fixed-chunk centering of a single coarse component of at least 131,072
entries, which uses the same tree whenever a team is installed.

Scoped changes may deliberately change algorithms, arithmetic order and storage. Declare affected outputs, routes and
errors before implementation; require independent scientific acceptance and
unchanged discrete model semantics. Unaffected outputs remain exact, and pure
refactors retain exact route/bit checks. No post-hoc parity exception is implied.

## 19. Memory behavior

Preserve supported memory modes, admission policy and public controls.
Preserve checked sizing, fallible allocation, r(909), ownership cleanup and
transaction atomicity. Reuse existing allocation/failure tests; no new exhaustive
memory-audit framework is required. The many-FE LSMR route transports each RHS window
from the row-major projection input only while its sessions start, releases
the column-major copy once each session owns its response, and transports the
columns again, bitwise, for an escalation or a preconditioner retry; OLS Fast
releases its projected matrix after the finite solve, which no later stage
reads. Both are arithmetic-preserving storage-lifetime changes. Admission
totals are unchanged (they still charge the released storage), so
`memory(auto)` chooses Fast or Low exactly as before. Estimates remain estimates, not process RSS
or strict OS bounds. No new route-specific admission fences may be added to
hide a performance regression. Future removal of a mode needs mapped coverage.

CMG admission assumes CMG's default hierarchy cap of 32 levels (CMG `5c5b5b2b`;
previously 128). Every CMG construction, workspace and plan bound fereg admits,
including the serial construction bound that two-FE OLS Auto keeps charged as
retained CMG storage, provisions at most 31 coarse graph copies instead of 127.
A hierarchy that would need more levels stops at the cap with CMG's iterative
`MaximumLevels` terminal inside the admitted bytes; its first 31 levels are
unchanged. Hierarchies of at most 32 levels are bit-identical. The lower bounds
can admit a CMG build, cell kernel, cell cache or interleaved application that a
tight memory limit refused before; an admitted route keeps its certificates.
See the [memory record](../bench/results/cmg-admission-level-cap-2026-10-27.json).

## 20. Accuracy tolerances and independent evidence

For well-conditioned comparisons, require coefficients rtol=1e-7/atol=1e-10,
SE/VCE rtol=1e-6/atol=1e-10, and residual/fitted values
rtol=1e-7/atol=1e-9. Preserve stronger named regression thresholds, including
the 2e-8 CMG dense-oracle check. Exact equality is required for applicable success/error, sample, omission/
status, VCE/DoF policy, clusters, singleton accounting and discrete scientific
metadata, except named independently justified bug-fix differences. Runtime tolerance is not permission to weaken these
independent accuracy checks. Adjudicate nonunique FE coordinates through
identified combinations, fitted values, residuals and objectives.

Retain the sealed 100-digit `tests/reference/weak_graph_decimal_oracle_v1`
CSV/JSON with original identity. Test chains, barbells, AKM weak mobility,
disconnected graphs, unequal weights, slopes and multiple RHSs against the
explicit design, with one grounded coordinate per component. Exercise every
supported backend, not just auto. Preserve the CMG raw-residual regression
and the independent witness that FE moment certification alone can accept a
wrong CG result (`fereg-oracle/tests/seeded_dense_ols_differential.rs`). Large LSMR stability comparisons include a 100-times-tighter
solve; reference residual distance is diagnostic when independent accuracy
and runtime certificates pass, while exact fields and coefficient/VCE/fit
envelopes remain binding.

Pinned reghdfe/ppmlhdfe closures in `references.lock.toml` are compatibility
references; dense weighted dummy OLS, Poisson, sandwich and recession-cone
oracles remain independent authorities. For PPML the pinned comparator's
reported ll/deviance are the reference objective. Its predicted state is an
additional witness only if independently feasible and stationary; projection
alone is insufficient. Fereg's own mean must replay its objective and pass
all score/model-space/final-Hessian checks. A reference-error exception needs
an independent optimum enclosure proving the discrepancy; it does not alter
runtime acceptance or rewrite historical failed pairs.

Performance workflows never weaken science. Missing or timed-out comparisons
are not successes. Licensed checks require application return codes and terminal
sentinels in addition to process status. Tests include the sealed weak-graph
witness, independent Python oracles, actual-plugin replay and native/Stata parity
families above. Current qualification and its limits are in [VALIDATION](VALIDATION.md).
