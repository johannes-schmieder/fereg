# fereg clean-core rebuild — milestones

Plan: [docs/cleanup/PLAN.md](docs/cleanup/PLAN.md), copied from [issue #28](https://github.com/johannes-schmieder/fereg/issues/28).
Legacy snapshot: `legacy-v0.5.0-beta` at `e61cb9ee0ed8f575bd972cf45fea08af27826bc8`.
Current task: M1.3; independent dense oracles and design generators.
Default engine: fereg = legacy, feppml = legacy.
Legend: [ ] todo · [x] done · [~] implemented, verification pending.

## Tasks
- [x] M0.1 Tag, plan, milestones
- [x] M0.2 New `AGENTS.md` (verbatim; local harness passes with pending M0.7–M0.8 repairs)
- [x] M0.3 Stage rebuild ADRs (structural and local lean checks pass)
- [x] M0.4 Transfer the resume record
- [x] M0.5 Inventory and prepare the lean harness; preserve scientific coverage
- [~] M0.6 CI (push workflow installed; GitHub billing blocks runner startup)
- [~] M0.7 Local suite passes; hosted CI confirmation awaits GitHub billing
- [x] M0.8 Minimal legacy safety fixes
- [x] M1.1 `docs/SPEC.md`; source reconciliation and provenance consolidated
- [x] M1.2 `docs/ARCHITECTURE.md`; legacy source and readers preserved
- [ ] M1.3 `crates/fereg-oracle`
- [ ] M1.4 `bench/` suite
- [ ] M1.5 SCC tooling
- [ ] M1.6 Legacy baseline
- [ ] M2.1 Move legacy
- [ ] M2.2 Create `crates/fereg-engine`
- [ ] M2.3 Harvest `data`
- [ ] M2.4 Harvest `topology`
- [ ] M3.1 `fe::Design` and `fe::Plan`
- [ ] M3.2 Direct projection for Q = 1
- [ ] M3.3 Schur-PCG for Q ≥ 2
- [ ] M3.4 Certificate and coefficients
- [ ] M3.5 Retained backends and convergence monitor
- [ ] M3.6 Property tests and micro-benchmarks
- [ ] M4.1 `wls`
- [ ] M4.2 `vce` and fit statistics
- [ ] M4.3 Outputs
- [ ] M4.4 `ols::fit(&ModelSpec, &Loaded, &Options) -> Result<OlsFit>`
- [ ] M4.5 Oracle and differential tests
- [ ] M5.1 Engine switch
- [ ] M5.2 Stata fixtures with `engine(next)`
- [ ] M5.3 Benchmarks
- [ ] M5.4 G-OLS, then flip the `fereg` default to `engine(next)`
- [ ] M6.1 `ppml::family` and IRLS
- [ ] M6.2 Separation
- [ ] M6.3 PPML inference and outputs
- [ ] M6.4 Bias correction
- [ ] M6.5 Tests and seam
- [ ] M6.6 Benchmarks and cutover
- [ ] M7.1 Remove legacy after coverage acceptance
- [ ] M7.2 Clean up the plugin
- [ ] M7.3 Dependencies and licensing
- [ ] M7.4 Ado cleanup
- [ ] M7.5 Final sweep after §0.9
- [ ] M8.1 Input path
- [ ] M8.2 Host threads
- [ ] M8.3 Faster FE operator (same math)
- [ ] M8.4 Thread scaling
- [ ] M8.5 Targets
- [ ] M9.1 User docs
- [ ] M9.2 Final sync
- [ ] M9.3 Release

## Gates

- [ ] G-M3 engine go/no-go: all supported backends and the automatic policy.
- [ ] G-OLS oracle, parity, differential, performance and timeout checks.
- [ ] G-PPML corresponding PPML checks, including mean bias correction.
- [ ] G-FINAL correctness, determinism, size and performance report.
- [ ] G-LICENSE native-plugin distribution.

## Blockers / questions for Johannes

- GitHub Actions run [36546517277](https://github.com/johannes-schmieder/fereg/actions/runs/36546517277)
  did not start: failed account payment or spending-limit restriction. Johannes
  was asked to resolve billing; no repository fix or retry can supply this check.
  Local Rust/FFI checks pass; committed-source Stata quick passes all 32 fixtures
  at b5a95266 (application rc=0 and sentinels). M1 documentation work is independent.
  Local diagnostic logs: `target/rebuild-m0/`.
- Stata/SPI licensing disposition remains a planned
  prerequisite to native-plugin distribution, not a completed gate.

## Log (newest first; one line per task)

- 2026-09-29 M1.2 (this task commit) — 286-line architecture, 24,500-line engine budgets, shared FE/seam/ownership rules; case-colliding legacy doc preserved with readers/links updated; four source-contract checks and local lean harness pass.
- 2026-09-29 M1.1 d176dcd9 — twenty SPEC sections with executable-test mappings; all named source contracts/ADRs reconciled; LSMR/CMG and both PPML bias increments retained; notices folded; link/option/section audit and local lean checks pass.
- 2026-09-29 M0.8 b5a95266 — b41f18e/tag core identical; Low raw residual used CG polish (max error 7.733e-4); full CMG projection passes Auto/CMG/LSMR dense oracles; public CG rejection and rollback pass; committed b5a95266 passes ./ci/run_stata_ci.sh quick: 32/32, rc=0, sentinels.
- 2026-09-29 M0.7 30725d78 — 1429 workspace tests, 164 serial FFI tests and nine isolated audits pass; three existing core tests remain ignored; native CMG adapter repair required for green baseline included; byte growth traced to d6c18ccb/d9e9d2f7.
- 2026-09-29 M0.6 516d0b39 — local lean checks pass; hosted run 36546517277 blocked before any step by GitHub billing; workflow/profile/guide pushed; remote Linux confirmation pending.
- 2026-09-29 M0.5 88ac3129 — generator/oracle identities verified; legacy harness retained; ci profile included for harness dependency; lean checks, universal build and 32 Mac quick fixtures pass.
- 2026-09-29 M0.4 db69cce4 — links and frozen STATUS/ADRs verified; requirement mapping retained; local lean checks and 32 Stata quick fixtures pass.
- 2026-09-29 M0.3 4f183240 — eight one-page staged ADRs plus README/TEMPLATE; original ADRs unchanged; GPL disposition explicitly unresolved; local lean checks pass.
- 2026-09-29 M0.2 33c10a3e — AGENTS matches plan §4.1 verbatim; lean checks pass in the working tree with pending M0.7–M0.8 repairs; legacy layout guards remain callable.
- 2026-09-29 M0.1 afbcfd2d — issue copy verified verbatim; all 52 tasks/five gates listed; annotated origin tag verified at e61cb9ee; pushed.
