# Second simplification program: M26–M35

Approved direction: Johannes, 2026-10-04. This document defines the next
architectural program; [MILESTONES.md](MILESTONES.md) remains the only mutable
status ledger. The [first plan](docs/cleanup/PLAN.md) and its evidence remain
historical prerequisites, not a second implementation to maintain.

## Objective and authority

Deliver a substantially simpler, maintainable fereg with broadly competitive
performance and predictable behavior across supported model and option
combinations. Replace complete competing implementations and scattered tuning
policies. Each architectural milestone must retire named production code or
repeated work; moving it into helpers or adding a common facade is insufficient.

Johannes approved three changes to the earlier implementation constraints:

1. Deliberate changes to automatic algorithms, arithmetic order and internal
   storage are allowed in declared scopes, subject to independent scientific
   acceptance and fixed-configuration repeatability.
2. `threads(n)` is an available worker budget. A stage may use fewer workers
   when its work or resources cannot usefully support the whole team.
3. Bounded, declared performance tradeoffs may purchase substantial
   simplification and broad gains. Serious regressions and unexplained cliffs
   remain failures, even when an aggregate benchmark improves.

[SPEC](docs/SPEC.md) continues to govern models, accuracy, supported features
and transactions. [ADR 0249](docs/adr/0249-general-execution-and-acceptance.md)
records these architectural and evidence decisions. The new authority applies
to scoped tasks under this plan, not retrospectively to failed earlier trials.
Writing this roadmap does not execute its runtime changes or pass their gates.

Preserve all supported OLS/PPML features, public memory choices, explicit solver
requests, both bias-correction modes, unsafe-CG rejection and the M22 scientific
repairs. No silent model change, tolerance weakening, hidden backend substitution
or native-to-legacy fallback. Stata retains syntax, metadata and the transaction;
Rust owns numerical work. Main-thread SPI, FFI panic containment, one substantive
native call, allocation/Break handling and atomic publication remain required.
The frozen comparator and licensing gate remain in force.

M25 is reserved for the first plan's normalization-reuse proposal. Its execution
is not assumed or required here. M26 selects the latest qualified runtime and
incorporates any completed M25 evidence; no numbering collision or automatic
revival of the rejected M23 adapter is intended. Implement within the working
engine in coherent retirement batches, never by reviving the abandoned clean-core tree.

## Starting evidence and what must change

The inspected source at `c126f5d9` retains the qualified M22/M23 production
runtime. Its inventory counts 106,083 core and 15,313 host production lines,
112 functions over 150 lines and 22 files over 1,500 production lines. These
are a design snapshot; M26 pins the actual qualification baseline and binaries.

M12–M21 retired useful ownership duplication but retained most execution routes.
The surviving complexity includes ordinary/categorical owning preparations,
dense/scratch fitting sequences, several FE representations and workspaces,
PPML canonical/specialized state, and separately maintained memory estimates.
Earlier 8-to-16-thread slowdowns occurred with unchanged route and iterations.
Scattered thread and option guards therefore require both architectural cleanup
and attribution of scheduling, synchronization and memory traffic.

Use the existing [M21 review](bench/history/2026-10-03-m21-strict-recovery.json),
[M22 PPML repair](bench/history/2026-10-03-m22-ppml-correction.json),
[saved-FE repair](bench/history/2026-10-04-m22-saved-fe-correction.json),
[M23 rejection](bench/history/2026-10-04-m23-frozen-scan.json) and
[M24 attribution](bench/history/2026-10-04-m24-ppml-cost-attribution.json).
M23 showed that a saved scan can lose to a compiled per-row call. M24 identifies
real repeated normalization and additional finite/strict work; it does not prove
that all correctness cost is removable. Benchmark owners must be verified:
several earlier controls bypassed the path being optimized.

## Target architecture

