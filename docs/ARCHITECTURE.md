# Clean-core architecture

This is the target structure in [PLAN](cleanup/PLAN.md), governed by
[SPEC](SPEC.md). It is not an implementation or qualification claim.
[MILESTONES](../MILESTONES.md) records the active engine and completed gates.
The [preserved legacy architecture](cleanup/legacy-architecture.md) remains
a migration source until M7; the relocation avoids case-only path collisions.

## Boundary and data flow

```text
Stata syntax, if/in, factor/TS metadata, held estimates and private outputs
  -> one guarded estimate-ols / estimate-ppml plugin call
  -> plugin: validate request; resolve resources; load via main-thread SPI
  -> engine::spec::ModelSpec + engine::data::Loaded
  -> data::prepare: row ledger, missing/domain rules, weights
  -> topology::build: codes, interactions, singleton fixed point, components
  -> ols::fit / ppml::fit
       shared fe::Engine -> wls -> vce -> outputs -> certified result
  -> plugin: validate all result values/bindings; store private shadows
  -> COMPLETE last; ado validates, posts e(), commits requested names
```

Stata owns syntax and the public transaction. Rust owns observation-scale
preparation and computation. C is an ABI adapter. Bounded metadata/result
validation in Mata is permitted; dummy expansion, FE coding, dense designs
and estimation loops in ado/Mata are not. SPI is called only on Stata's thread.
Workers receive Rust-owned buffers. Safe crates use `#![forbid(unsafe_code)]`;
project-owned unsafe operations live only in the plugin FFI boundary.

## Crates and budgets

Production Rust must stay at most 35,000 lines; engine at most 25,000 and plugin
at most 8,000. These are ceilings, not targets to fill. Files are at most 1,500
lines, functions at most 150. New engine/oracle limits apply immediately;
all crates are covered after M7. Split by cohesive responsibility, not suffixes
for another memory mode, VCE pipeline or special model envelope.

| Engine module | Responsibility | Production-line budget |
|---|---|---:|
| error | Typed failures and Stata return mapping | 150 |
| threads | Call-owned team, fixed partitions and reductions | 450 |
| spec | Model/request types and closed wire parser | 2,000 |
| data | Loaded sources, sample ledger, weights, prepared rows | 1,700 |
| factor | Canonical numeric/string/tuple coding | 600 |
| topology | Membership, singleton fixed point, components, slope channels | 1,800 |
| dof | Structural rank, redundancy and nesting policy | 1,000 |
| fe | Shared design/plan, four backends, certification | 7,000 |
| wls | Moments, rank-aware Cholesky/QR and information solves | 2,000 |
| vce | IID, HC1, cluster subsets, PSD and tests of coefficients | 1,800 |
| ols | One estimator orchestration | 600 |
| ppml | Family, IRLS, separation coordinator/detectors, bias | 4,000 |
| outputs | Reconstruction, normalization and requested row products | 800 |
| result | Certified estimator results and diagnostics | 600 |
| Total | Leaves 500 lines of engine headroom | 24,500 |

The plugin's 8,000-line ceiling covers FFI/panic containment, safe input loading,
wire/result validation, engine adapters, publication and installed diagnostics.
It contains no second numerical implementation. `fereg-oracle` is dev-only:
independent dense OLS/Poisson, seeded designs, comparison helpers and differential
tests. It does not contribute to native artifacts. Its production-like helpers
also obey safe-code, file and function limits.

Harvest code by copying the named source in PLAN §8 with a provenance line.
Keep numerical and behavioral tests; remove legacy instrumentation/routing only
when the target invariant is covered. The new engine never depends on legacy.
Only the temporary plugin dependency and oracle's dev-only differential
dependency are allowed. No new production dependency on testkit or oracle.

## Data ownership

`ModelSpec` contains syntax-derived source roles, canonical coefficient slots,
factor/TS recipes, absorbed terms, clusters, options and requested outputs.
It contains no SPI pointer and no generated N×K matrix. Parsing rejects unknown
keys, malformed length framing and incompatible wire identities before input.

`Loaded` owns each numeric/string source once, explicit missingness where needed,
the candidate marker, raw weight/adjustment sources and original-row identity.
Unweighted is a variant, not an allocated vector. Sources with multiple roles
share identity. Stata materializes only the allowed TS operators before binding;
the Rust side does not change host lag/lead semantics.

