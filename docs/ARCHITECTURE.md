# Incremental refactoring architecture

[SPEC](SPEC.md) governs scientific behavior; [PLAN](cleanup/PLAN.md) defines
M0–M25. [MILESTONES](../MILESTONES.md) distinguishes planned from completed work.
The [second program](../milestones2.md) and [replacement contract](cleanup/REPLACEMENTS.md)
define the remaining scoped retirements; M27 preparation and M28 capacity/storage ownership are accepted.
The previous clean-core engine is abandoned, not a future integration target.

## Module map

| Component | Responsibility |
|---|---|
| Shared Stata ado/Mata | Syntax, model metadata, private outputs, posting and rollback |
| C bridge | Stata ABI and native entry point |
| Active plugin host | Request validation, main-thread SPI, core calls, result validation/publication |
| Working Rust core | Samples, FE absorption, OLS/PPML estimation, inference, separation and bias correction |
| Testkit/oracle | Existing contracts, independent dense designs and seeded replay inputs |

M0.3 moved the working core/testkit from legacy into the active crates without
numerical changes; M1 applies four independently qualified corrections.
Existing duplicated representations, memory modes and routes remain until
individually scoped refactors remove them.

## Two-plugin boundary

Next keeps `fereg_<platform>.plugin` and `__fereg_plugin`. The frozen comparator
uses `fereg_legacy_<platform>.plugin` and `__fereg_legacy_plugin`. The selector
is explicit engine(), then the validated developer global, then next. It never
falls back between engines. Both commands retain one substantive native call.
No runtime dependency or Rust type adapter connects next to frozen legacy.

The legacy comparator is a canonical stored binary built from the complete
pinned revision, including its host, C bridge and vendored dependencies. The
shared ado remains current and must preserve the frozen native protocol.
Scientific fixes belong in next's Rust path where possible; necessary frontend
exceptions must preserve legacy behavior and have Stata regressions.

## Evidence and hotspots

The actual-plugin mock-SPI replay exercises both binary estimators without
Stata. Scientific oracle checks, exact-change checks and frontend Stata checks
are separate layers. Coverage uses an instrumented next build, never the
canonical comparator or benchmark binary.

The accepted M28 runtime's [versioned inventory](cleanup/complexity.md) measures
104,651 production lines in core and 15,118 in the host.
Core has 92 functions over 150 lines and 18 files over 1,500;
the host has 15 and 3. Its conservative dispatch inventory contains
1,443 core and 158 host candidates, including validation conditions;
15 core chunk conditions are counted separately. These measure complexity;
some conditions validate inputs rather than select algorithms. Local scientific,
transaction gates pass; Astra high reviewed the integrated retirement. Johannes
accepted M28 architecturally while its measured timing remains unresolved.
Total production is 119,769 lines, 1,627 below fixed M26 including all replacements.

Largest inherited files are `fe/borrowed_map.rs` (10,195 production lines),
`ols/prepared_fit.rs` (7,075) and `ols/linear_session.rs` (6,351). The largest
functions are PPML route-memory estimation (1,086 physical lines), PPML IRLS
orchestration (963), and retained-rank solving (972). Fast/Low storage remains
distinct; the preparation, inference and exclusive FE owners described below
replace several earlier forks without changing solver policy.

The [M1 Mac coverage summary](../bench/history/2026-10-02-m1-runtime-coverage-macos.json)
maps the bounded oracle/replay campaign to 2,813 reached source functions,
806 unreached functions and 20 without an exported instance. The
[Linux summary](../bench/history/2026-10-02-m1-runtime-coverage-linux.json)
independently reaches both native entry points and 2,808 source functions;
809 remain unreached and 22 have no exported instance. Each campaign verifies
221 native profiles. Mac LLVM reported two unmatched zero-hash getter records,
listed in its summary; normal export and HTML completed. This is function-entry
coverage, not a branch-coverage or complete scientific-qualification claim.