| Responsibility | End state | Implementations to retire |
|---|---|---|
| Model preparation | One finalized sample/source/model/topology owner; optional cluster metadata | Ordinary/categorical and clustered owning pipeline duplication |
| FE operations | Coarse setup, weight refresh, batched projection, recovery and certification | Competing representations, lifecycle code and feature-specific route trees |
| Finite weighted-linear work | Shared scaling, rank, factorization, refinement and product operations | Duplicated end-to-end OLS and retained/frozen WLS sequences |
| Storage and memory | Dense or bounded block storage for the same operations; owner-based lifetimes | Fast/Low estimator forks and a separate shadow model of allocation |
| Parallel execution | One worker budget with a few general work decompositions | Feature-specific worker ceilings, nested ownership and scattered crossover policies |
| Inference and outputs | Explicit consumers of accepted fit products | Output-driven refits and repeated score/residual/recovery assembly |
| PPML | One accepted/candidate protocol with explicit generation invalidation | Competing caches and repeated transitions with equivalent scientific jobs |

Dispatch once at a coarse operation or block boundary. Keep inner numerical
loops concrete, contiguous and inspectable in compiled code. A universal
per-observation interface, generic task engine, new arena framework or mandatory
materialization of a low-memory matrix is not the target. Shared operations
must not force OLS to allocate PPML checkpoint state or force PPML directions
through an OLS publication pipeline.

Algorithm families follow mathematical structure: no-FE finite work; direct
one-FE operations; a two-FE graph family where justified; a broadly capable
general FE family. Slope structure, rank, connected components and conditioning
still determine mathematical applicability and certification. FE count alone
does not justify applying an intercept-only graph algorithm to slopes.
Explicit supported solvers use the common lifecycle without being silently
replaced. Automatic policy must be small, documented and supported across
families, not reconstructed from thread/output/VCE combinations.

Weights, clustering, saved effects and memory controls have real semantics and
costs. Compose those requirements with the shared pipeline. Optimized kernels
for particular operations are permissible; independent estimators for a growing
cross-product of options are not. Extra cluster aggregation or certified FE
recovery is legitimate; an unrelated refit or loss of a generally useful kernel
merely because another output was requested needs removal or a specific proof.

## Acceptance shared by every milestone

### Scientific scope and compatibility

Before editing runtime code, declare whether the task is arithmetic-preserving
or intentionally changes an algorithm, reduction order, storage/admission policy
or route. Name affected cases, fields, decisions and errors, and map them to
independent witnesses. A change discovered after a failed exact comparison
cannot be reclassified merely to make the check pass.

Arithmetic-preserving tasks retain exact scientific outputs and available
route signatures against the qualified parent. Intentional changes require
independent dense/analytic/high-precision evidence and the full applicable
SPEC tolerances, with no loosening of stronger named regressions. Unaffected
cases and fields remain exact. Preserve samples, omission/rank/separation
decisions, DoF, weight semantics and estimator meaning. A proposed change to
those scientific decisions requires a reproducer and Johannes's decision.
Internal routes, iterations and work counts may differ only as declared.

Keep fixed-configuration deterministic repeats and fixed seeds/streams.
Different worker counts and platforms require scientific agreement and stable
discrete decisions, not identical floating-point bits. Keep independent raw
model, final-Hessian and reconstruction certificates even when a shared cache
could reproduce a previous calculation. Reuse requires a proven unchanged
sample/topology/weight/basis dependency, not merely equal dimensions.

Retain public syntax, supported explicit solver semantics, output names and
metadata, next/legacy selection, and the frozen native protocol. Preserve
failure classification, cleanup and publication atomicity. Any intentional
admission or error-precedence change must be separately declared, reviewed and
covered; it is not a by-product that may be hidden in a refactor.

### Performance envelope and interaction coverage

M26 freezes a versioned manifest using the existing benchmark runner and
scientific fixtures. Expand them for demonstrated architectural gaps; create no
new polling, receipt or general benchmarking framework. The manifest includes:

