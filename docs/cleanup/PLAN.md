# Incremental refactoring: M0–M21

Approved by Johannes on 2026-10-02. This replaces the clean-core rebuild plan.
Historical plan and evidence remain at 15a57ba285a3afede2e3337fe8598e72529a4090.
[AGENTS](../../AGENTS.md), [SPEC](../SPEC.md) and the [active ADR index](../adr/README.md)
define current rules; [MILESTONES](../../MILESTONES.md) alone tracks progress.

## Objective and acceptance policy

Move the complete working implementation into next, including the plugin host.
Both commands default to next after M1 integration. Bundle a separate frozen
legacy plugin. There is no next-to-legacy fallback and no broad numerical rewrite.
Retain full functionality, supported solvers, both bias modes and existing safety
repairs. Preserve initial algorithms/options/dependencies until scoped changes.

Correctness comes from SPEC and independent evidence; legacy and pinned packages
are comparisons. Never copy a bug or reject simplification merely for parity.
Keep fixed-thread deterministic repeatability and scientific agreement across
threads. Cross-thread bits are not required. Initial-copy and no-numerical-change
checks are bit-exact, excluding environment/build diagnostics. Intentional fixes
declare cases/fields; unaffected results remain exact, affected results satisfy
independent checks. Nonunique FE coefficients are compared through identified sums.

A failed/unverified gate stops dependent work. Independent work may continue.
After ten unsuccessful substantive repairs or a working day on one blocker,
record the reproducer/findings/attempts and ask Johannes. Use risk-proportionate
existing tests; no new cancellation/evidence framework. Tasks are small commits
with local checks and pushes. Do not silently expand M0–M1.

## M0: reset and comparison foundation

### M0.1 Governance reset

Update instructions, SPEC, short module-map ARCHITECTURE, plan and ledger.
Historical legacy ADRs are non-binding except requirements incorporated into
SPEC. Give all staged 0001–0011 explicit keep/defer/superseded dispositions and
record short successor ADRs. Covariance is resolved: full-matrix OLS multiway
PSD repair includes the reported constant. No scheduled legacy retirement or
aggregate production/plugin ceilings survive. Keep GPLv3 direction/G-LICENSE.

Checks: normal local harness, active-doc consistency, ledger <300 lines and
all lines <=200 characters. Check the ledger in CI. Current task and blockers
stay concise; log entries are single lines linking detailed evidence.

### M0.2 Frozen comparator

Pin 15a57ba285a3afede2e3337fe8598e72529a4090 as immutable legacy-comparator-v1.
Capture complete core/host/C bridge/lock/vendor/toolchain/build identities.
Requalify this exact build: vendored Rayon changed after the old Stata baseline.
Build canonical macOS universal, Linux x86-64 and Windows x86-64 artifacts;
store source/build identity and hashes in a compact manifest. Store binaries as
tag assets, initially authenticated draft assets pending the existing licensing
gate. Initial CI builds qualify the comparator once; later jobs cache/download
by canonical hash. A cache miss never substitutes a rebuild. Rebuilds remain
separately identified diagnostics. Normal next builds cannot overwrite legacy.

Checks: source/tag identity, hashes, native audit/mock checks and licensed Mac
baseline fixtures. Platform identities may qualify independently; no missing
platform evidence is called passed. Pinning source precedes M0.3; M0.3 need not
wait for unrelated platform jobs to finish.

### M0.3 Working copy

Use git mv for working core/testkit into active crates. Keep the existing host,
point it at the active core and remove abandoned engine/scaffolding/dependencies.
Retain useful independent oracles/witnesses and production fixtures without
porting the whole discarded test suite. Update workspace/build/import/doc paths.
No numerical/routing/dependency/option changes. Preserve CG rejection/CMG repair.

Checks: normal harness, active path checks and native Mac build/mock tests.

### M0.4 Actual binary replay and bounded matrix

Extend the existing actual_plugin_loader_test C harness with replay rather than
inventing a second harness. Inputs contain columns, masks, existing wire/request
metadata and SPI matrices/macros. Execute complete estimate-ols and estimate-ppml;
outputs contain return code, result matrices, observation outputs and solver
metadata, with lossless float bits. Load each plugin in an isolated process.

