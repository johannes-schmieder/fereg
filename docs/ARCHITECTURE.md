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

M0.3 moves the working core/testkit from legacy into the active crates, keeping
its numerical code unchanged. Existing duplicated representations, memory
modes and routes remain until individually scoped refactors remove them.

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

The inherited hotspots are six OLS host branches (representation × VCE),
fast/low memory twins, numerous FE entry points, PPML IRLS orchestration and
N/thread/weight/memory routing. M0.5 supplies the measured complexity and
coverage baselines before choosing a simplification. This map deliberately
does not redesign or catalog every legacy route.