- OLS and PPML; no/one/two/three-plus FE; intercept and supported slope designs;
  balanced and skewed level degrees; well-connected, weak and disconnected graphs.
- Small, medium and SCC-scale row counts, narrow/wide finite designs, repeated
  and sparse group combinations, weight kinds and large weight variation.
- A complete thread × requested-output × VCE cross-product within declared
  representative model strata. Outputs include none, residuals/predictions,
  saved FE and their combination; VCE includes IID where distinct, robust and
  one-/multiway clustering. Unsupported cells are explicit exclusions.
- Both storage modes and auto across targeted strata; every supported explicit
  backend; default/strict tolerances; PPML separation/rebase and both bias modes.
  Use covering combinations outside the core cross-product, plus full crossings
  where a shared owner or branch introduces a higher-order interaction.
- Mac native threads 1/2/4/8/16, with diagnostic 7/8/9 and 15/16 boundaries;
  SCC whole-node 1/2/4/8/14/28. Include neighbors of inherited row/width fences.
- Frozen development seeds and held-out seeds/topologies/shapes. Hold out
  combinations as well as individual sizes. Tuning a revealed holdout makes it
  development evidence; replace it before claiming generalization.

Use current qualified fereg as the acceptance reference and pinned external
packages where model and accuracy are comparable as contextual competitiveness
witnesses. Keep the fixed program baseline as well as each immediate parent;
successive small regressions must not accumulate unnoticed.

Use clean identified binaries and frontends, one warmup and at least five
fresh-process measurements, with paired orders and bounded confirmation of
flagged results. Use median command time as the primary timing statistic;
Before each prospective protocol is used, calibrate and freeze its screening,
confirmation, stopping and statistical rules; obtain one Astra protocol review.
Concentrate bounded confirmation on unresolved workloads while retaining all
observations and the unchanged allowance. Historical campaigns retain their
frozen rule. Trace and profile separately. Verify actual
owners/routes outside the timed interval. Report command time, affected stage time, numerical
work, row passes and admitted/observed memory using existing facilities.
Overlapping ranges alone do not prove equivalence; a geometric mean cannot
overrule a repeatedly slow family. Missing/failed/timed-out cells stay visible.

The default tradeoff envelope is a confirmed slowdown of no more than 5% for
a workload, or 1 ms for a short workload, whichever allowance is larger,
against both its parent and fixed baseline. This is an upper bound for a
documented tradeoff, not a regression entitlement. The affected families and
the simplification or broad benefit purchasing that cost must be declared
before candidate timing. M26 checks whether measurement noise can resolve the
envelope; unresolved results require better measurement, not a wider allowance.
A different envelope requires Johannes's decision before the affected change;
established regressions require correction or a separate explicit decision.
Johannes's M28–M30 architectural override permits accepting an inconclusive
performance result when the reviewed replacement substantially improves code
quality. Retain the original campaign, allowance and unresolved verdict; record
architectural acceptance separately from timing qualification. Scientific gates
remain mandatory. This does not establish final integrated performance acceptance.

Johannes's 2026-10-06 M32 successor authorization permits comparable architectural
acceptance: scientific/transaction gates and Astra review pass, substantial
retirement is delivered, every immediate-parent comparison is within, and only
previously accepted fixed-M26 costs remain, unresolved or established, without
observed worsening beyond their accepted costs. Retain all evidence and raw verdicts.
New established regressions require correction or a separate decision; the
allowance and final integrated performance requirements remain unchanged.

Apply the same envelope to matched 8→16 and 14→28 worker-budget comparisons,
with every other input fixed. Audit the adjacent boundary counts as well.
Plateaus are acceptable; repeated material slowdowns require correction or an
explicit unresolved finding that blocks the relevant final gate. Saturation
may reduce active stage workers under the common resource policy; it cannot
justify a special estimator/option-specific thread ceiling.