Reuse oracle seeded generators; commit a bounded representative corpus and a
feature-to-test matrix before fixes. Replay identical inputs against both real
release binaries and independently computed dense answers. Separate:
1. One-thread deterministic initial-copy bit identity.
2. Parent-next versus candidate-next exact-change checks.
3. Independent science, including bugs shared by both binaries.
Known defects have specific expectations and independent intended results,
never blanket skipped tests or claims of correctness. Include OLS/PPML, FE forms,
weights, inference, explicit solvers, outputs, separation and both bias modes.
Map frontend-only behavior to existing licensed fixtures. Linux push/PR CI runs
real binary replay without Stata; preserve assertion-based existing mock checks.

### M0.5 Complexity and coverage

Ratchet production lines per runtime crate, functions >150 lines, files >1,500
and N/thread/weight/memory dispatch conditions; kernel chunk sizes are separate.
Use versioned consistent definitions, excluding tests/generated code. Growth
fails unless a narrow checked-in reason/scope exists; decreases lower baseline.
No silent refresh, minification, renaming inputs or moving code into exclusions.
New functions/files respect limits; inherited excess does not force rewrites.

Run llvm-cov on mapped tests/replay with a separate instrumented next build.
Record reached/unreached functions/modules and hotspots, not a percentage gate.
Keep compact summaries; detailed coverage is a CI artifact, not tracked raw logs.
Coverage and inventory can proceed once their respective code/tests exist.

**G-M0:** canonical artifacts and copied implementation qualified; real OLS/PPML
replay passes Linux CI and exact-copy checks; feature matrix/known exceptions and
complexity/coverage baselines recorded. No M1 numerical correction before G-M0.

## M1: integrate, correct, qualify

### M1.1 Selectable plugins and exact public baseline

Next keeps fereg_<platform>.plugin and __fereg_plugin. Legacy uses
fereg_legacy_<platform>.plugin and __fereg_legacy_plugin. Selection is explicit
engine(), validated FEREG_ENGINE global, then next. Invalid engine gives r(198);
missing selected binary fails clearly. Use the selected handle for diagnostics
and the entire estimation transaction, with no extra substantive call/fallback.
Post e(fereg_engine)/e(feppml_engine); diagnostics identify engine. Print a short
note on every legacy call. Shared ado must preserve the frozen plugin protocol:
legacy comparison means frozen plugin plus current ado, both identities recorded.

Check next/legacy/next switching in one session, OLS/PPML interleaving and
representative public outputs. Reconfirm exact baseline before corrections.

### M1.2 One focused commit per correction

In order: factor precedence; full-matrix OLS multiway covariance repair;
reproduce/repair slope-DoF validation; reproduce/repair frequency-singleton
validation. Declare target cases/fields before each change. Prove original
failure and independent corrected result; unaffected science must remain exact
against preceding next. Record named legacy/reghdfe differences; never silently
edit parity expectations. Recheck CMG repair and CG rejection without a rewrite.

Prefer Rust fixes. fvexpand runs before Rust; if factor handling needs a frontend
adjustment, scope it to next and prove legacy behavior unchanged in Stata. Do
not duplicate whole frontend pipelines. A substantial architectural blocker
requires a scoped follow-up decision, not expansion of this milestone.

### M1.3 Bounded qualification

Run M0's declared matrix; add tests only for mapped gaps/concrete failures.
Reuse allocation, interruption, panic and rollback fixtures. Scientific tolerances
for well-conditioned cases: coefficients rtol=1e-7/atol=1e-10; SE/VCE 1e-6/1e-10;
fitted/residual 1e-7/1e-9. Keep stronger regressions and weak-design oracles.
Exact applicable discrete decisions and identified FE combinations remain.
Check scientific thread agreement and fixed-thread deterministic repeatability.
Tolerance passes never waive unexplained exact-change failures.

### M1.4 Installation/platform checks

Bundle both plugins/checksums; update manifests/help without renaming next.
Test defaults, explicit/global selection, diagnostics, stored/restored estimates,
alternating/repeated calls, missing binaries and failure transactions. Verify
legacy protocol compatibility and unchanged canonical hashes after next builds.
Licensed Mac Stata plus Linux/Windows builds, audits and mock SPI are required;
state limits accurately. External public release is not part of these milestones.

