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
the CMG bytes this scope admits and charges as retained. Qualified OLS `solver(auto)`
without `precondition()` (Auto/Auto below), a global intercept and exactly two categorical-intercept
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
Stata 15 is the minimum host. Without a variable list, `fereg` and `feppml`
replay their own last results (Stata's standard display options only), which
`estimates replay`, `bootstrap:` and `jackknife:` require; other options without
a variable list fail with `r(198)` and a missing result with `r(301)`. Both
commands post `e(fereg_version)`, the release label. The installed diagnostic is
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
| `solver(method[, sub])` | auto (default), map, lsmr, cg; sub-options `reduce()`, `precondition()`, `accelerate()` and the CMG-Schur controls below | auto (default), lsmr, cg; `reduce(auto\|full)`, lsmr `precondition(auto\|schwarz)`, cg `precondition(auto\|cmg)` |
| `solver(cg, reduce(eliminate) precondition(cmg) cmgdegree(#))` | Experimental CMG-Schur only; 4, 5 (default), or 6 | Unsupported |
| `solver(cg, reduce(eliminate) precondition(cmg) cmgsamples(#))` | Experimental CMG-Schur only; integer 0 (default) through 4294967295 | Unsupported |
| `solver(cg, reduce(eliminate) precondition(cmg) cmgseed(#))` | With positive cmgsamples only; integer 0 (default) through 9007199254740991 | Unsupported |
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

Preserve the supported solver routes, iteration options, poolsize(), memory()
and deterministic. `solver()` names the method first (`auto`, `map`, `lsmr`,
`cg`) and then, after a comma, the reduction (`reduce(auto|full|eliminate)`),
preconditioner (`precondition(auto|none|jacobi|block|schwarz|cmg)`) and MAP
acceleration (`accelerate(none|irons-tuck|squarem)`); the three choices are
independent. `solver(auto)` chooses all of them and takes no sub-option other
than `auto`. The supported explicit OLS requests are:

- `map`, with or without Irons–Tuck or SQUAREM acceleration;
- `lsmr` with `precondition(auto|none|jacobi|schwarz)`, modified LSMR on
  fedemean's full system, with or without absorbed slopes (with slopes,
  `jacobi` inverts each level's channel block and `schwarz` runs the pair
  subdomains over all channels described below);
- `cg` with `reduce()` and `precondition()` as in the table below; every
  preconditioner other than `cmg` runs fedemean's PCG on categorical intercept
  designs (no slopes);
- `cg, precondition(cmg)` on the full graph (the explicit CMG route) and
  `cg, reduce(eliminate) precondition(cmg)` (the experimental CMG-Schur route,
  the only one that takes the CMG-Schur controls).

| `solver(cg, ...)` | `reduce(auto)` | `reduce(full)` | `reduce(eliminate)` |
|---|---|---|---|
| `precondition(auto)` | elimination (2–5 effects), else full | Jacobi escalating to Schwarz | Jacobi escalating to block-Jacobi |
| `precondition(none)` | full | no preconditioner | r(198) |
| `precondition(jacobi)` | elimination (2–5 effects), else full | Jacobi | Jacobi |
| `precondition(block)` | elimination | r(198) | block-Jacobi from the first iteration |
| `precondition(schwarz)` | full | additive Schwarz | r(198) |
| `precondition(cmg)` | CMG graph PCG | CMG graph PCG | CMG-Schur (two effects) |

Sub-choices left at `auto` resolve to a supported formulation of the explicit
method; only the automatic preconditioners escalate. feppml accepts `auto`,
`lsmr` (`precondition(auto|none|jacobi|schwarz)`, §13) and `cg`
(`precondition(auto|cmg)`) on the full system. CG-accelerated MAP (`accelerate(cg)`) remains rejected with r(198)
under the existing safety repair. Every other combination returns r(198) naming
an alternative and is never substituted. Explicit fedemean requests fail
closed: non-convergence within `iterate()` is r(430)-class non-convergence, a
workspace beyond the memory limit an allocation refusal, and `reduce(eliminate)`
outside two to five effects is unsupported. Explicit LSMR/CMG and both PPML
mean-bias modes are required.

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

`within`, formerly the pinned modified-LSMR library, was never a public solver
and is no longer a dependency (retired in Milestone 3, 2026-11-09). Every
modified LSMR is `mlsmr` from the pinned `schwarz-precond` crate (the same
revision as before), run by fedemean's full system and, for PPML's explicit
joint `[X D]` recurrence (§13) and the developer LSMR Break hooks, by
fereg-core directly. All supported backends must honor the same scientific
model and accuracy contract. Shared internal operations preserve explicit solver and storage support;
unsupported requests fail clearly.