Intermediate gates apply this requirement to the operations migrated there.
M26 assigns each inherited cliff to its responsible milestone; earlier work
must not introduce a new regression but is not blocked by an explicitly recorded
route awaiting its scheduled replacement. M30 qualifies automatic FE routes
with the M29 scheduler, and M35 requires the complete integrated envelope.

Additional output or clustering work is assessed against its declared necessary
operations and its matching baseline, not against a fit that does not compute
those results. Reject unexplained extra factorization/absorption, loss of shared
products or abrupt cross-option scaling. The final program must demonstrate
repeatable gains in predeclared expensive workload families and remove the
known covered cliffs; a collection of neutral extractions is insufficient.

### Complexity, retirement and qualification

M26 names the owners, full pipelines, repeated passes and heuristic branches to
retire, with source locations, measured size and replacement budgets. Classify
guards as scientific/support validation, memory admission, kernel scheduling
or inherited performance routing. Lexical dispatch counts alone do not identify
which guards can safely disappear.

Count all adapters, compatibility shims, planning code and memory formulas.
Keep the existing complexity ratchet; no exclusions, minification or silent
baseline refresh. New functions/files meet the 150/1,500 limits. At each
architecture gate, remove the superseded implementation and lower the combined
size of the replaced subsystem. A temporary migration increase needs its
ordinary ratchet reason and a named removal task inside the same milestone.
Do not carry a permanent second engine or postpone cleanup until M35.

The program must finish with a substantial net production reduction against
M26, fewer oversized functions/files and fewer independent execution policies.
M26 derives a concrete aggregate reduction target from the named retirement
inventory minus counted replacement budgets, reviewed by Astra and recorded
before implementation. There is no arbitrary repository-wide ceiling or claim
that line count alone measures maintainability. Failure to find substantial
removals requires revising the design before starting a structural tranche.

Johannes's 2026-10-06 [approved budget revision](bench/history/2026-10-06-m34-budget-revision.json)
accepts 11,807 net production lines retired against M26, including replacement
code. The original 15,000 target and 3,193 shortfall remain historical evidence;
the named M34 successor is 7,206 against its original 6,390 budget. These unmet
line-count forecasts cease to block completion. Keep the ratchet, concrete
retirement audits and every scientific/performance/scaling requirement.

Runtime commits require the full local entrypoint, long independent oracles for
core changes, actual parent-next native replay, Mac build/ABI and mapped Stata,
allocation/rollback/Break fixtures. Deliberate changes use the declared science
gate rather than a false bit-identity claim. During Johannes's 2026-10-05
Mac-only authorization, local acceptance replaces the GitHub CI prerequisite;
remote/platform qualification is deferred until he resumes it. Historical CI
failures remain failures. At integration boundaries run five-thread science/repeats,
the licensed 46-fixture suite and the M22 strict-PPML/saved-FE witnesses; extend
existing matrices where the changed support demands it.

The active Mac-only period qualifies scaling and storage on this Mac and makes
no new cross-platform claim. Outside that period, parallel changes require
whole-node SCC qualification before acceptance as a general scaling solution.
Qualify Linux scratch/storage changes and Windows
build/ABI/transactions, with licensed Windows numerical/scratch fixtures where
available. A missing required platform check remains open. Reuse unchanged
artifact-bound evidence; repeat broad suites only for relevant changes or
unresolved concerns. Final combined qualification cannot rely on unrelated
older passing binaries.

## Milestones and dependencies

The sequence is deliberately architectural. Complete subsystem retirements
are implementation/qualification batches, with reviewable `M<n>.<k>` commits
around one responsibility. Stabilize with focused development feedback before
freezing the integrated candidate. Small prerequisites unlock a named larger
retirement inside the same batch. Research may
proceed independently; integrated runtime work follows the gates below. Every
milestone records its removed code, surviving responsibilities, science scope,
performance tradeoffs and artifact identities in a compact bench/history summary.