### M1.5 Performance and refreshed coverage

Use the existing runner with engine/binary-hash/frontend identity. Mac smoke and
targeted OLS/PPML scaling use identical data at 1/2/4/8/16 native threads, one
warm-up and five measurements, including prior routing boundaries. Investigate
changes beyond noise and attribute correctness-fix costs; never add narrow
routing fences to rescue cells. Store compact bench/history summaries and
refresh coverage/complexity. Defer large SCC work until routing/parallel changes.

**G-M1:** defaults/selection, corrections, exact unaffected outputs, bounded
qualification, bundled installation and identity-bound summaries pass. Then
choose one simplification from measured complexity AND exercised code paths.
Stop at M1; no unapproved broad rewrite or automatic deferred-ADR work.


## M2: shared OLS regressor classification

Authorized by Johannes on 2026-10-03 after reviewing M0–M1 results. This extends
the plan; the earlier stop at M1 governed the initial implementation only.

### M2.1 Boundary and parent baseline

Pin the qualified M1 next binary as the immediate comparator and record fresh
Mac smoke/scaling measurements with the existing runner. Audit fast/low QR
classification and validation order. Both paths defer absorbed intercepts and
classify zero reference columns, FE-collinear columns and finite candidates.
Fast checks the reference norm before skipping a zero reference; low checks
both norms first. Preserve these checks and their errors at the caller boundary.

### M2.2 Share classification and qualify correctness

Extract one shared classification implementation; delete the duplicated rule
from both QR paths. Preserve allocation order/capacity checks, column order,
error precedence, arithmetic, solver selection and the two storage strategies.
Use small static validation functions or closures if needed; no runtime mode
switch or generic engine framework. Add only focused checks for an uncovered
classification/validation boundary. No ADR is needed for this internal extraction.

Run the normal harness, long independent oracle tests, focused fast/low Stata
regressions and actual-plugin replay against parent-next with all three M1
fixes enabled for both. Require exact scientific outputs and route signatures,
independent scientific acceptance and fixed-thread repeats. Reuse the five-thread
matrix. No intentional-difference allowance is added for this extraction.

### M2.3 Measure, document and stop

Require a net reduction in duplicated production code, no new routing condition,
and a lowered complexity baseline. Compare the stable candidate with the parent
using existing Mac smoke and 1/2/4/8/16-thread scaling cells, one warm-up and five
measurements. Investigate differences outside observed noise with bounded paired
follow-up. Bind evidence to actual binary, source and frontend identities.
Retain current package/ABI contracts; reuse installation/platform evidence where
unchanged. Record compact evidence and discuss the next milestone with Johannes.
Do not implement M3. The existing blocker timebox applies.


## M3–M7: authorized incremental follow-through

Johannes authorized the next five milestones on 2026-10-03. This supersedes
M2's stop instruction. These are incremental milestones, not the abandoned
rebuild's similarly numbered tasks; frozen legacy remains available.

- **M3 — OLS completion:** replace six repeated post-fit sequences with one
  completion boundary using existing borrowed result views. Preserve output
  normalization, allocation admission, diagnostics and failure transactions.
- **M4 — QR completion:** share the fast/low triangular solve, coefficient/status
  finalization and matching rank decisions. Keep storage access and allocations
  at their existing boundaries; preserve arithmetic and error precedence.
  Shared QR invariant failures use common wording, retaining error codes;
  path-specific norm validation remains intact.
- **M5 — Wald calculation:** consolidate robust/clustered finite-coefficient
  Wald work where the algebra and rank policy agree. Keep estimator-specific
  covariance construction, PSD policy and independently checked certificates.
  Reuse finite-coordinate lookup for IID too; shared internal Wald failures
  use common wording with unchanged error codes and precedence.
- **M6 — categorical DoF:** consolidate repeated deferred categorical DoF input
  consumption and report construction using the existing inference view. Keep
  ordinary/categorical mathematics, cluster nesting and error timing intact.
  Share checked report-total accumulation across validated intercept/slope
  rows; retain their distinct identity checks and common overflow diagnostics.