| Request | OLS support to preserve | PPML support to preserve |
|---|---|---|
| `auto` | No FE, one/multiple intercept or slope-bearing FEs, disconnected designs | Same model grammar with changing positive IRLS weights |
| `lsmr` | General supported FE grammar, including slopes and multiple RHSs (fedemean's full system) | At least two categorical intercept FEs, including more than three; no absorbed slopes |
| `cg` (fedemean PCG, full or eliminated) | Categorical intercept effects, any count for the full system, two to five for the elimination; weights, finite controls, disconnected designs, saved effects, both memory modes; no slopes | Unsupported (PPML `cg` is CMG) |
| `cg, precondition(cmg)` (full graph) | Categorical intercept graph: two largest effects as bipartite core, additional effects with at most 64 non-base nuisance columns; no slopes | Exactly two or three categorical intercept effects; third-effect nuisance width at most 64; no slopes |
| `cg, reduce(eliminate) precondition(cmg)` (CMG-Schur, experimental) | Exactly two categorical intercept effects; positive observation weights, finite controls, disconnected components and saved effects | Unsupported |

The native request wire carries route tokens (`auto map lsmr pcg cmg cmgschur it
squarem` with `preconditioner=auto|off|diagonal|schwarz`, plus `block` and the
OLS-only `reduction=auto|full|eliminate` with `pcg`; PPML sends
`preconditioner=` only for explicit `lsmr` other than `auto`); one ado table maps the
public grammar onto them and the plugin re-validates the tokens. The hidden
developer adapters take the tokens directly, including the PPML `map` family,
which is not a public request.

The experimental CMG-Schur route eliminates the larger FE implicitly in the
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
`solver(cg, reduce(eliminate))` and retain its OLS-only support envelope. Repeated observations
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
no graph iteration occurred (method `direct`). PPML explicit requests outside its supported
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
CMG remains explicit: a Fast final CG verification reports the route
`map, accelerate(cg)` (CG over the MAP operator) with fallback set and its
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

**Exact elimination first (accepted by Johannes, 2026-11-08).** OLS Auto with
two to five categorical intercept effects (Auto preconditioner, no declined
recheck, default routing; Fast and Low memory) and PPML categorical frames
(MAP family, two to five effects; four or five effects through the categorical
owner in Fast mode only) first eliminate the largest effect exactly and solve
the Schur complement of the remaining effects by lockstep PCG with Jacobi on
the true Schur diagonal (up to eight right-hand sides; single-cell segments
contribute nothing and are skipped). A column stops when its reduced residual,
which equals its remaining-level moments, is at most 0.1×tol times the column
norm after removing the eliminated-effect group means; weighted and PPML
callers also bound the Jacobi coefficient-error estimate by tol×RMS(column).
A column whose recurrence reaches its rounding floor returns to its best
iterate meeting the earlier rule (0.5×tol of the raw norm) or declines.
Acceptance recomputes the remaining-level moments from the formed residual
with the ordinary certificate fold; eliminated-level moments are zero by
construction, and the derived final-residual certificate recomputes every
level. Any stall, refusal, rank or nonfinite case declines with the input
untouched (or, after a late decline, with the same projection) to the full
system below. Both commands report the route as
`cg, reduce(eliminate) precondition(jacobi)`; a batch that escalated to
block-Jacobi reports `precondition(block)` with each retained effect's local
solve in `e(fe_local_solvers)` and counts in `e(fe_escalations)`. The same
elimination is explicitly requestable as `cg, reduce(eliminate)` (escalating)
or with a fixed `precondition(jacobi|block)` (never escalating). PPML keeps the categorical
MAP family identity internally, which its compiled validator admits; only the
reported route changes. `FEREG_BLOCK_SCHUR=0` (environment, read once per process)
restores the established routes for comparisons.

**Full system next (Milestone 2, 2026-11-08).** OLS Auto designs with at
least two categorical intercept effects outside the elimination's range (six
or more), and every design whose elimination declines, next run fedemean's
full system: lockstep PCG on the singular consistent normal equations
`D'WD x = D'W y` (up to eight right-hand sides, cold start at the largest
effect's level means), preconditioned by Jacobi. A batch whose late rate
after a 24-iteration pilot predicts more than 100 iterations for a column,
that stalls or that reaches the budget continues from its iterate on the true
residual with one-level additive Schwarz over the star of factor-pair
subdomains (the largest effect paired with each other effect, one subdomain
per connected component; each local operator's larger side eliminated
exactly and its Schur complement factored by a dense grounded Cholesky when
small or dense, an envelope Cholesky when its factorization work is at most
128 operations per Schur entry, otherwise approximate Cholesky AC(2) with a
fixed seed, of the whole component's Laplacian when that is sparser than the
Schur complement; a `tau 11'/n` term per component keeps the action SPD) when
it fits the memory limit; a slow batch whose Schwarz does not fit continues
with Jacobi. Later batches and projections of an escalated design start on
Schwarz. The recurrence stops at 0.1×tol of the starting
residual's norm and is accepted only by the recomputed moment certificate over
every level; it runs within half of `iterate()`. Results do not depend on the
worker count. A decline (memory, stall, budget, certificate) hands the columns
to the established routes below (for three or more FEs, MAP) with the rest of
the budget, and the certified result reports `e(fe_fallback) = 1` with the
decline in its warnings. The route
reads `cg, reduce(full) precondition(jacobi)`, or `precondition(schwarz)` with
`e(fe_escalations)` after an escalation, with the
subdomains counted by local solve in `e(fe_local_solvers)` and
`e(fe_subdomains)`. Declared scope: 6+ FE Auto fits and fits whose elimination
declines change at tolerance level (coefficients, covariance, fit statistics,
residuals, FE coordinates, iteration signatures); 2–5 FE fits that the
elimination certifies are unchanged bit for bit; sample, rank, omission and
other discrete decisions do not change. Witnesses: dense MAP oracles at 1–7
effects with and without weights, worker-count bit identity, exact admission
at fedemean's bound, and prepared 6/7-FE fits against tightly converged MAP.

The established routes (and the developer arms) are as follows. Public OLS
Auto/Auto uses the many-FE geometry policy (`routing_many_geometry_v1`, below)
for eligible designs with at least three categorical-intercept FEs whose
fedemean routes declined, or that `FEREG_BLOCK_SCHUR=0` or a hidden Auto
request with an explicit LSMR preconditioner keeps off fedemean. Eligible two-FE designs use
[structural CMG gate v2](two_fe_auto_gates.md) (`routing_two_gates_v2`),
with ownership and eager cells in the narrower global-intercept scope above. Zero/one FE,
slopes and explicit choices retain their specified behavior; PPML Auto adds only
the two-FE CMG reuse below. Activation and performance evidence for both OLS
families are complete, see [VALIDATION](VALIDATION.md).

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

The many-FE geometry policy is the general categorical route of OLS Auto
(three or more effects, and two-effect designs whose graph and compact routes
the memory budget refused). With exactly three unit-weight symmetric FEs, every
projection action may first run the bounded geometry-qualified MAP pilot
described in [ARCHITECTURE](ARCHITECTURE.md) within half of `iterate()`;
otherwise, or when the pilot does not certify, MAP restarts from the original
inputs with the remaining budget. Every exit is accepted only by the
independent original-design certificate at the requested tolerance. A MAP
workspace beyond the memory budget fails as an allocation error (r(909)) and
an exhausted budget as the MAP budget error (r(459)). The policy no longer has an LSMR
leg: its Jacobi-trial modified LSMR on the pinned within solver (additive
escalation, seeded correction) was retired on 2026-11-09 (Milestone 3) after a
decline census found that it never produced a certified result: the 37 bench
designs, the Stata fixtures and tight tolerances never reach it (a Break is an
interruption, never a decline); under memory caps the fedemean declines left its additive admission refused
(MAP ran unchanged); and with `iterate(20)` or `iterate(30)` on six-, seven-FE
and chain designs it ran uncertified before a MAP fallback that also failed.
Declared scope: fits that reach this route after a fedemean decline or with
fedemean off may change coefficients, covariance, fit statistics, residuals,
FE coordinates, iteration signatures, the reported solver and fallback
diagnostics within §20 tolerances, or move between a certified result and the
MAP budget error: a weak graph that only the retired LSMR certified within
the budget now fails clearly, and budget the LSMR no longer spends can let MAP
certify. The census fits are unchanged. Sample, rank, omission, original factor slots/base flags
and other discrete decisions remain unchanged. Witnesses: fedemean's full
system against the MAP route on mixing and near-confounded panels (row
agreement, FE-coefficient reconstruction, bit repeats), the full system's
escalation bit-identical at one to eight workers, the pilot's debit and
skipped-pilot identity, the analytic incremental-family witness, and the
prepared 6/7-FE starvation fallback against tightly converged MAP.

The categorical additive-Schwarz admission model of the formerly pinned
within solver is retired with it (Milestone 3, 2026-11-09). It charged
`T + R` level-sized worker banks per concurrent RHS apply, where `R` bounded
the factor-pair components whose local solve may yield into nested work (a
block of more than 10,000 levels), first by the level-count bound
`R = levels * (effects - 1) / 10,002` and, after that bound refused, by an
exact count of those yielding pair components (the borrowed many-FE route,
explicit and automatic owned LSMR on intercept-only designs and the PPML
preflight). fedemean's routes admit their own workspace, and §19 gives the
current LSMR admission of owned designs and PPML.

Hidden `routing_many_windows_v1` (bounded within LSMR session windows) and
`routing_incremental_many` (strict unseeded within LSMR) qualified many-FE
LSMR arms of the retired solver; on three or more categorical FEs they, and
the many-FE arms of `routing_portfolio_v1`, `routing_portfolio_unseeded` and
the isolated `routing_prefix_*` arms, fail with InvalidArgument (r(498))
before any work. Their two-FE meanings are unchanged.

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
and slope routes retain their existing behavior. The many-FE LSMR comparison
arms are retired with the pinned within solver (above).

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

Explicit OLS `solver(map, accelerate(irons-tuck))` (IT) retains its support envelope and incumbent
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

The isolated categorical prefix arms (`routing_prefix_{none,map4,it4}`)
qualified a bounded MAP or compact-IT prefix before a seeded within LSMR
session on three or more categorical FEs; with that solver they are retired
there and fail clearly. With two FEs they keep the incumbent recurrence
without an optional upgrade, a no-upgrade baseline for the routing witnesses.

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
The canonical census cache is admitted beside the census scratch and kept for
edge emission only while the exact initial-graph stage (scratch, raw edge bank,
Laplacian construction and the cache) fits; it is released with the raw edges,
so no later stage or retained charge includes it. These are
prospective engineering constants, not a policy selected by the earlier failed
Mac panel. There is no sampling ladder, resampling or second optional attempt.

The bounded build's terminal factor follows the hierarchy options (direct
threshold and stopping rules) and memory admission only; no construction-work
screen redirects a direct terminal to coarsening. Compatible Laplacian action
witnesses and exact disabled-path replay precede qualification.

