# fereg clean-core rebuild — milestones

Plan: [docs/cleanup/PLAN.md](docs/cleanup/PLAN.md), copied from [issue #28](https://github.com/johannes-schmieder/fereg/issues/28).
Legacy snapshot: `legacy-v0.5.0-beta` at `e61cb9ee0ed8f575bd972cf45fea08af27826bc8`.
Current task: M4.4 OLS orchestration on the shared engine; M3.5 dependency determinism decision pending; M1.6 SCC full array running (7779872), Mac standard and auto/LSMR/CMG baselines captured.
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
- [x] M1.3 `crates/fereg-oracle`; dense oracles, seeded designs and legacy comparisons
- [x] M1.4 `bench/` suite; 22 cells, pinned comparators and Mac smoke
- [x] M1.5 SCC tooling; compute-node smoke passed and summary collected
- [~] M1.6 Legacy baseline; Mac cohorts captured, SCC full campaign running
- [x] M2.1 Move legacy; Rust checks, path checks and universal plugin/ABI build pass
- [x] M2.2 Create `crates/fereg-engine`; safe skeleton, wire parser and deterministic team
- [x] M2.3 Shared data/factor harvest; exact legacy sample/weight parity on 2,400 designs
- [x] M2.4 Shared FE topology, singleton fixed point, components and DoF; exact legacy parity
- [x] M3.1 Shared FE design, structural solver plan and coefficient/residual diagnostics
- [x] M3.2 Direct projection for Q = 1
- [x] M3.3 Schur-PCG for Q ≥ 2
- [x] M3.4 Certificate and coefficients
- [ ] M3.5 Retained backends and convergence monitor
- [ ] M3.6 Property tests and micro-benchmarks
- [x] M4.1 `wls`
- [x] M4.2 `vce` and fit statistics
- [x] M4.3 Outputs
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

- M3.5: pinned `within` 1ac3522 violates SPEC §18 / architecture's same-arithmetic
  thread contract: a 20,000-row weighted three-FE probe changes 407/419
  Schwarz-LSMR coefficients at T8 versus T1 (max 9.16e-16), and
  18,563/20,000 fitted sums (max 1.44e-15); diagonal
  preconditioning changes 419/419 (max 7.27e-14). All solves report convergence.
  Explicit ParallelReduction does not fix this; current upstream 0319e7a also
  documents width-dependent reductions. Nested/single-thread secondary pools
  and vendored patches conflict with accepted architecture/dependency rules.
  Johannes is asked to authorize an upstream deterministic-reduction fix and
  a new qualified pin, or explicitly amend the reproducibility contract.
  Shared direct/Schur/certificate work is complete; M3.5 acceptance is paused
  pending this decision, with independent milestones continuing.

- SCC inspection on 2026-09-29 reports 6,963.31 GB used against the 6,950 GB
  `/projectnb/welfgr` quota; Johannes was asked about freeing space. The live
  filesystem reports 92,530,540,544 bytes available at the full-baseline
  preflight, sufficient for its estimated <50 GB including build and raw
  outputs. Existing runs and other active jobs have not been modified.
- GitHub Actions run [36546517277](https://github.com/johannes-schmieder/fereg/actions/runs/36546517277)
  did not start: failed account payment or spending-limit restriction. Johannes
  was asked to resolve billing; no repository fix or retry can supply this check.
  Local Rust/FFI checks pass; committed-source Stata quick passes all 32 fixtures
  at b5a95266 (application rc=0 and sentinels). Independent rebuild work continues.
  Local diagnostic logs: `target/rebuild-m0/`.
- Stata/SPI licensing disposition remains a planned
  prerequisite to native-plugin distribution, not a completed gate.

## Log (newest first; one line per task)

- 2026-09-29 M1.6 Mac capture (this commit) — frozen-source [standard](bench/history/2026-09-29-3a931afe-darwin-standard.csv): 152 groups, 150 completed groups, 148 accurate; all 38 fereg groups pass, weak-graph fixest disagrees and fixest-matched fails convergence at T1/T8; wall 10,765s including launcher pauses exceeds the 3,600s budget, with raw failures retained and no retries. Separate [auto](bench/history/2026-09-29-cdfa8f85-darwin-standard-solvers-auto.csv), [LSMR](bench/history/2026-09-29-cdfa8f85-darwin-standard-solvers-lsmr.csv) and [CMG](bench/history/2026-09-29-cdfa8f85-darwin-standard-solvers-cmg.csv) cohorts each pass 6/6 groups and 30/30 measured fits; plugin hash be208844176c58a85485b79d9c97c421fc378058dbb7f4b9117fddf30f4ad5d5; summary source/schema/coverage checks and full lean suite pass; SCC 51/56 application markers, full collection pending.

- 2026-09-29 M4.4 preparation 8a640981 — authoritative finite-column compiler retains omitted/base/vanished slots and literal factor/interaction levels without another code bank; pending TS and unsupported grammar fail before reads; the pinned hidden no-absorb structural constant uses the shared one-level FE engine and identical implicit DoF metadata; 119 engine tests, focused Clippy, full lean and long oracle suites pass; estimator orchestration remains open.

- 2026-09-29 M4.3 cb932c6f — requested-only original-axis residual/xb/xbd/d/drop-reason/saved-channel banks from returned coefficients, no output re-solve; original-equation certification plus checked reported-constant transfer preserves scales when y equals the constant; disconnected weighted/sloped gauges, missing scatter, request invariance, malformed/stale-frame rejection and 1/2/4/8-worker identity pass; all 4,400 direct/Schur oracle designs now check saved-output sums; 115 engine tests and full lean/long suites pass; Mac postprocessing paused 14:04:49–14:08:14 UTC (205s), timed fits already finished; SCC 48/56 application markers.

- 2026-09-29 M4.2 e96bb84f — shared IID/HC1/multiway sandwich with reverse influence replay, harvested certified CGM PSD policy, distinct OLS/PPML DoF/corrections, reference weights, fit statistics and canonical Wald tests; 2,200 dense-oracle inference designs including independent closed-form PSD repairs, literal frequency replication, ten-way tuple coverage, rescued subnormal weights and exact 1/2/4/8-worker identity pass; full lean/long suites pass; Mac launcher pause 13:46:53–13:57:24 UTC (631s), no active fit; SCC 45/56 application markers, two comparator failures retained.

- 2026-09-29 M4.1 25f7cf7b — one finite WLS kernel with fixed-block moments, guarded pivoted Cholesky/shared scaled QR, pre-projection FWL rank scales, full slot/status/bread mapping and independent score/two-sided inverse checks; weak-direction score failure localized and repaired by bounded same-factor residual refinement at unchanged tolerances; 96 engine tests, 2,200 dense-oracle WLS cases, all FE regressions, full lean/long suites pass; exact 1/2/4/8-worker identity; Mac launcher pause 13:27:17–13:37:13 UTC (596s), fit clocks unaffected; SCC 43/56 application markers.

- 2026-09-29 M3.5 qualification 9f6d5122 — pinned within 1ac3522 cross-worker discrepancy reproduced under explicit Schwarz ParallelReduction and diagonal preconditioning; executable oracle diagnostic records coefficient and fitted-sum differences, no runtime tolerance weakened; upstream 0319e7a still documents the limitation; dependency-scope decision requested under AGENTS conflict rule; full lean/long suites and focused diagnostic/Clippy pass; Mac launcher pause 13:12:19–13:16:52 UTC (273s), no active fit during audit; independent M4.1 continues.

- 2026-09-29 M3.4 2d4dcb11 — shared fresh reconstruction/local-span certification and component-weighted intercept normalization; independent reorthogonalized span witness, raw/RMS diagnostics, stale-weight/tampering/weak-direction/unit-rotation tests; 90 engine tests, 4,400 direct/Schur oracle designs and full lean/long suites pass; normalization preserves fitted sums and slopes, deterministic at 1/2/4/8 workers; Mac launcher pause 13:06:17–13:09:56 UTC (219s), fit clocks unaffected; SCC 36/56 application markers, two comparator failures retained.

- 2026-09-29 M3.3 8767cc66 — matrix-free Schur-PCG eliminates the largest effect with shared rank-aware level QR, block-Jacobi solves, independent RHS budgets and immutable-frame warm starts; 84 engine tests, 2,200 dense-dummy graph/weight/slope comparisons (4,400 nonzero RHSs), bit identity at 1/2/4/8 workers, full lean and long oracle suites pass; recurrence candidates await M3.4 certification/M3.5 monitoring; Mac launcher pause 12:57:17–13:00:41 UTC (204s), fit clocks unaffected.

- 2026-09-29 M3.2 713aa14c — direct weighted per-level projection with shared scaled pivoted QR, canonical deficient-rank representatives and multi-RHS factors; 81 engine tests and 2,200 independent dense-dummy comparisons (6,600 RHSs) pass, exact 1/8-worker identity; full lean and long oracle checks pass; candidates remain explicitly unaccepted until M3.4 shared certification; Mac launcher pause 12:47:36–12:51:46 UTC (250s), fits unaffected; SCC 33/56 completed, strict fixest weak-graph failures at T1/T8 retained.

- 2026-09-29 M3.1 47a1ed0c — shared matrix-free D/D'W, weighted level blocks and canonical channel/level layout; structural auto candidate, explicit LSMR/CMG support and bounded fallback plans, common result diagnostics; 74 engine tests, full lean and long oracle suites pass; explicit-matrix equality and 1/2/4/8-worker bit identity; 546 FE lines; Mac launcher pause 12:34:14–12:37:43 UTC (209s), fit clocks unaffected.

- 2026-09-29 M2.4 acad66ae — one owned topology with canonical deduplicated partitions, certified CSR membership, borrowed slope channels, frequency-aware singleton cascades and reference DoF/nesting; 66 engine tests, 2,200 topology/sample/component comparisons, 35,200 intercept and 512 slope DoF comparisons pass; full lean and long oracle suites pass; topology 834 lines, DoF 324; Mac launcher pause 12:19:01–12:25:36 UTC (395s), fits unaffected.

- 2026-09-29 M2.3 a077c374 — shared owned inputs, sample ledger, in-place compaction, model columns and canonical factor/tuple coding; 52 engine tests, 2,400 exact sample/weight comparisons, 100 factor comparisons, full lean and long oracle checks pass; data/factor total 1,397 lines including embedded tests; Mac launcher pauses 11:42:01–12:00:31 UTC (fit continued) and 12:03:19–12:06:44 UTC (205s for checks); strict fixest weak-graph warmup nonconvergence retained in baseline, no retry.

- 2026-09-29 M2.2 0c3a3763 — independent safe engine, fallible wire metadata, retained solver options, call-owned deterministic team; 18 engine tests, full lean checks and long oracle suite pass; 1/2/4/8-worker bit identity; largest file 1,240 lines, Clippy 150-line function bound and 1,500/1,501-line guard checks pass; Mac launcher pause 11:26:06–11:30:40 UTC (274s), fit clocks unaffected.
- 2026-09-29 M1.5 completion 146c2b4b — setup 7779824/smoke 7779825: failed=0/exit=0, 28 slots, maxvmem 10.841/1.438G; 16/16 groups and 80/80 measured fits accurate, 155.7s, no failures/timeouts; [summary](bench/history/2026-09-29-242f5201-linux-smoke.csv); collector stdin/permissions repair passes focused retest, 13 harness tests and full lean checks; Mac launcher pause 11:11:47–11:16:08 UTC (261s), fit clocks unaffected.
- 2026-09-29 M1.6 full submitted cdfa8f85 — setup 7779871, array 7779872 (56 tasks); owner-only runs/20260929-rebuild-m16-cdfa8f85; verified existing Veneto storage authorization and 92.5 GB free; raw data stay on SCC.
- 2026-09-29 M2.1 b1700e6d — moved 155 legacy files; all numerical source/test blobs unchanged, one testkit Cargo path adjusted; workspace/plugin/oracle/fuzz and live path readers updated; historical pins preserved; 36 path/harness tests, full lean checks and universal plugin/ABI tests pass; Mac launcher pause 11:03:33–11:09:23 UTC (350s), fit clocks unaffected.
- 2026-09-29 M1.6 preparation cdfa8f85 — declared separate auto/LSMR/CMG cohort; pinned original Veneto N=3,159,720/K=4 and lossless DTA conversion; 12 benchmark tests, full local lean harness and frozen universal build/ABI tests pass; Mac launcher paused 10:52:56–11:01:32 UTC (516s) for checks/build, fit clocks unaffected.
- 2026-09-29 M1.5 repair 242f5201 — build from source/; controllers extracted from the verified commit archive; owner-only run roots; shell checks, 12 benchmark tests and full local lean harness pass; focused retest setup 7779824/smoke 7779825 queued in dev/20260929-rebuild-m15-242f5201.
- 2026-09-29 M1.5 preparation 3a931afe — local checks pass; archive hashes verified; setup 7779802 exited 101 before compilation because Cargo ran above source/; dependent 7779803 exited 1 at its setup guard without estimation; failed logs preserved in dev/20260929-rebuild-m15-3a931afe.
- 2026-09-29 M1.4 d47a91b5 — 22 cells/three tiers; verified fixest 0.14.2 and Stata pins; Mac smoke 16/16 groups, 80/80 measured fits accurate, zero failures/timeouts, 48.0s <300s; [dirty-source diagnostic summary](bench/history/2026-09-29-60da973a-dirty-darwin-smoke.csv); 27 additional option comparisons, five harness tests, universal build/self-test and lean harness pass.
- 2026-09-29 M1.3 60da973a — 27 regular oracle tests, 2,000-design long differential suite and ignored CG-defect diagnostic pass; six 100-digit fixtures, direct CMG/LSMR, weighted inference and Poisson self-checks pass; files <1,500 lines, Clippy 150-line function limit brought forward from M2; local lean harness passes.
- 2026-09-29 M1.2 1bb169fc — 286-line architecture, 24,500-line engine budgets, shared FE/seam/ownership rules; case-colliding legacy doc preserved with readers/links updated; four source-contract checks and local lean harness pass.
- 2026-09-29 M1.1 d176dcd9 — twenty SPEC sections with executable-test mappings; all named source contracts/ADRs reconciled; LSMR/CMG and both PPML bias increments retained; notices folded; link/option/section audit and local lean checks pass.
- 2026-09-29 M0.8 b5a95266 — b41f18e/tag core identical; Low raw residual used CG polish (max error 7.733e-4); full CMG projection passes Auto/CMG/LSMR dense oracles; public CG rejection and rollback pass; committed b5a95266 passes ./ci/run_stata_ci.sh quick: 32/32, rc=0, sentinels.
- 2026-09-29 M0.7 30725d78 — 1429 workspace tests, 164 serial FFI tests and nine isolated audits pass; three existing core tests remain ignored; native CMG adapter repair required for green baseline included; byte growth traced to d6c18ccb/d9e9d2f7.
- 2026-09-29 M0.6 516d0b39 — local lean checks pass; hosted run 36546517277 blocked before any step by GitHub billing; workflow/profile/guide pushed; remote Linux confirmation pending.
- 2026-09-29 M0.5 88ac3129 — generator/oracle identities verified; legacy harness retained; ci profile included for harness dependency; lean checks, universal build and 32 Mac quick fixtures pass.
- 2026-09-29 M0.4 db69cce4 — links and frozen STATUS/ADRs verified; requirement mapping retained; local lean checks and 32 Stata quick fixtures pass.
- 2026-09-29 M0.3 4f183240 — eight one-page staged ADRs plus README/TEMPLATE; original ADRs unchanged; GPL disposition explicitly unresolved; local lean checks pass.
- 2026-09-29 M0.2 33c10a3e — AGENTS matches plan §4.1 verbatim; lean checks pass in the working tree with pending M0.7–M0.8 repairs; legacy layout guards remain callable.
- 2026-09-29 M0.1 afbcfd2d — issue copy verified verbatim; all 52 tasks/five gates listed; annotated origin tag verified at e61cb9ee; pushed.