- **M7 — scaling diagnosis and combined qualification:** use existing phase
  timers on one reproducible OLS scaling slowdown; separate startup, frontend,
  absorption and finite-fit costs. Record bounded evidence and the justified
  next change. Do not invent a repair where attribution is inconclusive or
  change solver policy without a scoped decision. Complete combined scientific,
  transaction, thread and performance checks and refresh the architecture map.

Each code milestone must delete its superseded implementation, reduce net
production complexity, and pass the full local harness plus mapped witnesses.
Use actual-plugin parent-next replay at one thread for nonnumerical changes;
keep exact scientific fields and available routes, without new exceptions.
Run long oracles for core changes. Preserve each qualified native predecessor.
Independent milestones may proceed while hosted CI runs; a failed gate blocks
its dependents. The final runtime must pass full Linux CI/native replay.

Reuse existing tests/benchmarks; add only demonstrated coverage gaps. Run the
five-thread matrix and licensed output/transaction fixtures on the combined
candidate. Compare clean final and M2 binaries with the existing smoke/scaling
protocol; investigate differences outside observed noise. No new framework,
platform/package contract, scientific tolerance or legacy mutation is in scope.
The existing blocker timebox applies. Stop after M7 and discuss next steps.


## M8: shared Schur RHS preparation

Authorized by Johannes on 2026-10-03. This supersedes the M7 stop instruction.
The M7 proposal is narrowed by the preparation-order audit: wide/narrow probes
share their entire level-total/RHS/left-projection setup; full Schur solves use
the same level-sum and projection kernels, with their original validation and
normalization boundaries. Full solves validate group totals and unnormalized
products before dividing. Probes divide immediately and retain their existing
nonfinite admission policy. These differences must remain explicit.

### M8.1 Boundary and baseline

Preserve the qualified M7 next Mac binary (runtime source b3fa58a7) and record
fresh smoke and OLS/PPML scaling with the existing runner, one warm-up and five
measurements. Audit active/tail clearing, level-member traversal, selected RHS
order (including repeated columns), weighted multiplication and projection
subtraction. Map accepted/rejected probes, narrow continuation, weak weighted
cuts, saved effects, full/low modes and allocation failures to existing checks.

### M8.2 Consolidate and qualify

Use one allocation-free level-sum kernel and one left-projection kernel.
Both probes call one preparation stage; the full solve uses those kernels at
its existing validation boundaries. Delete the duplicate loops. Keep fixed
row/column arithmetic order, inactive-lane clearing, allocations, normalized
weights, probe policy, convergence, certificates, fallbacks and solver routes.
Do not retain the probe workspace across the full solve or add a size/thread
route. No new generic engine or representation is needed. Require a net
production reduction. The measured column-access repair may add three lines
relative to the first M8 commit, with a narrow ratchet reason; M8 remains
smaller than the qualified M7 parent.

Run the full harness, long oracle tests, mapped Schur/weak-graph regressions,
Mac native build/ABI, 73-case exact parent-next replay with the five-thread
scientific/repeat matrix, and mapped licensed Stata fixtures. The clean final
runtime must pass Linux CI/native replay. No new intentional scientific
allowance is authorized for this arithmetic-preserving consolidation.

### M8.3 Measure, document and discuss

Compare clean final and parent binaries using the same smoke/scaling protocol;
investigate differences outside observed noise with bounded paired follow-up.
Use the existing phase timer on the M7 OLS cell. Record compact identities,
checks, complexity and timing evidence. A maintenance gain does not imply a
speedup or a repair of the 8-to-16-thread slowdown. Preserve frozen legacy.
Discuss three substantive follow-up milestones, M9–M11, with explicit boundaries,
risks and acceptance checks. Do not implement those proposals in M8.


## M9–M11: authorized follow-through

Johannes authorized implementation of the next three milestones on 2026-10-03.
This supersedes M8's discussion-only boundary. Complete the pending M8 gates;
independent OLS/PPML work may proceed while hosted Schur qualification runs.
Each milestone removes a named repeated decision or produces a bounded,
conclusive performance experiment; do not ship an unearned scheduling change.

### M9: one QR factorization driver, two storage adapters