`Prepared` is the sole final-sample representation: stable row map/drop ledger,
borrowed or owned compact sources, generated-column recipes, normalized weight
view, and one `FeTopology`. Different solver, VCE and output requests borrow
this same representation. Ownership transfer may reuse an allocation without
creating an alternative "fast" representation or retaining two equivalent banks.

`FeTopology` owns compact codes and CSR membership per distinct partition,
declared effect/channel order, references to aligned slope sources, components
and original-row links. Codes and CSR must agree independently. Interactions
use exact tuples; hashes never define equality. Singleton pruning reaches a
fixed point using the correct multiplicity before topology is finalized.
DoF and cluster nesting consume this topology without recoding observations.

PPML's ledger has a monotonically changing sample generation. A certified
separation union/cascade commits one generation and compacts all row-scoped
state together. Numerical sessions cannot survive that generation change.
Public masks and outputs are always scattered through the ledger, never through
a detector's private row order.

## Common fixed-effect interface

`fe::Design` borrows topology and current positive weights and exposes D,
D'W, per-level weighted blocks and coefficient layout. `fe::Plan` records
the public request, mathematical eligibility, selected backend, total budget
and allowed fallback. It cannot inspect N, finite width, workers, weight kind
or memory to choose a route. Structural dimensions can determine mathematical
support and allocation sizes; they cannot implement performance fences.

`fe::Engine` applies the plan to a batch of RHSs and optional coefficient warm
starts. All backends return coefficients in canonical channel/level order,
explicitly reconstructed residuals, and the same diagnostics/certificate type.
No solver returns a privileged "already certified" estimator state. Private
`PlanOverride` is an explicit test input, not a cfg(test) branch in production
control flow or a replacement public solver option.

For a RHS v solve

```text
min_alpha sum_i w_i (v_i - sum_q x_qi' alpha[q,g_q(i)])^2.
```

An effect has one intercept and/or several slope channels per level. Local
rank-deficient blocks use the shared rank-aware span policy. Backend success
must preserve this span even where coordinates are nonunique.

### Direct projection

No FE is no-work; one FE solves each small per-level weighted block and
subtracts its contribution. Multiple RHSs share membership/weight passes and
local factors. Singular channels use their identified local subspace rather
than arbitrary diagonal regularization. Return coefficients and run the same
reconstruction and orthogonality checks as iterative backends.

### Schur-PCG

Eliminate the effect with the most parameters, using canonical effect order
to break ties. Partition the coefficient normal system as

```text
[ A  B ] [a] = [g]
[ B' C ] [b]   [h]
S b = h - B' A^+ g,       S = C - B' A^+ B
a = A^+ (g - B b).
```

A consists of independent per-level blocks; A^+ denotes the rank-aware
identified solve, not a dense generic inverse. Apply S through observation
passes over all active RHSs. Do not form the global dense FE matrix or Schur
matrix. Block-Jacobi uses per-level blocks with the same rank policy.
Operate on the compatible identified subspace and preserve component/nullspace
constraints. Breakdowns do not justify adding a ridge or changing model rank.

Each RHS has its own recurrence and convergence evidence; shared passes must
not make one RHS's stop decide another's. Warm starts are coefficients in the
same declared layout and generation. A monitor uses observed progress and the
remaining SPEC budget. Stagnating columns can transfer once to the same LSMR
adapter used for explicit requests; transfer cannot reset iteration accounting.
The initial monitor checks the best preconditioned residual norm every eight
iterations, requiring two complete windows. Less than one percent improvement
over the latest window signals stagnation. Otherwise, the fastest observed
log-linear rate predicts the work needed to meet the recurrence target;
transfer is permitted when that exceeds the remaining budget. Converged columns
are excluded. These are progress rules, not model-size routes. Their performance
admission belongs to G-M3; qualification can disable the monitor or force a
checkpoint through explicit internal controls.

### Retained LSMR and CMG

The LSMR adapter uses pinned `within::Effect`/`Solver::new_raw` batch interfaces
and maps coefficients back to `FeTopology`. Raw construction preserves the
original slope span; upstream whitening/rank truncation cannot make fereg's
rank decisions. Retain coefficients and reconstruct residuals independently.
Use static additive Schwarz with explicit `ParallelReduction` in the call-owned
team; no nested pool or upstream worker-dependent Auto policy.

