# fereg clean-core rebuild — milestones

Plan: [docs/cleanup/PLAN.md](docs/cleanup/PLAN.md), copied from [issue #28](https://github.com/johannes-schmieder/fereg/issues/28).
Legacy snapshot: `legacy-v0.5.0-beta` at `e61cb9ee0ed8f575bd972cf45fea08af27826bc8`.
Current task: M0.2.
Default engine: fereg = legacy, feppml = legacy.
Legend: [ ] todo · [x] done · [~] implemented, verification pending.

## Tasks
- [x] M0.1 Tag, plan, milestones
- [ ] M0.2 New `AGENTS.md`
- [ ] M0.3 Stage rebuild ADRs
- [ ] M0.4 Transfer the resume record
- [ ] M0.5 Inventory and prepare the lean harness; preserve scientific coverage
- [ ] M0.6 CI
- [ ] M0.7 Make the legacy suite pass
- [ ] M0.8 Minimal legacy safety fixes
- [ ] M1.1 `docs/SPEC.md`
- [ ] M1.2 `docs/ARCHITECTURE.md`
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

- None requiring input. Stata/SPI licensing disposition remains a planned
  prerequisite to native-plugin distribution, not a completed gate.

## Log (newest first; one line per task)

- 2026-09-29 M0.1 (this task commit) — issue copy verified verbatim; all 52 tasks/five gates listed; annotated origin tag verified at e61cb9ee.