PPML IRLS, route-memory estimation and retained-rank solving are reached
hotspots; the 666-line reusable two-FE PCG path is not reached by this matrix.
The [M1 timing evidence](../bench/history/2026-10-02-m1-performance.json) shows
overlapping next/legacy ranges for all 34 paired groups and exact emitted
coefficients/samples in 214 paired command runs. Both engines retain higher-thread
slowdowns in some cells. These local measurements do not establish large-data
scaling or promise that simplification alone will improve speed.

M2 replaces the duplicated FE-collinearity/candidate classification with one
31-line helper. The fast and low QR functions shrink from 497/486 to 455/448
lines; their norm-validation precedence and storage/allocation behavior remain
separate. The net reduction is three production lines. Long-function counts
and dispatch inventory are unchanged: this removes a duplicated scientific
rule, not either QR pipeline. The new characterization reaches both real QR
entry points and preserves their error-order/deferred-intercept boundaries.

The [M2 qualification and timing summary](../bench/history/2026-10-03-m2-classification.json)
records exact parent-next replay at one thread, scientific acceptance and
fixed-thread repeats at 1/2/4/8/16, and identical coefficients/samples in 214
paired benchmark commands. All 34 timing ranges overlap. Higher-thread
slowdowns remain; M2 makes no speedup claim.

M3 shares the six OLS completion sequences through `complete_ols`, using the
existing borrowed result views. M4 shares QR pivot selection, triangular solve
and coefficient/status finalization while retaining fast/low storage. M5 gives
robust/clustered inference one Wald solve and shares finite-coordinate lookup
with IID. M6 shares deferred categorical DoF consumption, nesting state and
checked report-total accumulation; the distinct row identity/rank checks remain.
Common internal invariant errors use common wording with the same error codes.
These remove repeated operations without changing numerical routes or public
options. Together they remove 158 production lines, one oversized core file and
two lexical dispatch candidates. The latter are shared QR validation conditions;
they are not two removed solver routes. Long-function counts are unchanged.

The [M3–M7 evidence](../bench/history/2026-10-03-m3-m7-refactoring.json) binds
each change to parent-next replay and local checks. The clean final binary passes
73 scientific cases at five thread counts, exact one-thread comparison with M2,
fixed-thread repeats, 46 licensed Stata fixtures and combined Linux CI/replay.
The initial smoke/scaling timing ratios are 1.040/1.073; 13 PPML pairs have
nonoverlapping ranges. A reversed-order follow-up of all 15 PPML groups has
overlapping ranges and geometric-mean ratio 1.00009. Both campaigns are retained.
The slowdown was not reproduced; these bounded measurements establish neither
performance equivalence nor a speed gain. No new Windows or SCC qualification
is claimed, and the frozen binaries are unchanged on all three platforms.

M7 localizes the existing 100,000-row, 16-regressor OLS slowdown: uninstrumented
estimation medians rise from 43 ms at 8 threads to 53 ms at 16; M2 also rises
from 43 to 51 ms. Existing phase timers put most of the increase inside native
fitting. Initial absorption rises about 2.53 ms, finite QR 0.97 ms and the
overlapped residual/bread interval 1.22 ms. Schur selection, iteration counts
and overlap are the same at 8 and 16 threads. The probe alone rises from
3.21 to 4.43 ms within absorption. Timings are nested and their independent
medians need not add. Process startup/data loading are outside the command
timer; input loading and host preparation change much less. Scheduler overhead,
task granularity, memory traffic and hardware placement remain hypotheses.

## M8: shared Schur preparation

Full Schur solves and both probes now use one level-total/RHS kernel and one
left-projection kernel. Wide/narrow probes share their preparation stage.
Static completion operations preserve immediate probe normalization/projection;
full solves retain raw group-state validation before division. Clearing,
selected-column order, arithmetic, allocation lifetimes and routes remain.
This removes 36 production lines overall and a duplicate preparation stage;
long-function, oversized-file and dispatch/chunk counts are unchanged.
Probe workspace reuse and elimination of repeated runtime scans remain separate
questions; M8 shares source implementations rather than promising those gains.