| Milestone | Deliverable | Required predecessor |
|---|---|---|
| M26 | Frozen scientific/performance envelope and retirement inventory | Qualified current runtime; incorporate terminal M24 evidence |
| M27 | Canonical prepared model and optional cluster metadata | M26 |
| M28 | Common storage operations and owner-based memory admission | M27 |
| M29 | General worker-budget and kernel scheduling policy | M28 |
| M30 | Small FE backend portfolio and common lifecycle | M29 |
| M31 | Shared finite weighted-linear computation | M30 |
| M32 | Inference and output products from one accepted fit | M31 |
| M33 | PPML state, generation and certificate consolidation | M32 |
| M34 | Whole-call integration and remaining route retirement | M33 |
| M35 | Held-out, cross-platform and scalability acceptance | M34 |

### M26 — Establish the replacement contract

Pin source, native binaries, frontend, compiler and datasets. Reuse M24 cost
attribution and the M22/M23 witnesses; do not repeat a completed diagnosis
without a new uncertainty. Map each benchmark to the owner/operation it exercises.
Record phase and allocation boundaries for preparation, FE work, finite fitting,
PPML certification, inference, saved products and publication.

Inventory all automatic route and worker-policy guards, including the narrow
three-FE ceiling, seven-worker recurrence crossover, Schur row/width/thread
gates, residual-overlap eligibility and output-dependent retention. For each,
identify its original purpose, actual support requirement and planned replacement.
Inventory duplicate representations and lifetime/byte formulas separately.

Freeze the interaction matrix, holdouts, performance budgets and quantified
retirement target. Use a separate Astra extra-high review to challenge missing
families, infeasible replacement budgets and accidental loss of independent
certification. Gate: an executable baseline and concrete deletion inventory,
including failed or unqualified cases; no runtime speedup is claimed here.

### M27 — One authoritative prepared model

Unify finalized row identity, source encodings, column recipes, weights and FE
topology ownership for ordinary/categorical models. Compile distinct input
grammar into that representation; preserve factor precedence, omitted/base
terms, interactions, singleton cascades, sample rebasing and effect identity.
Cluster dimensions attach to the finalized sample and may borrow source codes.
DoF rules remain mathematically distinct where required; metadata variants must
not force separate owning estimation pipelines.

Migrate host and core consumers in a coherent replacement batch and remove duplicate
`PreparedOlsData`/`PreparedCategoricalOlsData` ownership, clustered wrappers and
equivalent construction sequences. Public compatibility aliases may remain if
they contain no second state or scientific implementation.
Gate: one finalized owner, demonstrable code/allocation retirement, exact
unchanged preparation outputs/errors, all affected grammar and rebase witnesses,
and no repeated encoding or topology build introduced by clustering/outputs.

### M28 — Storage and memory as execution resources

Define only the block/column operations required by actual FE and finite kernels.
Implement admitted dense and bounded scratch storage beneath them, keeping
`memory(fast|low|auto)` useful and supported. Select concrete operations outside
inner row loops. No forced full matrix, per-row virtual interface or extra bank
solely to make interfaces match.

Co-locate checked capacity estimates with workspace constructors. Compose phase
peaks from actual owned components and lifetimes, including old/new PPML
generations, recovery, strict certificates, inference and output buffers.
Replace duplicated byte formulas and drifting admission/setup flags. Preserve
conservative preflight and refusal before allocation; explicitly qualify any
intentional change in admission boundaries.
Gate: one source for each moved workspace formula, a materially smaller route
calculation, exact unchanged inventory/admission witnesses, bounded allocation
audits and tight-budget failure/cleanup. Remaining mode-specific numerical
orchestration is named for retirement in M31, not hidden in the storage layer.

### M29 — Parallelism governed by work and resources

Use one command worker budget and a few explicit decompositions: row blocks,
level-group work, finite dense algebra and cluster reductions. Bound task size
and worker-local storage; balance by actual work when group degrees are skewed.
Keep reproducible reductions and controlled synchronization. Stage overlap is
allowed only when the same resource accounting admits both stages and measured
bandwidth/scheduling behavior supports it.