M2/M4 share classification, pivot choice, triangular solution and coefficient
finalization, but `fit_scaled_pivoted_qr` and its low-memory counterpart still
own two rank/Householder loops. The corresponding functions are 406/399 lines.
One driver should own pivot order, stopping rules, reflector scalar decisions,
rank/condition bookkeeping and dropped-span certification. Dense storage and
scratch storage should supply only the column access, norm and reflector-update
operations they actually need. Keep the storage strategies and public memory
options; use static calls and existing borrowed buffers, not a general engine
framework or virtual dispatch per observation.

Start by characterizing the real differences. Dense QR refreshes trailing norms
before the next pivot and can reuse its selected pivot norm. Scratch QR updates
norms while writing each target and rereads its pivot. Dense updates can run in
parallel; scratch needs admitted reflector/target buffers and I/O. Rank-zero
behavior, weighting/standardization and validation precedence also differ.
Preserve these boundaries first. Do not obtain one loop by materializing the
low-memory matrix or by adding mode branches throughout a new driver.

Implement in small tasks: characterize norm/reflector boundaries with existing
fast/low witnesses; introduce the limited storage operations; move the common
rank progression and delete the old loops. Reuse the shared M2/M4 operations.
Count the adapters as production code: reject a design whose scaffolding
cancels its maintenance gain. This is a larger risk than M8 because a pivot
change can affect rank, reported coefficients and covariance.

Acceptance: one source of rank progression; smaller combined production code
and no new size violation/route; exact applicable parent outputs; independent
full/weak/rank-deficient tests with weights, omitted and absorbed columns;
fast/low memory and allocation failures; fixed-thread repeats and scientific
thread agreement; saved outputs and Stata transactions; paired smoke/scaling
and representative wide QR timings. Shared internal QR invariant errors may use common wording with unchanged
codes and precedence; storage-specific validation errors remain distinct.
Arithmetic changes require an explicitly
scoped numerical change and independent evidence, not an unexplained parity
exception. A useful memory path remains even if its timing is slower.

### M10: scheduling in the shared Schur level kernels

M8 gives the probe and full solve one level-accumulation kernel. M7's timers
showed an 8-to-16-thread rise inside absorption, with unchanged route/iterations;
its cause was not established. Diagnose task granularity, imbalance and memory
traffic at that shared boundary before changing execution. Use balanced,
skewed-degree, weighted and weak-cut designs at 1/2/4/8/16 threads and widths
that exercise selected-column probes, narrow continuation and full solves.

Test a bounded scheduling change: fixed groups of independent FE levels or a
fixed grain within the existing iterator. Keep each level's member/RHS order
and its scalar accumulation; the scheduling change must not alter the operator,
certificate, stopping rules or numerical route. Choose the grain from comparative
evidence across the suite, without N/thread/width selection fences. A workload
with uneven level sizes is a required check: fewer tasks can worsen imbalance.
Keep the existing residual-overlap path during this milestone; a second scheduling
experiment should not obscure attribution of the first.

Acceptance: a repeatable gain outside noise in the geometric mean of the declared
representative suite, with no repeatable regression outside noise; exact parent
scientific outputs and routes for arithmetic-preserving scheduling; independent
weak-graph tests, thread repeatability and allocation/Break checks. Compare
uninstrumented binaries and retain phase diagnostics separately. Only launch a
whole-node SCC scaling campaign when the candidate merits larger-data/28-thread
qualification. If the scheduling hypothesis is disproved, record M10 as a completed negative
experiment with unchanged scheduling, and discuss a new bounded target; do not manufacture a speed milestone by adding
a cell-specific route. M10 can proceed independently if M9 is blocked.

### M11: common PPML memory inventory and storage contributions

`ppml/memory.rs::estimate_route` is a 1,208-line calculation, evaluated for fast
and low modes. It mixes immutable input/topology/row-ledger costs, storage-specific
workspaces and phase-overlap peaks. Solver admission in `prepared_irls.rs` adds
further owner/overlap accounting. Separate these responsibilities into a compact
common component inventory, storage contributions and explicit phase lifetimes.
The current estimator already has one source of formulas evaluated twice;
sharing execution alone is insufficient. Compute genuinely common components
once and clarify their owners/lifetimes; route-dependent capacities remain
explicit. Consolidate a repeated formula only after identifying its owner and
simultaneously live buffers. Reuse existing capacity estimators rather than
introducing a second memory model.