With three or more categorical FEs this arm tried four MAP sweeps before a
seeded within LSMR session; that arm is retired and fails clearly. Its two-FE
portfolio is experimental and does not change public Auto or establish
performance evidence.

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

Explicit OLS `solver(lsmr)` and `solver(cg)` (other than CMG) on categorical
intercept designs run fedemean's full system or elimination (Milestone 2,
2026-11-08; declared scope: these explicit fits change at tolerance level,
discrete decisions do not). Modified LSMR runs one column at a time on
`W^1/2 D` with `M ~ D'WD` (Jacobi, identity or additive Schwarz, each SPD on all
of coefficient space); its stopping tests are proxies, so each pass is followed
by the moment certificate on the formed residual over every level and a pass
that misses restarts from its iterate at a tighter tolerance (at most four
passes within `iterate()`). Both methods stop at 0.1×tol of the starting
residual's norm and certify at `tolerance()` (the 1e-14 final residual
projection of saved outputs included); neither falls back.

Absorbed slopes (Milestone 3, 2026-11-09; covered by the M2 decision for
explicit LSMR). fedemean's full system takes each effect's channels (its
intercept, if any, and one per slope variable), with channel-major
coefficients `effect_offset + channel*levels + level`, fereg's layout.
`precondition(jacobi)` (and the automatic preconditioner's first stage)
inverts each level's `channels x channels` block of `D'WD` exactly; a block
whose channels are dependent on the level's rows (a slope constant or zero on
the level, fewer distinct rows than channels) is a null direction of `D` and
acts as its pseudo-inverse on the range. fedemean forms each level's blocks
in its centered basis (the intercept, then each slope less its weighted level
mean), an exact reparametrization; the operator, coefficients and certificate
stay in the raw basis. `precondition(schwarz)` (and the automatic escalation,
with two or more effects) is additive Schwarz over factor-pair subdomains: a
pair of two intercept-only effects keeps its Laplacian subdomains, and a pair
in which either effect has slopes is one subdomain over both effects'
channels, exact on the larger effect's level channel blocks and solving the
smaller side's Schur complement densely when it is small, otherwise by its
diagonal blocks. A design with one effect has no pair and keeps its channel
blocks. Results report `jacobi` and `schwarz`.

Explicit `solver(lsmr)` on a slope design stops on fedemean's per-level
certificate at `tolerance()`, which equals §16's on nonsingular blocks;
fereg's independent §16 certificate then decides, after at most one
correction of the residual at a tenth of `tolerance()` within the remaining
`iterate()`. A fit fedemean does not certify (a numerical decline, a memory
decline or a missed certificate) continues with MAP from the original input,
reported as the fallback (`e(fe_fallback)=1`) with the reason in the
warnings. There is no within leg any more: until the within retirement
(Milestone 3, 2026-11-09) such a fit first ran within's modified LSMR with
the same preconditioner (its recurrence at a hundredth of `tolerance()`
floored at 1e-15), then MAP, and explicit `precondition(schwarz)` with fewer
than two intercept effects ran within's additive Schwarz directly; it now
runs fedemean's pair subdomains over all channels. A design or choice
fedemean refuses as unsupported (before it reads any data; since fedemean
lifted its channel cap in 95f22670, only a test fault plan reaches this)
continues with MAP, reported as the fallback (formerly within, then MAP, with
no fallback flag); the public Rust entry `absorb_lsmr` refuses such a design
as Unsupported. Auto, `absorb_auto` and saved-effect recovery are unchanged:
for such a design they run MAP, as before fedemean served slopes, with no
fallback flag. fedemean's memory limit is the budget left beside the buffers
the route holds (input copy, candidate, coefficient bank; §19); a workspace
beyond it is a decline that continues with MAP, and a fedemean fit publishes
its own workspace.

Declared scope: explicit slope `solver(lsmr)` fits change at tolerance level,
including, since the within retirement, those with `precondition(schwarz)`
and fewer than two intercept effects and those fedemean does not certify,
which now continue with MAP; sample, rank, omission and other discrete
decisions do not change, apart from one declared rare-case outcome. Case 1
of the slope-boundary fixture
(`tests/stata/development/within_plugin_slope_boundary.do`: `g##c.(z1 z2)`
with slopes near ±2^24 that differ by units,
`solver(lsmr, precondition(none))`, `tolerance(1e-10)`) now fails with
r(459) and full rollback, like the fixture's cases 2–3: fedemean's
raw-basis certificate has a rounding floor of about 1e-10 of the column at
offsets of 2^24, so it declines, and MAP's raw level Gram drops the slopes'
pivots (independent certificate 0.164). Formerly within's per-level
centering certified it, reported as the fallback. At `tolerance(1e-9)` or
looser, without residual-derived outputs (`residuals()`, `predict, xbd`,
saved effects certify the final residual at 1e-14), fedemean certifies it
directly with no fallback (independent certificate 2.3e-10). At
`tolerance(1e-8)` its b, SE and rss are bitwise those of the former route
at `tolerance(1e-8)`; against the former route at `tolerance(1e-12)`,
|Δb| ≤ 8e-17, relative ΔSE ≤ 1.9e-16 and relative Δrss 3.4e-16, and
against the fixture's exact oracle |Δb| ≤ 1.7e-15. Witnesses: that fixture
(case 1 rejected at 1e-10 and certified at 1e-8, at one and four workers
and repeated), `fe::auto`'s explicit slope LSMR tests (the MAP continuation
bitwise as called directly, the independent centered projection, the
eligibility fault plan and the capped-memory declines) and
`fereg-testkit/tests/fe_certificate_oracle.rs` (explicit Schwarz with fewer
than two intercept effects against the explicit-design QR). Explicit PPML
LSMR keeps its scheduled certificate and one tighter continuation (§13).