The preconditioned recurrences' stopping metrics differ from the shared FE
span target. Schur-PCG, LSMR and the complete CMG nuisance frame use one
original-input residual-refinement protocol. It requires the unchanged FE
score target and stable fitted corrections at the within-residual scale,
bounded below by machine precision at the original RHS scale. A mostly
orthogonal residual cannot stop a correction at zero work merely because its
local normal score is small: the internal target also reduces that initial
score, with a double-precision floor. Each correction consumes the remaining
per-RHS budget and increments the same backend attempt's refinement count;
stagnation/breakdown/exhaustion fails closed. Schur corrections start without
the original-response warm guess and reuse exact factors. Only the shared
final certificate accepts a projection; fallback uses the same refined LSMR.

CMG maps the supported two-intercept core to a weighted bipartite Laplacian,
anchors components, and eliminates supported nuisance intercept columns using
the documented small Schur block. Recover potentials into declared FE order,
then use common residual reconstruction and normalization. Core RHSs use the
call-owned worker team with fixed scalar CMG arithmetic per RHS and reusable
workspaces; dependency-owned executors and size-based execution policies are
not used. The graph solve uses an absolute residual target derived from the
original weighted RHS norm and minimum core level mass. Initial and nuisance
setup solves retain zero relative backward-error tolerance. True-residual
corrections permit an epsilon backward-error allowance inside PCG to avoid
exhaustion at its representational floor; this never changes the original-input
FE certificate or establishes forward accuracy. Full-model exact-ray and
independent weak-graph oracles remain mandatory alongside fitted-update checks.
No slopes or unsupported multiway graph can be smuggled through a graph adapter.
The small nuisance Schur block is represented by the core-projected indicator
columns, factored with the shared weighted pivoted QR instead of squaring their
condition number. Rank is measured against the original weighted indicator
norms, including a unit reference when every projected column is tiny. The
guard is `8*epsilon*max(rows,columns,1)`; setup core solves target the smaller
of the requested tolerance and one sixteenth of that guard. This does not
change final acceptance scales. Recover nuisance coefficients and subtract
their stored core projections from the core solution. Each setup indicator
has a visible attempt and the same per-RHS budget; failed setup retains these
counts and cannot be retried implicitly. Numerical state belongs to one frame.
Preserve SPEC's OLS/PPML support distinction, including nuisance limits and trivial
OLS direct cases. Changing PPML weights invalidates numerical Laplacian,
hierarchy/preconditioner, nuisance factor and certificate state; immutable
membership can be retained. Any permitted reuse must bind exact current weights.

Explicit requests start with that backend. SPEC bounds one numerical transfer
to LSMR and makes topology/input/allocation/Break/internal errors fatal.
The engine checks each completed candidate against original inputs after
normalization. A failed certificate or numerical stop can transfer that RHS
once to the same refined LSMR adapter, solving its original RHS with the exact
remaining budget. Certified columns stay on their original backend. OLS CMG's
zero/one-effect cases report actual no-work/direct execution and retain the
CMG request. Numerical CMG setup failure is visible separately from each model
RHS; setup RHS budgets are not repeatedly charged to all model columns.

Projection diagnostics retain requested/selected/used/checkpoint backends,
each attempt, total per-RHS work and the fallback reason. Auxiliary setup work
is reported once per immutable frame and forwarded through shared WLS and
separation span checks. Reports remain inspectable on engine errors, including
the full primary bank if resolving an earlier RHS aborts publication. Reusing
a factor cannot repeat setup work or implicitly retry failed setup. Final
projection construction still requires the common original-input certificate.
Diagnostics identify every attempt and the backend that actually produced each
accepted frame. CMG automatic selection needs separate comparative admission.
Both dependencies must pass cross-worker determinism and independent weak-graph
tests; their own convergence flags and existing reproducibility claims do not
establish the new contract.

### Shared certification

Compute r=v−Dalpha afresh. Independently check row reconstruction and weighted
local-span orthogonality at SPEC scales, including slope rank and input norms.
Normalize only by transformations that preserve the fitted FE sum. Store
scientific diagnostics separately from timings. Certificates are necessary
runtime checks; dense/100-digit oracle tests establish independent accuracy
on ill-conditioned graphs. The legacy CMG raw-residual failure stays a direct
explicit-CMG regression, not merely a test of a replacement auto route.