Preserve public memory options, telemetry fields, conservative bounds, preflight
failure timing, solver admission and separation/rebase semantics. The largest
risk is an omitted lifetime or double count, especially old/new prepared
generations during separation and buffers that overlap strict certification,
inference or saved-effect recovery. Optional reuse mechanisms must keep their
existing admission order in this milestone. Changing memory policy or removing
an optimization is a separate decision; do not use this refactor to introduce
one memory mode or rewrite IRLS.

Acceptance: explicit owners/lifetimes for every moved component; identical
scalar inventory/peak/admission results on the existing dimension and overflow
matrix; conservative bounds against isolated allocation audits; tight-budget
failure before allocation; separation/rebase, explicit solvers, both bias modes,
fast/low and final-publication fixtures; native scientific/repeat qualification.
Require fewer repeated formulas and a materially simpler calculation after
counting new component code. This is groundwork for later PPML orchestration
cleanup, not a claim of faster IRLS. It can proceed independently of M9/M10.

After these three, ordinary/categorical preparation, FE routing and PPML's
large orchestration still require their own scoped work. Progress should be
judged by the repeated decisions and stages removed, alongside measured
correctness/performance, rather than by the milestone number alone.


Each runtime task uses the existing full harness, long oracles, mapped witnesses,
actual parent-next replay and native build/ABI checks. Final combined runtime
must pass Linux CI/native replay, five-thread science/repeats and mapped licensed
fixtures. Keep exact scientific fields and routes for these nonnumerical changes.
Compare clean binaries with the existing smoke/scaling protocol and targeted
changed-path checks. Preserve qualified predecessors and frozen legacy hashes.
Count all new adapters/components in the complexity ratchet; no silent baseline
refresh or growth exclusions. M9–M11 concludes with the requested Astra max review. Johannes subsequently
authorized the following ten milestones, planned by an Astra high-effort agent
on 2026-10-03, with a separate Astra review after implementation.


## M12–M21: retire redundant stages and competing owners

These are nonnumerical refactors. Preserve every public feature, memory mode,
solver route, admission predicate, error precedence and scientific acceptance.
No new size, weight or thread routing fence. Legacy remains frozen. A completed
milestone must delete an unnecessary stage, encoding, owner or repeated decision;
wrapping or relocating the same complexity does not satisfy acceptance.

For each runtime task run the full local harness, long oracles for core changes,
Mac build/ABI, 73-case actual-plugin replay against the immediate qualified parent,
and mapped licensed fixtures. Push after local gates; Linux CI/native replay is
a prerequisite for dependent milestones. Preserve qualified binaries. At the
three tranche boundaries (M14, M17, M21) run all five thread counts and 46 licensed
fixtures. Exact scientific fields and available routes must match the parent;
independent science and fixed-thread repeats remain authoritative. An eliminated
allocation may retire its injection site; record it and retain surviving rollback
coverage. No new replay exception.

Use clean predecessor/candidate binaries for existing warm-up/five-measurement
protocols, changed-path cells and tranche smoke/scaling. Investigate out-of-noise
differences with bounded reverse-order follow-up; no speed gain is presumed.
Count every adapter in the ratchet and document narrow necessary growth. Keep
preflight conservative while removing preparation allocations; do not spend the
slack on different numerical routes. SCC/Windows qualification is triggered by
actual parallel, scale, platform, scratch or ABI changes, not milestone numbers.

### M12: categorical availability owner

Replace the temporary full ordinary `PreparedOlsData` with an internal compiled
model, classified sample and encoded-source owner. First separate source coding
and recipe resolution from ordinary FE finalization, then use that boundary in
categorical preparation. Remove disabled-singleton packaging and ordinary topology
construction/validation where discarded. Preserve owned early factor overlap and
large unweighted single-source membership transfer, source order, model recipes,
classification, factor-domain checks and independent final categorical validation.

Witnesses: numeric preparation/error precedence, factor-overlap/membership reuse,
final-sample rebasing, cluster preparation, factor availability, interaction
missing/cascade and PPML generation rollback. Acceptance: disposable ordinary FE
topology disappears from the general categorical path; allocation observation
identifies retired work and transferred indexes retain their existing owner.