Replace feature-specific worker ceilings and scattered concurrency guards with
these policies. Kernel thresholds must have a general work/overhead rationale
and broad evidence across sizes, options and machines; do not centralize the
old collection of benchmark-cell rules in a new planner. Record requested,
resolved-budget and effective-stage concurrency through existing diagnostics
where feasible; explain their distinction in help.
Gate: matched Mac/SCC scaling and held-out shapes for migrated scheduling and
resource operations satisfy the declared envelope under verified fixed
mathematical routes, with bounded allocation/admission across worker counts.
Record inherited auto-route fences and associated cliffs for M30; do not call
their removal or whole-program scaling complete here. Numerical reduction
changes are declared and independently checked.

### M30 — FE families with a common scientific lifecycle

Provide coarse operations for setup, weight refresh, batched projection,
coefficient recovery and certification. Preserve no-FE/direct one-FE treatment;
qualify one broadly useful two-FE graph family if it earns its complexity;
use a general family for higher-dimensional/slope designs. Preserve supported
explicit backends and genuine support restrictions. Avoid forcing equivalent
backend storage or one inner-loop implementation on different mathematics.

Consolidate duplicated FE indexing, setup/recovery lifecycles, admission and
certificates where their mathematical authority agrees. Replace narrow/wide
auto pilot trees and thread/output/VCE-dependent eligibility with a small
structural policy. Any adaptive fallback needs a numerical progress or resource
reason, bounded work and explicit accounting, rather than a benchmark-cell fence.
Gate: retire superseded auto routes and owners; independently qualify weak,
disconnected, weighted, slope, multiple-RHS and saved-FE cases across supported
solvers. Qualify the automatic FE families with M29's worker policy, including
the inherited routing cliffs assigned here. Account for startup, iterative and
recovery cost separately; an average gain cannot hide a failed graph family or
lose the M22 reconstruction protection.

### M31 — One finite weighted-linear implementation by operation

Unify scaling, rank authority, stable products, factorization/refinement and
certification used by one-shot OLS and PPML retained/frozen work. Dense and
scratch storage may retain different physical kernels. Strict rank discovery
and frozen-basis use remain explicit scientific operations within one service.
Do not replace stable solves with unchecked normal equations to obtain speed.

Retire duplicated end-to-end Fast/Low fits and equivalent retained/frozen
assembly. Reuse row/block work and rounded normalization where valid; incorporate
terminal M25 evidence if available. Fusion must preserve required independence
of certificates and be checked in compiled code, including call boundaries,
vectorization, scratch traffic and instruction work.
Gate: fewer full fitting implementations and repeated scans, identical public
storage capabilities, independent rank/weak-design/refinement witnesses, and
affected-path gains over both small and large held-out shapes. Qualified losses
must fit the predeclared tradeoff envelope; retain useful separate kernels when
one generic loop would erase the gain.

### M32 — Inference and outputs compose with the accepted fit

Represent required final products explicitly: coefficients, bread, scores,
residuals, predictions, FE sums/channels and bias inputs. Compute each shared
product once when its dependencies allow; consumers request extra work through
bounded stages with visible lifetimes. Cluster reductions and saved-effect
recovery use the same accepted sample, basis, weights and solver certificate.

Retire duplicated result assembly, score/residual reconstruction and output-
dependent fitting variants. Preserve factor/DoF metadata, robust and multiway
covariance, full-matrix PSD policy, normalized effect channels and publication
transactions. Keep independently required raw-model checks independent.
Gate: the full thread × outputs × VCE interaction cube, including combined saved
effects/residuals and clustering, has exact scientific decisions and predictable
incremental work. Saved products cannot silently change the fitted estimator or
trigger an unrelated fitting route to compensate for a missing shared product.

### M33 — PPML generations and numerical state