## Finite WLS, inference and outputs

`wls` forms deterministic moments, tries guarded pivoted Cholesky and falls
back to scaled pivoted Householder QR. QR controls canonical rank/status.
One result carries coefficients, bread and slots. PPML bias reuses its information
operator, never substitutes e(V) for full-system information or builds another estimator.

Shared FWL assembly reconstructs the original residual and checks both its FE
certificate and finite score. Cancellation may require a derived FE-target
solve. When the original response is much smaller than y-Xb, that target's
certificate alone does not control the original finite score. At most three
additional FE projections of the freshly reconstructed residual may correct
the combined coefficients, reusing only the exact immutable weight frame.
Each correction is normalized and independently recertified; nonimprovement,
exhaustion or any remaining score failure rejects the result. These solves
retain their full recovery reports and counts. Rank, finite coefficients and
all mathematical acceptance limits remain unchanged.

`wls::reporting` supplies one canonical slot map for OLS and PPML. PPML inference
borrows a sealed fit: the exact sample, raw columns, means and final-Hessian bread
stay immutable, with streamed recipe validation. Its ordinary constant is
identified; an absorbed one is zero outside the sandwich, with one DoF transfer
unless nested. `vce` shares score/subset/PSD mechanics under each estimator's SPEC
corrections; independent checks and statistics use original/accepted inputs.

`outputs` constructs only requested residual/xb/d/eta/mu/saved-channel banks,
checks reconstruction and scatters once. PPML certifies sealed qmu-normalized
FE coefficients without a re-solve; its statistics use the accepted objective
and offset-free reference null. Bias retains its distinct anchor; requests do
not change inference or replace strict final-Hessian/tighter accuracy checks.

## PPML orchestration

`separation::run` owns one Prepared ledger, immutable proposals and fresh
certificate/replay acceptance in canonical FE/simplex/IR/Mu order. Atomic
singleton/FE cascades commit one generation; every deletion discards the old
numerical scope and regenerates finite recipes from compacted sources.
Bounded method totals, a 64-event prefix and deterministic digests retain all
generations' work. The sealed FinalSample owns no exploration; cold IRLS
recomputes rank and final-Hessian authority on its sample.

IR uses an immutable artificial weight frame (unit interior weights and
`tol^2/N0` boundary weights), independent of prior weights and adjustments.
Before recurrence it freezes the full auxiliary finite span in a fresh strict
unit-metric WLS frame. Estimator FE-omission thresholds cannot remove a cone
direction: the auxiliary guard trims only squared machine-roundoff residuals,
and shared QR owns numerical span rank. The recurrence uses those unit-within
columns, preserving span without large finite/FE cancellation. A validated
deferred constant has exact zero within values; subtraction roundoff cannot
become a new RHS. Weighted recurrence uses the registered IR projection target;
fresh cone certification uses original columns and strict unit projection.
Every frame and correction contributes observable work.

Generation-bound IR proposals need fresh cone/span acceptance or exact no-drop
recurrence replay. Cached state has no terminal authority. Exhaustion,
stale/tampered evidence and unsupported solvers fail clearly.

Simplex counts the entire explicit finite/FE/channel width before dense work,
including checked QR, null-space, tableau, topology and certificate storage.
Column scaling preserves the cone; auxiliary QR removes gauges and rechecks
every original column's span. Positive elimination also reduces boundary rows;
homogeneous coordinates keep small contrasts visible to Bland phase-I. Rays
need independent original cone/span certification; infeasibility needs a
separately checked Farkas multiplier against the immutable boundary cone.
No-drop reports require fresh deterministic replay. A resource skip has no
terminal authority alone: the requested comprehensive IR must freshly replay
the same current model and generation with no unhandled separation. An IR drop
restarts the chain, invalidating the old skip and its capacity-resolution seal.

Mu uses the same bounded IRLS trajectory in exploration mode; accepted
linear/state/mean/objective evidence is required, but a finite root is not.
Small means nominate candidates only: a fresh unit-metric full-model
projection proposes a cone direction, then independent ledger span/sign
certification decides deletion. No-drop reports need full current-generation
replay. Discard means, parameters, basis and factors before final-sample IRLS.

