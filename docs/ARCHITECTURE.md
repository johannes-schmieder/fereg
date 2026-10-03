# Incremental refactoring architecture

[SPEC](SPEC.md) governs scientific behavior; [PLAN](cleanup/PLAN.md) defines
M0–M1. [MILESTONES](../MILESTONES.md) distinguishes planned from completed work.
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

The [versioned inventory](cleanup/complexity.md) measures 104,913 production
lines in core and 15,248 in the host. Core has 98 functions over 150 lines and
20 files over 1,500; the host has 16 and 3. Its conservative dispatch inventory
contains 1,476 core and 165 host candidates, including validation conditions;
13 core chunk conditions are counted separately. These counts measure
complexity; some conditions validate inputs rather than select algorithms.

Largest inherited files are `fe/borrowed_map.rs` (10,198 production lines),
`ols/prepared_fit.rs` (7,398) and `ols/linear_session.rs` (6,213). The largest
functions are PPML IRLS orchestration (2,014 physical lines), PPML route-memory
estimation (1,208), and retained-rank solving (925). Six OLS host branches
(representation × VCE), fast/low twins and repeated FE entry points remain
candidate simplifications, to be chosen with coverage evidence after M1.

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

A concrete next candidate is the duplicated FE-collinearity/candidate
classification inside `fit_scaled_pivoted_qr` (497 lines) and
`fit_scaled_pivoted_qr_low` (486 lines) in `ols/prepared_fit.rs`. Both functions
are exercised on Mac and Linux, and M1's frequency-singleton repair had to change
both copies. Scope the follow-up to one shared classification helper, preserving
mode-specific validation/error order, allocation admission, storage, solvers and
routing. Require fewer duplicated production lines, exact parent-next results
and route signatures, the fast/low singleton regressions, and no timing
regression beyond observed noise. This is a proposal; M0–M1 does not authorize
its implementation or merging the two entire QR pipelines.