### M13: source-role categorical encoding

After the complete authoritative source-use classifier, narrow encoding by actual
role: partition, ordinary factor or cluster. Continuous slope use alone does not
require categorical codes. Preserve mixed roles, missing-slope FE exclusion and
validation of base/omitted factor terms. Delete both restricted continuous-only
`compact_slope_availability` branches and their N/model-shape gates. Keep effect
identity/order and recipes. Requires M12.

Witnesses: existing OLS/PPML compact-slope comparisons, mixed slope/factor/cluster
roles, factor availability, intercept/slope and multipartition Poisson oracles.
Acceptance: no slope-only N-row code or membership construction on any supported
shape; exact sample, final recipes/topology and errors with no new route fence.

### M14: move unchanged categorical encodings

Consume candidate non-FE ordinary/cluster encodings when the authoritative
singleton mask proves the row axis unchanged, including no-FE PPML. If rows
change, retain final factorization/order. Topology-backed sources continue to
borrow one N-row code owner. No clones, persistent dual copies or generation
cache. Equal counts or `keepsingletons` alone are insufficient proof. Requires
M12; implement after M13.

Witnesses: vanished levels/bases, reordered interactions, cluster/shared-source
preparation, probability-weight generations and separation rebase rollback.
Acceptance: one factorization removed per eligible source, demonstrable buffer
ownership transfer, exact changed-axis recipes and report slots.

### M15: three certified OLS VCE owners

Replace ordinary/categorical result pairs in IID, robust and clustered modules
with one report-parameterized owner per VCE. Keep concrete absorbed-DoF types,
public names/signatures via aliases/default parameters, source-bound construction
and original covariance/certificate validators. Delete duplicate layouts/getters
and assembly. Specialized byte-accounting methods are acceptable; no universal
inference framework, new public trait solely for reports, boxing or erasure.

Witnesses: all six inference oracles, malformed DoF/source-bound perturbation,
full-matrix PSD, saved output and eclass. Acceptance: six implementations become
three owners; exact byte totals/scientific outputs; no native protocol change.
Independent of M12–M14.

### M16: VCE-independent OLS preparation

Reduce six complete host estimation pipelines to four preparation dispositions
(ordinary/categorical, with/without clusters), followed by typed inference and
common output admission/completion. Use a small host-local owner; no cross-crate
engine framework. Preserve moment overlap, loaded-row ownership, metadata drop
boundaries, timing labels, main-thread SPI and error precedence. Requires M15.

Witnesses: loaded moments/rows, memory admission, six results, OLS transactions,
hidden publication failure, engine switching, parser/cluster. Acceptance: IID and
robust share preparation/lifetime discipline; a shorter outer sequence and less
combined host code, counting new helpers.

### M17: one active OLS FE owner

Map all `FitFeAbsorber` constructor/mutation states, then replace exclusive slope,
single-slope, one-FE, CMG, retained-LSMR and pending-Schur owners with an enum.
Keep genuine route/probe history and discarded-work diagnostics separately.
Migrate construction/dispatch and low-memory/final-residual mutations in predicate
order. Drop replaced storage at its existing admission boundary. Preserve raw
CMG versus fast polish, low LSMR death before QR, single-slope rejection budget
and original-value retry, and accepted Schur probe semantics.

Witnesses: slope ownership/admission, Schur guards, residual overlap/error order,
explicit solvers, weak decimal oracles and raw-CMG Stata. Acceptance: one active
storage owner, explicit transitions, no option-precedence dispatch or incompatible
None-fill constructors; exact routes, work counts and admission.

### M18: ordered PPML installation admission

Make one owner hold the evolving admitted memory plan, execution bytes and setup
choices, consumed once by installation. Preserve public/explicit admission and
auto-LSMR veto, then profile, curve, centered storage, fast canonical allowance,
solver/projection, certificate geometry/options/slopes and low checkpoint in
exact order. Centralize checked transactional updates; refused later options
cannot evict accepted earlier ones. Keep existing byte estimators, Fast/Low
policy, error order and conservative bounds. Independent of OLS changes.

