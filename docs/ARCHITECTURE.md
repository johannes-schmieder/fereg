# Incremental refactoring architecture

[SPEC](SPEC.md) governs scientific behavior; [PLAN](cleanup/PLAN.md) defines
M0–M11. [MILESTONES](../MILESTONES.md) distinguishes planned from completed work.
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

The [versioned inventory](cleanup/complexity.md) measures 104,708 production
lines in core and 15,243 in the host. Core has 98 functions over 150 lines and
19 files over 1,500; the host has 16 and 3. Its conservative dispatch inventory
contains 1,477 core and 165 host candidates, including validation conditions;
13 core chunk conditions are counted separately. These counts measure
complexity; some conditions validate inputs rather than select algorithms.

Largest inherited files are `fe/borrowed_map.rs` (10,162 production lines),
`ols/prepared_fit.rs` (7,123) and `ols/linear_session.rs` (6,213). The largest
functions are PPML IRLS orchestration (2,014 physical lines), PPML route-memory
estimation (1,154), and retained-rank solving (925). Six OLS host branches
(representation × VCE) now share completion, but preparation/inference branches,
fast/low twins and repeated FE entry points remain candidate simplifications.

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