IRLS evaluates the Poisson family, refreshes h=qmu, solves shared weighted
linear frames, and step-halves the joint predictor/coefficient proposal.
Use global plus analytic one-FE group starts only. Two accepted convergence
passes, strict WLS, frozen-basis revalidation and independent final-Hessian,
score/objective/model-space checks are mandatory. A weight generation can
reuse structural topology; it cannot reuse a stale numerical certificate.

Shared WLS records FE work before finite fitting or speculative rejection can
discard a frame. Fixed-size summaries separate primary projection, derived-RHS
recovery and auxiliary setup; retain backend attempts, refinements, stop and
fallback counts; and distinguish summed iterations from the maximum per RHS.
IRLS accumulates every completed or resumed numerical frame, including an
unrepresentable Poisson checkpoint and a rejected basis rebase. Fisher checks,
final-Hessian checks and rebases have separate counts. Setup is charged once
per immutable frame, and overflow is a fatal error rather than saturation.
The final accepted frame still carries its detailed per-RHS reports.

Optional bias correction uses the full identified FE-plus-control information
operator at the accepted state. A scoped frame borrows sealed final qmu and
the accepted finite basis; shared WLS records all solve/recovery work. Raw eta
recovery and every auxiliary action share a support-only component anchor,
fresh reconstruction and original-coordinate scores. Validate all declared numeric terms, including omissions; stream exact units or fixed shared Philox signs for both increments.
Probe-count weighted groups report D/C variance and covariance. Shared output allocation maps nine ordinary channels and requested FE banks; the atomic calculation leaves ordinary results unchanged and option-off creates no work.

## Memory, threads and errors

One call-level memory plan covers input, topology, numerical work and output
lifetimes. Use request dimensions and conservative source bounds before loading;
the plan owns accounting through the complete call. There is no independent
per-backend admission controller and no alternate low-memory estimator.
Clearly insufficient capacity and checked-size overflow fail early; otherwise
allocations remain fallible. Allocating a buffer is never proof of scientific
eligibility. SPEC's visible simplex-cap rule remains a detector contract.

Use one call-owned Rayon team. Fixed logical partitions and deterministic
merge order do not depend on worker count; size constants can tile a kernel
but cannot choose a numerical route. Dependency adapters must bind the same
arithmetic ordering. Nested pools and Stata SPI on workers are prohibited.
All foreign work must be joined before borrowed/native owners are released.

`error` retains semantic classes and Stata mappings from SPEC. Carry contextual
solver/frame/certificate information without exposing partially valid results.
Panic is caught once at FFI. Resource guards release Rust/foreign workers and
state on errors; the ado guard restores public state and any temporary Stata
processor setting on every exit. COMPLETE remains the final native operation.

## Migration seam and validation

Before M2, core/testkit stay in their original paths; M2 moves them under
`legacy/`. The plugin dispatches its existing legacy dependency or the new
engine through one hidden selector. Explicit engine() overrides the developer
global, then the current default. `engine_next.rs` converts certified results
to existing shadows; extend old validators minimally for new solver identities.
No alternate publication or sample path is introduced by that conversion.

OLS switches only after G-OLS; PPML only after G-PPML including mean correction.
Keep mapped source docs, scientific tests and independent oracles until their
replacement checks pass. At M7 remove legacy and both temporary dependencies,
the selector, tracking allocator and obsolete result rows. Result wire v2 and
matching ado validation become one path. Regenerate/remove compiled Mata
helpers together with their source/object checks. Retain dependency notices;
native distribution separately requires G-LICENSE.

CI runs the lean harness on every push. Independent oracle tests force every
supported backend and errors, include at least 2,000 seeded designs and sealed
weak graphs, and compare samples/status/DoF exactly against legacy during
migration. Stata fixtures verify the public transaction and installed outputs.
Mac/SCC benchmark tiers measure default policy and explicit solvers separately.
M8 can compress FE edges, batch RHS kernels and improve SPI transport while
keeping this math/interface; changed data layout or parallelism requires both
scientific checks and measured suite evidence. No performance cliff earns a
new model-size route. Gates and any failed evidence remain in MILESTONES.