The first candidate's scalar matrix indexing increased full Schur initialization
beyond the phase ranges at 1/4/8 threads. Restoring contiguous row-slice access
inside the shared kernel addresses that measured regression. A narrowly recorded
three-line ratchet increase relative to the first M8 commit preserves the net
reduction relative to M7. Both the failed timing candidate and repair evidence
are retained in the [M8 summary](../bench/history/2026-10-03-m8-schur-preparation.json).

The clean repaired M8 source passes 73 scientific cases at five thread counts,
46 licensed fixtures and Linux CI 37131877394. Fresh smoke/scaling ratios are
0.980/0.949, with no repeated regression. The evidence is local and bounded;
it does not establish consistent acceleration or explain higher-core cliffs.
Historical dirty diagnostics remain identified in the summary.

## Approved M9–M11

Johannes authorized M9–M11 on 2026-10-03. The implementation scopes, risks
and acceptance checks are in [the plan](cleanup/PLAN.md#m9m11-authorized-follow-through).
Progress remains in MILESTONES. M9 consolidates QR rank progression; M10 tests
Schur scheduling and may retain current execution on a negative result; M11
clarifies PPML memory ownership, storage contributions and phase lifetimes.

M9 now shares pivot selection, rank progression and Householder stopping in
`ols/prepared_fit/qr_factorization.rs`. Dense and scratch adapters retain their
norm refresh and storage behavior. This removes one duplicated policy, with a
net reduction of one production line; inherited large fit functions remain.
[Clean paired evidence](../bench/history/2026-10-03-m9-qr-factorization.json)
shows exact parent science and no repeatable local timing change.

M10 retains the current Rayon scheduler. A fixed minimum grain of eight levels
showed campaign gains that also appeared in inactive controls; fresh results
did not establish an attributable broad benefit. Routes and iteration/certificate
signatures matched. [The bounded experiment](../bench/history/2026-10-03-m10-scheduling.json)
changes no runtime policy and does not resolve the scaling cause.

M11 groups PPML resident/I/O/file storage, prepared-generation payload,
numerical-detector work and final result owners at their existing lifetime
boundaries. One checked predicate handles optional reuse admission. The
71-field public inventory stays exhaustive. Owner totals are reused within
each route; Fast then Low remain separately evaluated to preserve errors.
The calculation falls from 1,208 to 1,154 physical lines, with a net reduction
of twelve production lines after the new helpers. [Scalar and runtime evidence](../bench/history/2026-10-03-m11-memory.json)
checks exact budgets and conservative allocation bounds. The broad OLS
preparation/inference forks, FE routing and PPML IRLS orchestration remain.

The final combined source `a5cd94ba` passes clean Mac five-thread replay and
46 licensed fixtures plus Linux CI/native replay. The 34 local timing ranges
all overlap (smoke/scaling ratios 1.014/0.999). M9–M11 establish maintenance
consolidation, not consistent acceleration. The evidence summary includes the
requested independent Astra max assessment. The following authorized tranche addresses temporary categorical topology and
encoding work, duplicated inference owners, and competing FE/PPML state.


## M12–M21: preparation and numerical-state owners

Categorical preparation now consumes source availability directly, without a
full ordinary prepared-data/topology stage that would be discarded (M12).
Authoritative source roles eliminate slope-only categorical encodings and two
restricted shape/size branches (M13). When the final singleton mask proves an
unchanged row axis, non-FE ordinary/cluster encodings move into their final
owner; changed axes still factorize final rows (M14).

IID, robust and clustered OLS each have one certified owner parameterized by
the original concrete DoF report (M15). The host prepares four dispositions,
ordinary/categorical with/without clusters, then selects typed inference (M16).
One `ActiveFeAbsorber` enum owns exclusive OLS FE storage; genuine route and
probe history remain separate (M17). Fast/Low, explicit solvers, Schur policy,
raw certification and fallback behavior remain supported and unchanged.

PPML installation consumes one ordered admission owner containing the plan,
execution overlap and accepted setup choices (M18). `OrdinaryDirection` couples
LSMR variant identity with its workspace and excludes competing CMG/profile
owners (M19). The canonical checkpoint session and profiled curve remain
independent because they coexist with that direction. One typed ordinary
canonical retry shares certification/accounting/extraction while retaining
classified failure reasons and their mutation order (M20).

`StrictCheckpointControl` owns pending work, tolerance, recovery counters,
accepted passes and final readiness (M21). Borrowed phase methods preserve the
candidate transaction and distinct final-Hessian synchronization; they allocate
no new numerical representation. Its new controller file is 694 production
lines and every method is at most 77 lines. Admission and direction modules
also use bounded methods. The IRLS driver falls from 2,014 lines at M11 to 962.

This series increases core production source by 479 lines and reduces host
source by 21 relative to M11. Core long functions fall from 98 to 97 and host
from 16 to 15; large-file counts stay 19/3. It removes work, duplicate layouts
and invalid owner combinations, but does not substantially shrink the whole
codebase. Source growth includes counted types and phase signatures. Completed
tranche qualification and the separate review are recorded in the
[M21 evidence](../bench/history/2026-10-03-m21-strict-recovery.json), with progress
only in MILESTONES. Maintenance consolidation is not a speed claim.

Two extra numerical witnesses exposed defects in the qualified predecessors:
strict no-FE PPML failed with r(430), including a full-rank constant-outcome
design with condition number 7.885 and an independent closed-form solution;
near-within saved-FE CG failed with r(459). The historical
[M14](../bench/history/2026-10-03-m14-encoding-transfer.json) and
[M17](../bench/history/2026-10-03-m17-active-fe-owner.json) summaries retain those
failures. Passing the original required matrices did not certify these cases.

[M22 PPML](../bench/history/2026-10-03-m22-ppml-correction.json) adds independent
row-residual coefficient refinement in retained finite solves. A cancellation or
underflow guard checks covariance with independently accumulated transformed score
factors, preserving original bits when the checks agree. Independent tests cover
lost-null-direction meat and the original-space score/meat identities.
[M22 saved-FE](../bench/history/2026-10-04-m22-saved-fe-correction.json) repairs
CG polishing and original-model FE-discrepancy stopping without weakening the
original residual certificate. Independent optimum/design tests and public
24/48-configuration witness matrices cover the repairs.


The clean M21 runtime `9c08141b` passes 73 exact parent cases, five-thread
science/repeats/discrete checks, all 46 licensed fixtures, Mac ABI checks and
full Linux CI/native replay 37158517146. All 46 final timing ranges overlap;
290 coefficient/sample command pairs are exact. Whole-series M11 comparisons
have smoke/scaling ratios 1.007/1.006; M20-to-M21 PPML paths have ratio 1.001.
These results establish no reproduced regression in this bounded campaign,
without establishing equivalence, consistent acceleration or broad scaling.

The M21 separate Astra review found all ten implementation scopes fulfilled and
no demonstrable new runtime defect. It confirmed concrete removals in
preparation and OLS owners, while observing that the PPML phases largely make
existing policy easier to inspect. It recommended prioritizing the two numerical
witnesses, subsequently repaired in M22, then using attributable
phase-level evidence to choose performance changes. M17's stale qualification
placeholders were corrected during review. The full report is preserved in
the M21 evidence; no new implementation scope is implied by this assessment.

## Measured PPML work after the numerical repairs

[M24 attribution](../bench/history/2026-10-04-m24-ppml-cost-attribution.json)
compares verified M21, M22.1 and restored M23 stock binaries on the same frontend
and datasets. Finite-solve and strict-final phase increments repeat in the auto
order reversal, while whole-command and current-versus-M22.1 differences remain
order-sensitive. Phase names cover broader intervals than individual kernels:
ordinary joint finite time includes certification; strict time includes candidate
assessment and final Hessian work. They cannot isolate all repair cost.

Stock sampling locates `ols/retained_refinement.rs` in fresh and strict canonical
solves; explicit MAP also reaches it in ordinary frozen solves. Its residual and
score loops each divide the same row value by the same reference norm, and the
stock ARM64 function contains both divisions. Reusing the normalized value in
existing row scratch is the proposed next local change. The two callers supply
unique canonical active positions; dense, implicit and scratch readers refill the
complete row. M24 changes no runtime. Samples identify an exercised target, not
wall-time shares, a dominant package bottleneck or an established future speedup.

The accepted M27 runtime gives OLS and PPML one finalized `PreparedModel` in
`ols/prepared_categorical_data.rs`: sample, finite recipes, source handles,
validated FE topology, ordered effect metadata and optional cluster metadata.
Compatibility aliases contain no second state. Host preparation owns this
model directly; PPML generations share one `Arc`. Ordinary/categorical DoF and
arithmetic remain distinct non-owning adapters where mathematically required.

Both grammars use the same partition/pruning/finalization sequence. Original
request metadata are borrowed; bound recipes contain source positions only.
Unchanged codes and ordered memberships transfer into the topology. Final
source handles use its sorted final level axis; original factor slots and base
flags are preserved. Cluster-only encoding occurs once after pruning. Source
and model certification share one implementation, while dynamic responses and
explicit artifact validation retain their required checks.

The ordinary final-parts chain, owning clustered wrappers, synthetic
availability graph/request/partition construction, second finite recipe bank,
identity reporting vector, host owning dispatch and three compiled preparation
scaffolds are retired in this integrated runtime. Public syntax, solver
policies and the frozen legacy protocol are unchanged. The [scope and local evidence](../bench/history/2026-10-04-m27-canonical-owner.json)
and [calibrated timing acceptance](../bench/history/2026-10-05-m27-canonical-owner-acceptance.json)
bind the accepted integrated runtime and Astra high completion review.
Qualification is Mac-only under Johannes's override. Prior inconclusive trials,
interrupted calibrations and historical CI failures remain unchanged.

## Shared capacity and storage owners

`memory::Capacity` provides checked element/byte counts and preserves exact
reservation where a concrete owner uses it. `fe::workspace_capacity` owns the
projection, MAP, categorical parallel and two-FE accelerator shapes consumed
by allocation and preflight. Concrete observed vector capacities remain the
allocation authority. `finite_wls_kernel` owns both its concrete moment layout
and the distinct monotone retained-scratch envelope. OLS and PPML use the
scratch owner's named conservative reserves, preserving empty and wide rows.

`PreparedOlsScratch` has one row-tile transfer operation for reads and patches
and one immutable-reader refill operation. Direct reads retain exact spans;
scans retain aligned pages. Patch poisoning starts with mutation, and failed
reads revoke the reader page. Dense, scratch and implicit centered numerical
kernels retain the operations appropriate to their storage; end-to-end finite
orchestration is assigned to M31.

PPML admission carries profile/curve layouts and optional lifetime allowances
into installation. Constructors freshly verify those allowances against their
owned requirements, including the coexisting coefficient/checkpoint frames.
The allocation certificate retains its original observation once, derives
allocator growth, and is recomputed for fresh publication validation. Linear
fingerprints retain the nested FE owner rather than repeating its fields.

Host `action_memory::Composition` assembles each stage once and supplies
separate conservative admission, expected heap and coverage channels. Unknown
expectations remain unknown; gaps accumulate. Old/new preparation generations,
detector/cascade, Mu, strict rollback, recovery, inference and publication retain
their distinct simultaneous lifetimes. These costs remain until their consumers
are retired within the later architectural milestones.

The [M28 acceptance](../bench/history/2026-10-05-m28-capacity-owner-acceptance.json)
binds the unchanged local evidence, actual +96-line change and explicit
architectural exception. Its terminal performance evidence remains unresolved.