OLS Auto on an owned slope design with at least two effects and without the
exact two-effect slope solver (three or more effects, or two effects whose
coefficient solver is not admitted; one effect keeps MAP's exact single
sweep) runs fedemean's full system first: PCG with the level channel
blocks, escalating to the pair-subdomain Schwarz above when a 24-iteration
pilot predicts a slow recurrence or it stalls, within half of `iterate()`, certified by the
generic owner's independent §16 certificate (one correction at a tenth of
`tolerance()` after a miss). A decline, or a correction that still misses,
continues with symmetric MAP from the current residuals under the original
reference norms, reported as the fallback (`e(fe_fallback)=1`). Certified
projections retain fedemean's coefficients for saved-effect recovery. The
exact two-effect slope solver stays the Auto route for the designs it serves
(VALIDATION records the comparison). Declared scope: these Auto fits change
at tolerance level; discrete decisions do not.

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

PPML categorical saved-effect recovery may use owned coefficient-space CG
when retained coefficients and direct two-effect graph reconstruction do not
certify. Before CG, a bounded MAP attempt may use at most eight sweeps and at
most one quarter of the original iteration budget. It must pass the same fresh
original-target, FE-moment and final-Hessian component-normalization checks.
An uncertified attempt releases all of its workspace; CG starts from the original
target with the remaining budget, and reported recovery iterations include the
attempt. Allocation, interruption and input errors propagate without retry.
The original accepted absorbed predictor remains the target. Recovery
uses the existing admitted coefficient banks, reconstructs search directions from
them, and permits one reliable residual replacement from the original target
within the same total iteration budget. Reconstruction and independently recomputed
FE moments govern convergence; the subtractive projection-energy stopping heuristic
is not used for this complete-reconstruction solve. Final-Hessian weighted
normalization is followed by fresh original-target global and per-row checks.
No saved-output request changes the fit's sample, rank, coefficients or covariance.
OLS, one-effect and slope recovery, successful direct paths and fits without saved
effects retain their existing behavior, except for slope designs as follows.

Slope-design recovery (Milestone 3): retained fedemean coefficients (Auto's
full system, explicit `solver(lsmr)`) are the first tier; a single slope
effect keeps its direct level solves; otherwise fedemean's full system
projects the target from the retained coefficients (a warm start) or zero at
`min(tol, 1e-12)` with coefficients requested; MAP stays the fallback after a
decline or a failed check. On every slope path a level whose channels the
scaled pivoted group solve finds rank deficient is re-expressed in that
solve's representative (the intercept kept on ties, dropped channels exactly
zero; nonsingular levels keep their coefficients), then the intercept
effects are normalized as above, the FE sum is reconstructed from the
coefficients and checked against the target (`max(100 tol, 1e-10)`; strict
PPML recovery also every row at its reconstruction tolerance), and the
independent certificate decides. This applies the §5 gauge to slope designs
with two or more intercept effects, which the general MAP recovery
previously left unnormalized (a named gauge correction: saved intercept
channels move by per-component constants; fitted sums, slope channels on
identified levels, sample, rank and covariance are unchanged). Independent weak three-effect, disconnected,
rescaling and outside-span witnesses cover the recovery boundary.

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