Witnesses: 312 shapes, 71-field route inventories, 2,808 admissions, constrained
memory/CMG early admission, checkpoint refusal, allocation audits and hidden PPML
failure. Acceptance: one plan/installation boundary, no drifting totals/flags,
exact inventories/admissions and bounded methods retiring driver responsibility.

### M19: one active PPML direction owner

Replace competing joint-LSMR/identity, CMG and one-FE profile locals with one
ordinary-direction enum, including canonical MAP. Couple LSMR identity to its
workspace. Retain the canonical checkpoint session independently. Keep profiled
curve separate because it coexists with a direction owner. Centralize first-basis
initialization and committed-basis replacement; drop stale storage first. Retain
Mu restrictions. Requires M18.

Witnesses: explicit solvers, warm state/fallback, profile/refusal, slope rollback,
higher-dimensional frames, basis injections and Mu separation. Acceptance: no
impossible competing owners or workspace/identity divergence; exact routes/basis
and live memory; one independent checkpoint session.

### M20: one ordinary PPML canonical retry

Share the canonical retry/certify/account/diagnose/extract sequence with typed
existing reasons. CMG failure refreshes its stale canonical frame and retires the
owner; frozen failure discards invalid state. Preserve profile refusal, fatal
domain/Break/allocation classifications and exact retry eligibility/count. Audit
equivalence stays a distinct measured-window witness. Requires M19.

Witnesses: invalid warm projection, failed retry atomicity, fatal domain/interrupt,
CMG failure, warmed allocation equivalence and hidden transaction. Acceptance:
one retry transition and no duplicated candidate-publication sequence, no added
fallback or numerical policy.

### M21: strict PPML recovery controller

Consolidate recovery flags/tolerance/counters into one controller, then strict
candidate transaction orchestration, then explicit final publication readiness.
Keep numerical transactions/comparisons and distinct final Hessian synchronization.
Preserve rank/basis changes, rollback, monotone tolerance refinement, bounded
recovery, two consecutive accepted passes, objective/score and accepted replay.
Use bounded methods, not an oversized relocated block. Requires M19–M20.

Witnesses: moving checkpoints, same-rank pivot, persistent profile failure/bounds,
tiny tolerance, canonical reconstruction, weight-frame reuse, rollback/unwind,
final-Hessian dense oracles and bias recovery precision. Acceptance: one pending
recovery/publication owner, no overlapping driver flags; exact iteration/routes/
receipts and independently authoritative final Hessian/rank/score.

Implement in three tranches: M12–M14, M15–M17, M18–M21. A blocker stops dependents,
not unrelated work. Existing timeboxes apply; scientific/domain/rank/support/
admission-policy ambiguities require a reproducer and Johannes's decision.
After implementation a separate Astra agent reviews concrete removals, ownership,
coverage, complexity and measured performance. Report all limitations accurately.


## M22: two numerical witnesses

Authorized by Johannes after M12–M21 completion. Reproduce the strict no-FE
PPML r(430) and saved-FE near-within CG r(459) on the qualified M21 binary;
localize numerical error before choosing each repair. Preserve requested
models, solvers, memory modes, tolerances, transactions and fail-closed checks.
Keep independent known-optimum/explicit-design regressions. Declare affected
cases/fields and require exact unaffected parent science; never use legacy
parity, tolerance weakening or backend rerouting to hide a defect.

M22.1 repairs no-FE PPML first; M22.2 repairs CMG's CG polishing and downstream
saved-FE recovery, localized as distinct numerical stages of the public witness.
Each runs full local checks, long oracles, native Mac build/ABI, parent replay,
mapped licensed fixtures and Linux CI. Exercise each new witness across its
relevant solver/memory/weight/thread configurations. Record original failures,
localization, repair attempts and terminal results in compact bench/history
summaries; existing timeboxes and gates apply. After both qualify, a separate
Astra extra-high review assesses the repairs and unresolved limits (M22.3).

If fixed and reviewed, Astra extra-high proposes one next important milestone
(M23) towards a cleaner, maintainable and consistently faster package. The
user authorizes implementing that scoped plan, then another Astra extra-high
review and next-step assessment. Choose scope from concrete numerical,
duplicated-work or measured performance evidence rather than a driver-size
target. No unbounded structural tranche or numerical policy waiver is implied.