Build directions and accepted/final products through the common FE/WLS services.
Give every retained cache an explicit sample, topology, weight and basis identity
and an invalidation rule. Retire equivalent canonical/specialized setup and
refresh work; keep distinct state only when it has a demonstrated simultaneous
scientific role. Weights changing does not by itself justify rebuilding topology.

Keep accepted/candidate transactions, frozen-basis validation, rank rediscovery,
separation generations, strict recovery, two accepted passes where required and
final-Hessian synchronization. Reuse proofs must preserve objective/score/model
consistency and both bias modes. Never delete certification because a speculative
candidate already computed a similar quantity.
Gate: fewer independent sessions/caches/reconstruction sequences and repeated
weight-only work; separation/rebase, checkpoint rollback, allocation refusal,
strict known-optimum and bias witnesses pass; PPML workload-family performance
improves without trading away M22 accuracy or creating output-dependent cliffs.

### M34 — Integrate and finish the retirements

Consolidate the host around one preparation boundary, shared typed fit products
and one publication discipline for each estimator. Remove temporary adapters,
obsolete dispatch options internal to next, dead caches, duplicate planning
tables and superseded code identified by M26. Keep public feature/solver/memory
controls and legacy protocol compatibility. Audit every remaining heuristic
against its classified mathematical, resource or kernel purpose.

Refresh the actual module map, dependency directions, ownership/lifetime map and
feature-to-test coverage. Inspect major drivers for responsibilities left behind
by extraction; do not satisfy function limits through arbitrary forwarding layers.
Gate: the audited retirements and retained scientific responsibilities are
accepted under the approved scope, no migration duplication remains, and combined
runtime/transaction evidence passes. The 2026-10-06 budget revision resolves the
recorded line-count misses; it does not waive a source defect or scientific gate.

### M35 — Demonstrate the clean version

Qualify the integrated artifact against the frozen program baseline and the
last qualified predecessor. Run the declared interaction matrix and held-out
families, Mac ladders and scientific/repeat/transaction suites. Whole-node SCC,
Windows qualification and final Linux CI are deferred during the authorized
Mac-only period; report those platform gaps explicitly. Use the inherited
single-thread comparator only where it is a scientifically valid reference.

Publish compact per-family timing, worst-cell/timeout, scaling, memory, route and
complexity summaries with binary/data identities and all admitted tradeoffs.
Keep development and untouched holdout results distinct. A fresh Astra
extra-high review checks scientific scope, concrete deletions, surviving policy,
platform gaps and whether broad performance claims match the evidence.
Gate: the audited target architecture and approved measured retirement scope hold;
predeclared expensive families improve; covered scaling/option cliffs are
resolved within the envelope; required evidence is complete. Otherwise keep
the corresponding gate open with a reproducer and bounded next action.
This is engineering completion, not permission to publish native release assets.

## Execution and reporting discipline

Work on main, one named retirement responsibility at a time, using reviewable
M26–M35 commits. Push after required local acceptance checks; those gates precede
dependent work during the authorized Mac-only period. Keep implementation and
qualification with the lead; use bounded Astra
help for difficult questions and independent acceptance at milestone completion.
Additional reviews resolve concrete issues. Coordinate builds and pause agents
for clean timings; coordinate unrelated workloads without terminating them. SCC work follows the existing 28-core, whole-node, at-most-22-hour rules.

After ten substantive unsuccessful repairs or one working day on a blocker,
record the reproducer, findings and attempts, then ask Johannes for direction.
Do not widen the feature scope, performance allowance or scientific contract to
complete a gate. Independent research may continue behind a blocked integration.

Keep status and one-line log entries only in MILESTONES.md. Store compact
evidence in bench/history and raw results in the existing ignored/local/SCC
locations. This roadmap changes only when scope or policy changes, not after
each test. Update SPEC, active ADRs and help when a qualified change affects
their contracts; keep descriptions of current runtime distinct from targets.