For no-FE Fast unweighted IID Auto/Auto fits with a global intercept and no
requested outputs or developer routing override, row-parallel stable centered
moments and orthogonal compression are permitted (Johannes, 2026-11-02).
Arithmetic and storage may change within existing scientific tolerances;
original-data residual/score and bread/covariance certificates remain mandatory.
Explicit centered values, rather than cancellation-prone subtraction of raw
cross-products, define the finite problem. Canonical sample, rank, omission and
factor-slot semantics are unchanged: an uncertain classification or an apparent
rank-deficient compressed factor must use the compatible canonical path.
Validated loader summaries may be consumed by core only with their exact
prepared row/column/sample binding. Public memory controls and one-call atomic
publication continue to apply.

A Fast fit that requests no residual, xbd, d or saved-FE output may keep its
within residual `r = y~ - X~b` instead of projecting it again, when the derived
certificate of §16 passes; its residual solver row reports the absorption's
method, zero iterations, zero pilot work and stop code 5. The derived residual
serves the two-FE categorical owner (decided by Johannes, 2026-10-27) and, for
beta.10, the direct one-FE Auto route and the many-FE Auto geometry policy,
whose routes all certify at the requested tolerance (decided by Johannes,
2026-10-28). It also serves no-FE Fast,
unweighted IID fits with a global intercept and Auto solver/preconditioner,
without developer routing overrides (decided by Johannes, 2026-11-02). Their
certificate independently recomputes the structural-constant moment. Requested
outputs, Low memory, explicit CMG, LSMR, MAP, CG, IT or SQUAREM sessions,
many-FE developer routing arms outside the geometry policy, other no-FE fits
and a failing derived certificate keep the certified final projection. Residuals,
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

The OLS frontend displays the stored sample size, model F and its F-tail
p-value, R², adjusted and within R², RMSE and per-dimension cluster counts
above the coefficient table. Its absorbed-DoF table uses `e(dof_table)` and
preserves the inexact and cluster-nesting markers. Undefined statistics remain
missing. Display and replay do not change posted estimates, sample or native
execution; prefix replay retains Stata's own inference display.

PPML deviance is `2 sum q_i[mu_i−y_i+y_i log(y_i/mu_i)]`, with zero-outcome
contribution `2q_i mu_i`. Log likelihood is
`sum q_i[y_i eta_i−mu_i−lgamma(y_i+1)]`, including fractional outcomes.
The reference null mean is `sum(qy)/sum(q)` irrespective of FE/offset;
ll_0 uses that mean and pseudo-R² is `1−ll/ll_0` when defined. Publish outer
and inner counts, halvings and convergence. Do not post OLS RSS/TSS/MSS,
R²/adjusted R², RMSE, F or e(df_r) for PPML.

The PPML frontend displays convergence iterations, effective N, residual DoF,
Wald chi-square and its p-value, pseudo-R², log pseudolikelihood and deviance,
plus cluster counts and the same absorbed-DoF table used by OLS. Wald inference
uses `e(df_chi2)` when available; an undefined test remains missing. Both
commands share a Stata display helper; PPML reporting remains after transaction
commit, and prefix replay retains Stata's inference display.

Both estimators preserve e(b), e(V), e(sample), coefficient/equation stripes,
model/command/weight/cluster metadata, effective and physical N, candidate/full
counts, ranks, absorbed metadata, singleton accounting, prediction binding
and requested output names. Sample/status/count/DoF fields are exact.
Required OLS families include `fereg_status`, `fereg_dof`, `fereg_fe`,
`fereg_cluster`, `fereg_subset`, `fereg_certificate`, `fereg_runtime` and the
solver results below. Solver results use the vocabulary of `solver()`:
reduction `full|eliminate|none`, method `map|cg|lsmr|direct|derived|none`,
preconditioner `none|jacobi|block|schwarz|cmg` and MAP acceleration
`none|cg|irons-tuck|squarem`. `direct` is the exact projection onto a single
intercept channel (or the constant alone) when MAP was not requested by name;
`derived` is a final residual certified from the absorption without a new
solve (zero iterations); `none` means nothing was absorbed. For the absorption
(outcome/regressors, aggregated across Low blocks) fereg posts `e(fe_route)`
(the used route in `solver()` syntax), `e(fe_reduction)`, `e(fe_method)`,
`e(fe_preconditioner)`, `e(fe_local_solvers)` (with `block`: `name=dense|cmg|jacobi`
per retained effect; with `schwarz`: the subdomains counted by local solve,
`trivial|dense|skyline|approximate=count`, nonzero kinds only, and the scalar
`e(fe_subdomains)`), `e(fe_acceleration)` and the scalars
`e(fe_iterations)` (also `e(ic)`), `e(fe_escalations)` (solves that switched
from Jacobi to block-Jacobi or additive Schwarz) and `e(fe_fallback)`; the
final residual projection posts the same items with the `fe_residual_` prefix.
`e(fe_solver)` is the normalized request; with `cg, reduce(eliminate) precondition(cmg)`,
`e(fe_cmgdegree)`, `e(fe_cmgsamples)` and `e(fe_cmgseed)` record the effective
CMG-Schur controls. Matrix `e(fe_solve)` (rows `absorption residual`; columns
`iterations escalations fallback warnings pilot_iterations pilot_contraction
setup_seconds solve_seconds certificate_absinf certificate_relinf
certificate_rel2 normal_equation`) holds the numeric diagnostics and no codes.
The route reports the actual certifying method: automatic two-FE Schur PCG is
`cg, reduce(eliminate) precondition(jacobi)`, and actual CMG application on that
path is `precondition(cmg)`; an allocated but unused hierarchy is not CMG. A MAP
continuation reports `map` with fallback set; already-certified zero-work
inputs report `map` with zero iterations. Low aggregation preserves CMG, block-Jacobi
and Schwarz use and sums escalations across blocks unless a certified fallback
takes precedence. The display names the absorption route (marking an
escalation), any local solves (`Local solves:`) and the passed accuracy check
with the effective tolerance, each on its own line.
The former 2x16 code matrix `e(fereg_solver)` and the `fereg_solver_*`,
`fereg_preconditioner` and `fereg_cmg_*` results are removed. The route row
is the native transport (OLS result wire 1.6, see the
[plugin protocol](plugin_protocol.md)); the ado decodes and validates it before
posting. This reporting changes only telemetry, not arithmetic, route
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
  drop_counts, separation_sequence, certificate, runtime and receipt
  matrices (all names after the first have the `feppml_` prefix);
