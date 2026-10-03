# Incremental refactoring: M0–M8

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
After three unsuccessful substantive repairs or a working day on one blocker,
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