- `dof_table` when applicable; `fe_solver` (the normalized request); the route
  of the final weighted least-squares solve, which always runs in the certified
  canonical session, as `fe_route`, `fe_reduction`, `fe_method`,
  `fe_preconditioner`, `fe_local_solvers`, `fe_acceleration`, `fe_iterations`
  and `fe_escalations` (the requested `lsmr`/`cg` serves the ordinary IRLS
  frames); `fe_fallback` (visible solver fallbacks over the fit) and
  `fe_fallback_reason`; matrix `fe_solve` with rows `canonical lsmr lsmr_joint
  cmg` and columns `ordinary_frames checkpoint_frames retries fallbacks
  iterations setup_seconds solve_seconds certification_seconds
  certificate_absinf certificate_relinf certificate_rel2 normal_equation
  projection_absinf backward_error` (the native 4x22 solver shadow and its
  identity codes are checked by the compiled validator and not posted). The
  compiled scalar poster still posts `feppml_linear_route` (a route code) and
  `feppml_solver_fallback_count` until the next Stata 15 Mata rebuild;
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

**Elimination pre-solve (accepted by Johannes, 2026-11-08).** A fresh MAP-family
publication fit in Fast mode with categorical intercept effects and continuous
regressors first takes plain IRLS steps whose weighted projections of
`[z, X]` use the exact elimination (each step reprojects the previous
residuals plus the working-response change under the new weights, which has
the same projection), solving the small normal equations of the regressors.
Only full Newton steps that do not increase the deviance are accepted; any
damping, FE-collinear regressor, rank, bound or nonfinite problem discards the
pre-solve and restores the seed. Only the predictor is handed over. The
pre-solve stops once a step changes the deviance by at most 0.01×tol. If its
last step meets the deviance criterion and the start passes the independent
score certificate, the pre-solve counts as the first of the two consecutive
passes; the certified iteration and the strict checkpoint then decide exactly
as above. Pre-solve steps count toward `iterate()` (the pre-solve takes at most
`iterate()-2` steps) and toward the reported outer iterations; their projection
iterations count as inner iterations. `FEREG_PPML_PRESOLVE=0` (environment)
restores the plain seed.

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
(§1) is such state.

**Explicit `solver(lsmr)` frames (fedemean migration, 2026-11-09).** Each
ordinary frame and strict witness solves the weighted least-squares Newton
correction `z=(y−mu)/mu` (zero outcome: −1) on the frozen kept controls `X`
and the categorical intercepts `D` at the current Hessian weights, in one of
two formulations with the same solution. *Joint:* modified LSMR on
`W^1/2 [X D]` with a block preconditioner (the controls' current-weight Gram
factor, or their diagonal when it is not positive definite; the FE block's
inverse level masses, or the identity for `precondition(none)`), warm-started
at the previous frame's solution scaled by the accepted step. *Projections*
(weighted Frisch–Waugh–Lovell): `[X, z]` projected onto `D` by fedemean's
full-system modified LSMR (`precondition(auto)`: Jacobi escalating to additive
Schwarz; `precondition(schwarz)`: Schwarz), the projected controls' normal
equations, and the residual `z̃−X̃b`; controls start from their previous
frame's FE coefficients, the correction from zero. `precondition(auto)` runs
the joint recurrence with Jacobi; a stall (staleness window 4, factor 0.75) or
a recurrence or certificate miss after one continuation at 0.01×tol (floor
1e-14) hands the frame to the projections, and later frames of the fit start
there. `precondition(jacobi)` and `precondition(none)` keep the joint
recurrence, whose double miss is the frame's numerical refusal;
`precondition(schwarz)` runs the projections. Every frame is accepted only by
the exact normal-equation certificate of its formed residual on the raw
controls and every FE level at the frame tolerance relative to
`max(||z||_w, ||r||_w)`; a projection miss reprojects once from its solved
coefficients at 0.01× its tolerance and lowers later frames' projection
tolerance tenfold (at most to 1e-4× the frame's). No preconditioner state
survives a weight change: the core-pair Schwarz factor and its drift-bounded
reuse (approved 2026-10-31) are retired with the pinned within solver. The
joint operator is kept because one Krylov recurrence on `[X D]` converges in
about as many steps as one projected column, so the projections cost about
`k+1` times as much where Jacobi preconditions the FE block well. Declared
scope: explicit PPML LSMR fits. Fits with two effects whose Jacobi recurrence
certifies run the former route's arithmetic and keep its bits; fits with three
or more effects (formerly core-pair Schwarz from the first frame), and fits
that reach the projections, change inner iteration counts, solver signatures
and low bits within §20; sample, rank, separation, IRLS iteration counts and
every other discrete decision are unchanged on the witnesses. Witnesses:
dense-oracle frame tests for every preconditioner (joint and projections),
the stall hand-off and projection-retry tests, repeatability at a fixed
worker count with agreement across counts (the projections bitwise), memory
refusal and cancellation, the two- and three-effect strict-final fixtures,
and the matched one-million-row comparison (two effects bitwise; three
effects |Δb| 2e-13, identical deviance).

**Checkpoint accelerator (2026-11-09).** Explicit LSMR and CMG fits with two or
three categorical effects whose canonical elimination declines project the
strict checkpoint's untouched columns with fedemean's full-system modified
LSMR (Jacobi escalating to Schwarz) at the checkpoint tolerance, within a
third of the remaining budget, in place of the pinned within LSMR batch. The
comparison and decisions are unchanged: only the original-weight
certificate accepts the candidate; one retry, of the failed columns when
their copy fits beside the retained candidate, runs from untouched inputs at
`max(0.01×tol, 1e-14)`; a numerical miss falls back to MAP on the untouched
inputs, reported as `checkpoint_lsmr_numerical_nonconvergence`; a workspace
beyond the admitted allowance is an allocation refusal. Declared scope:
checkpoints the elimination declined (none of the bench designs).

**Six or more effects (2026-11-09).** PPML's canonical generic owner of a
categorical intercept design outside the elimination's range (Fast mode)
projects first with fedemean's full system (Jacobi PCG escalating to additive
Schwarz when slow or stalled, within half the iteration budget and the solver
memory budget, with the coefficient stopping rule), reported under the
MAP-family engine with the route it used; a decline returns the input to the
generic MAP owner. The ordinary joint frames of Auto keep their coefficient
recurrence. Declared scope: Auto PPML fits with six or more categorical
intercept effects; their strict checkpoints and final solves change iteration
counts, the reported route and low bits within §20; discrete decisions
unchanged on the witnesses (six and seven effects against the MAP route,
repeatable at a fixed worker count and agreeing across counts; the
one-million-row six-effect bench, |Δb| 6e-17, identical deviance).
An ordinary joint frame of PPML Auto (§1's recurrence over the retained FE and
finite coordinates) may start from the last certified joint frame with the same
sample, topology and kept generated columns. That frame's fitted values `f`
(its response minus its residual) lie in the span of the effects and those
columns, so the next frame solves for the correction `z - f` of its working
response `z` and adds the seed's finite coefficients to its own: the warm start
at the previous solution, after an undamped step the Newton correction
`(y - mu) / mu`. The weighted norm of the full response `z` still scales the
recurrence and every stopping, reconstruction and acceptance test, and the
residual that the independent FE certificate and the original-input finite
scores check is the full response's, so each frame keeps its guarantee. A
correction whose weighted norm is not below the response's is refused and the
frame solves `z` itself, bitwise as without a seed; a failed or invalidated
frame and any other column identity, sample or topology leave no seed. The
fitted values are one observation vector in the joint workspace's admitted
memory (approved by Johannes, 2026-11-01). Declared scope: PPML Auto joint
frames after the first may change inner iteration counts, solver signatures and
low bits within §20; on the replay inputs IRLS iterations and every discrete
decision are unchanged
(`tests/replay/scopes/2026-11-01-ppml-seeded-joint-frames-macos.json`). The
accepted iterate may itself be closer to the optimum: on two disconnected
components whose weights differ by 1e13, cold frames lose the light component
below the heavy one's stopping scale and the strict check rejects twice, while
seeded frames pass the first strict check (eta error 1.7e-14 instead of 1.8e-11)
and the guarded final-products reuse then applies; a test-only fault plan keeps
the cold path's rejection coverage. Witnesses: seeded and cold frames in
lockstep with independent full-response finite scores and bitwise cold results
for every stale or refused seed at one and three workers, that fixture in both
forms, and the 77-case replay at one, two and four workers.
Sample mutation invalidates all reuse. Warm starts are guesses, never certificates. Final inference and correction
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
model span; LSMR works on the weighted operator itself. Its workspace
(fereg's estimate of fedemean's modified-LSMR workspace, §19) is bounded on
generation zero and charged to the memory plan only when the step is
re-executed; a step that certifies on its first attempt is never
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
polling occur on Stata's main thread, with one exception below; workers touch
Rust-owned memory. Native work from preparation through derived outputs and
saved-FE recovery runs under Break polling, and long serial row passes
checkpoint at a fixed row stride.
Catch Rust panics at FFI; no unwind crosses C. Safe crates forbid unsafe code.

Parallel numeric input. On a qualified host, action workers may call Stata's
read-only numeric accessor (`vdata`) inside a numeric window. The main thread
opens a window after validating the variables (numeric, in range) and the
observation count, lends one bounded phase of original-row blocks to the
action team and waits; while a window is open the bridge refuses every other
SPI call, including Break polling, display, metadata and stores. It closes the
window when every worker of the phase has returned (also on unwind) and polls
Break between phases. No Stata memory or dataset layout is used or retained,
and no window outlives the phase. Workers write disjoint rows of reserved
Rust-owned destinations; lengths are set only after every block succeeded.
Values, `value >= SV_missval` flags, excluded-row fills, candidate validation
and error precedence equal the serial readers, and destinations keep their
representation and row order, so loaded inputs and scientific results are
unchanged. A refusal before the first phase (policy, work plan, admission,
window) keeps the serial reader; any failure after the first phase starts is
returned, never retried serially. Qualification keys on public facts: plugin
target, Stata major version, edition, `c(console)` interface, and no alias
variable among the bound inputs; unknown combinations keep the serial reader.
StataCorp documents no thread-safety guarantee for `vdata`; the capability
rests on the recorded qualification (VALIDATION).

Windows x86-64 Stata/MP 19 GUI is eligible under the same input and resource
policy. Johannes waived Break-during-loading qualification on November 7,
2026: cancellation during Windows loading may be delayed or unavailable.
This exception changes no polling code, scientific criterion, failure rollback
or cancellation requirement outside Windows loading. Other Windows host
combinations retain serial input.

Estimation forwards the original `in` qualifier to the native call, where
Stata's main-thread `SF_in1`/`SF_in2` provide the candidate envelope. The `if`
expression is evaluated only by the existing frontend marker. Ordinary serial
and parallel loaders skip marker and payload access outside that envelope;
parallel workers initialize those original rows with the unchanged excluded
fills. Preparation diagnostics still validate the complete marker. Full-length
buffers, PPML extra-column fills and original observation identity are preserved.
Phase admission counts reads, value/missing writes, marker validation, possible
diagnostic replay and packed-RHS copies. Each phase admits at most 16 Mi original
row/slot pairs and at most 128 Mi accounted work units. The latter covers the
maximum eight work units per slot for a validated layout, including a packed
intercept; neither bound can be enlarged for worker occupancy. Ordinary input
shapes retain beta.13's phase sizing and one-block Rayon task splitting. This
permits longer intervals between Break polls than the worker-scaled candidate;
work bounds are not elapsed-time guarantees. Contiguous original-row phases
retain serial error precedence and disjoint writes. A diagnostic retry never cancels an original SDK error. Immutable window
positions are validated once and borrowed across phases. The parallel reader
admits the actual capacities of its target and bank descriptor tables before
dispatch; these are additional to the ordinary loader's existing array and
serial-transport allowances. Developer traces expose bounds and fallback causes.


The main-thread numeric row pipeline admits at most 262,144 doubles per tile
and at most 4,096 rows, reducing rows as input width grows. Eligible numeric
FE designs retain deferred column initialization, optional categorical
preparation and packed RHS reuse beyond 64 source columns, subject to complete
memory admission. Newly eligible wide serial preparation reads complete rows
in original column order before preparing categories. No-absorb loading,
parallel accessor group sizes and core moment/Gram width policies are unchanged.


COMPLETE is the last native operation; no allocation, polling, display,
computation or store follows it. A partial physical SPI write has no logical
meaning. The final ado commit protects posting and public output renames.
Any error, Break, store failure, bad receipt or missing helper restores prior
e(), named/null estimates, data, order and output names. No correction or
postestimation path makes an extra substantive estimation call.

Retain independently versioned exact estimation handshakes. Current
versions are action 1.7, model 2.1, OLS result 1.5, PPML request 1.8, PPML
result 1.5, ado identity 0.5.3 and SPI 3.0. Diagnostics accept same-major,
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
objects compiled by Stata 15 and verify source/object identity. A `version 15`
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
Declared scope (accepted by Johannes, 2026-11-08): the elimination workstream
changed the fixed reduction orders of passes shared by every route (column
statistics, score and reconstruction certificates, the IRLS objective, weight
and VCE weight sums, separation certificates and digests), which now use fixed
chunks merged over fixed binary trees. Every OLS and PPML route, including no-FE,
slopes, explicit `solver(map)`/`solver(lsmr)` and PPML low memory, may therefore
differ from 0.5.3 in the last bits (about 1e-15 relative); samples, ranks,
omissions, separation and every other discrete decision are unchanged, results
repeat exactly at a fixed worker count and agree scientifically across worker
counts.
Exclude timing, build identity and environmental resource observations only.
A tolerance pass cannot silently waive an unexplained exact-change failure.
Preserve Stata processor restoration on success, error and Break. `feppml`
runs its public call on one Stata processor below one million observations,
because Stata's MP workers once active compete with the native team, and keeps
the caller's count from one million observations, where the frontend's per-row
passes on one processor cost more than that competition (Johannes's 1M+
performance target, 2026-11-01).

Every additive-Schwarz preconditioner is fedemean's: it sums each level's
subdomain contributions in pair order, so its action does not depend on the
worker count in any mode. fereg no longer builds one in the formerly pinned
within solver (retired in Milestone 3, 2026-11-09). From beta.10 until then, those
builds (OLS borrowed and owned LSMR, the many-FE Jacobi trial's escalation,
explicit `solver(lsmr, precondition(schwarz))` and the PPML joint Schwarz
factor) used within's Ordered reduction, which commits subdomain
contributions in subdomain order, in every mode instead of its Auto
reduction, which may resolve to an atomic scatter whose summation order
follows thread timing; before beta.10 only `deterministic` selected it.

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
memory-audit framework is required. OLS Fast releases its projected matrix
after the finite solve, which no later stage reads (the many-FE LSMR route,
retired in Milestone 3, likewise released its RHS windows once its sessions
owned their responses). These arithmetic-preserving storage-lifetime changes
left admission totals unchanged (they still charge the released storage), so
`memory(auto)` chose Fast or Low exactly as before. Estimates remain estimates, not process RSS
or strict OS bounds. No new route-specific admission fences may be added to
hide a performance regression. Future removal of a mode needs mapped coverage.

**LSMR admission (Milestone 3, 2026-11-09).** The within-shaped
modified-LSMR estimate (recurrence windows, per-RHS result metadata, slope
reparametrisation storage) and the categorical Schwarz model with its exact
yielding-pair-component refinement (owned designs and PPML; §1) are removed
with the `within` dependency. Owned explicit `solver(lsmr)` and owned Auto's LSMR
attempt (`absorb_auto`) charge fereg's held buffers (the input copy, and the
coefficient bank when coefficients are retained) and fereg's own certificate
and projection scratch (the generic MAP-family estimate); fedemean admits its
own workspace (structure, weight state, recurrence and any Schwarz
preconditioner) within the budget left beside the held buffers, and a
fedemean memory decline continues with MAP, reported as the fallback. PPML
explicit LSMR projections, the strict-checkpoint accelerator and the
separation detector's LSMR re-execution (§13, §14) charge fereg's estimate of
fedemean's modified-LSMR workspace on categorical intercept designs
(structure, weight state, LSMR vectors) plus a Schwarz grant: for fedemean's
star pairs (the effect with the most levels paired with each other effect),
24 bytes per cross-tabulation entry (`min(N, L_a·L_e)` entries) and 20 per
level, plus 48 bytes per entry and smaller-side level for the components'
Schur complements and local factors, plus 16 bytes per level for apply
columns; identity and Jacobi re-executions charge no grant. fedemean admits
each Schwarz part from its exact size within that limit: Auto keeps Jacobi
beyond it, explicit Schwarz declines. These estimates no longer depend on the
worker count. Declared scope: admission outcomes change only under memory
limits between the former and the new estimates. Owned explicit LSMR that
the larger within-shaped estimate refused now runs fedemean when its own
workspace fits, otherwise MAP as the fallback; owned Auto may now try
fedemean where it went straight to MAP, and reports the fallback when MAP
then continues; PPML LSMR admits smaller grants. On the 161 Stata fixtures the pass set is unchanged (152 pass; the same 9 staging and installation failures as `9fe647ab`), and 13 matched one-million-row Mac cases (the defaults and every explicit route the change does not touch) are bitwise identical to `9fe647ab` on the same routes, so no admission outcome changed there.

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
