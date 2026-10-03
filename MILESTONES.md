# Incremental refactoring — milestones

Plan: [M0–M8](docs/cleanup/PLAN.md). Previous rebuild history: Git revision 15a57ba285a3afede2e3337fe8598e72529a4090.
Comparator source: 15a57ba285a3afede2e3337fe8598e72529a4090; tag pinned; Mac/Linux/Windows canonical assets qualified and stored.
Current task: M8.1 Schur preparation boundary and parent baseline; M9–M11 discussion follows qualification.
Current runtime: next defaults for OLS/PPML; separate frozen legacy selectable.

## Tasks and prerequisites

- [x] M0.1 Governance, ADR dispositions, resolved covariance decision and new ledger.
- [x] M0.2 Frozen/tagged baseline; all three canonical platforms built/stored; Mac 34 Stata fixtures pass.
- [x] M0.2a Immutable tag, build/fetch/registration tooling and platform factory.
- [x] M0.2b Hosted Linux/Windows assets, canonical registration and verified storage.
- [x] M0.3 Working core/testkit moved; host retained; abandoned rewrite removed; full local harness/native ABI and long oracles pass.
- [x] M0.4 Mac/Linux replay passes 73 exact/repeat cases: 70 scientific passes, 3 explicit defects; bounded matrix fixed.
- [x] M0.5 Ratchet, Mac/Linux coverage and full Linux checks pass; corrected artifact verifies 221 native profiles and both estimator entry points.
- [x] M1.1 Two-plugin selection/default, diagnostics and exact public baseline pass.
- [x] M1.2a Next-only factor precedence correction; excluded-row, TS and independent dummy checks pass.
- [x] M1.2b Full-matrix covariance repair; independent PSD and exact unaffected replay pass.
- [x] M1.2c Slope-DoF validation repaired; independent rank, malformed-report rejection and exact unaffected replay pass.
- [x] M1.2d Frequency-retained singleton fix; dense fit and public fast/low pruning/retention checks pass.
- [x] M1.3 Bounded qualification: 73 science cases at 1/2/4/8/16 threads, exact fixed-thread repeats and 46 licensed fixtures pass.
- [x] M1.4 Six-plugin review bundle; licensed Mac installation, Linux/Windows builds/audits/ABI and final Linux source/replay CI pass.
- [x] M1.5 Identity-bound smoke/scaling, Mac/Linux coverage and refreshed complexity; proposed next cleanup remains unimplemented.

- [x] M2.1 Boundary regression passes on parent; 34 fresh timing groups recorded with qualified parent Mac binary.
- [x] M2.2 Shared classification; full/long tests, exact parent replay, five-thread science/repeats, native ABI and 3 Stata fixtures pass.
- [x] M2.3 All 34 timing ranges overlap; 214 coefficient/sample pairs exact; complexity lowered; M3 proposal documented for discussion.

- [x] M3 Shared OLS completion; full local checks, 73 exact native replay and 4 licensed output/transaction fixtures pass.
- [x] M4 Shared QR pivot/solve/status kernels; full/long checks, 73 exact/scientific native replay and 2 fast/low Stata fixtures pass.
- [x] M5 Shared robust/clustered Wald solve and IID coefficient lookup; full/long checks, 73 exact/scientific replay and 2 Stata fixtures pass.
- [x] M6 Shared deferred categorical DoF and checked totals; full/long checks, 73 exact/scientific replay and 3 Stata fixtures pass.
- [x] M7 Scaling diagnosis and combined qualification; shared Schur preparation proposed for discussion, not implemented.

- [ ] M8.1 Preparation-order audit and qualified parent timing baseline.
- [ ] M8.2 Shared Schur RHS/projection kernels; scientific, transaction and complexity qualification.
- [ ] M8.3 Paired timings, phase comparison, final CI and detailed M9–M11 proposals.

## Gates

- [x] G-M0: canonical comparators, copied implementation, real OLS/PPML replay on Linux CI, exact-copy pass, explicit matrix/exceptions and baselines.
- [x] G-M1: defaults/selection, corrections, exact unaffected outputs, mapped qualification, installation and summaries.
- [x] G-M2: shared classification, exact parent-next checks, independent science and performance/complexity evidence; CI 37118979350 passes.
- [x] G-M3–M7: local/long checks, exact parent replay, five-thread science, 46 Stata and Linux CI 37121923471 pass; timings investigated.
- [ ] G-LICENSE: existing native-distribution gate; draft development assets do not establish release permission.

## Decisions

- Full-matrix OLS multiway PSD repair including the reported constant is approved; it is not an open question.
- Scientific agreement across thread counts; retain fixed-thread deterministic repeatability and exact-change checks.
- Bundle separate plugins; legacy/source policies are historical except retained SPEC/active ADR requirements.

## Blockers

No remaining M0–M7 blockers. The intermittent dictionary audit was isolated and verified in CI 37093239587.

Historical v39/v24 allocation manifests predate the frozen source; do not refresh them as proof.
Old source-string assertions for fit_absorb_matrix and shadow.retained_rows also fail against frozen code; runtime checks remain required.

## Log

- 2026-10-02 M0.1 6643939b — governance reset; fmt/Clippy, 1906 workspace, 173 doc/FFI, 9 isolated working audits, size/mock/static pass; rewrite-only audits retired.
- 2026-10-02 M0.2a 9323fe00 — tag 15a57ba2; Mac 80329f51 ABI/native audit and 34/34 Stata pass; 5 identity tests pass; hosted platforms pending.
- 2026-10-02 M0.2b this commit — build setup repair 1: scoped Linux checkout trust and LF archive override; frozen source hash check retained.
- 2026-10-02 M0.3 599793c3 — 176 retained source files byte-checked; 1635 test invocations, long oracle suite, Mac ABI and 13 bench tests pass.
- 2026-10-02 M0.2b this commit — Linux ABI/audit pass; Windows ABI/audit pass, final checksum failed under old PowerShell; factory uses pwsh.
- 2026-10-02 M0.2b this commit — canonical Windows 1c8ac304 verified; hosted build/ABI/audit and draft storage pass; all platforms registered.
- 2026-10-02 M0.4 this commit — Mac 73 exact/repeat cases, 70 dense passes, 3 known defects; full local harness/ABI and 4 replay acceptance tests pass.
- 2026-10-02 M0.4 this commit — CI download repair: isolate draft-read permission in a job without repository code; replay keeps read-only access.
- 2026-10-02 M0.4 this commit — use qualified glibc-2.28 build image; remove obsolete diagnostics-only fake estimator; real ABI/bridge checks pass locally.
- 2026-10-02 M0.5 this commit — ratchet: core/host 105136/15248 lines, 99/16 long functions, 20/3 large files; four guard tests and size/governance checks pass.
- 2026-10-02 M0.4 this commit — Linux CI 37085394220 replay passes 73 exact/repeat cases, 70 science and 3 defect reproductions; compact summary in bench/history.
- 2026-10-02 M0.5 this commit — bounded oracle/ABI/73-replay coverage passes; 2814/807/20 reached/unreached/unmapped; broad stress instrumentation stopped as out of scope.
- 2026-10-02 M0.5 this commit — CI 37085394220 succeeds: full Rust/FFI/allocation/ratchet/mock checks and real Linux replay; profiling bundle links pinned Rust runtime.
- 2026-10-02 M0.5 this commit — CI coverage 37086231092 succeeds and retains export/HTML; G-M0 passed; no numerical changes yet.
- 2026-10-02 M0.5 this commit — reopened gate: Linux artifact had 0 reached host functions despite passing replay; ELF runtime anchor/per-process emission check added.
- 2026-10-02 M0.5 this commit — corrected CI 37086868628: 221 native profiles, both entry points, 2808/811/22 reached/unreached/unmapped; G-M0 verified.
- 2026-10-02 M1.1 this commit — selection/interleaving/restore and 2 contract fixtures pass; 73 exact replays, full local harness and 24 frontend checks pass.
- 2026-10-02 M1.2a this commit — 9 licensed fixtures, 73 exact parent replays, 29 long oracles and full harness pass; core -3 lines; legacy failures unchanged.
- 2026-10-02 M1.2a 744ef7f4 — CI 37088854188 all jobs pass; earlier selector CI had one 56-byte global-allocation audit discrepancy; audit unchanged and passes now.
- 2026-10-02 M1.2b this commit — 71 science/2 defect replays; 72 unaffected cases exact; 13 cluster oracles, 3 Stata, full/long harness pass; core -231 lines, -1 long fn.
- 2026-10-02 M1.2b dbc8505a — CI 37089724723 passes all jobs, including real Linux replay.
- 2026-10-02 M1.2c this commit — 72 science/1 defect; 72 unaffected cases exact; 3 Stata, full/long harness pass; core +5 lines; precommit ratchet base corrected.
- 2026-10-02 M1.2d this commit — 73 science; 72 unaffected cases exact; 3 Stata, full/long checks pass; +6 core lines; replay constant allocation corrected/retested.
- 2026-10-02 M1.3 this commit — 803 native processes; 73 science cases at five thread counts, discrete/repeat checks, frozen comparison and 46 Stata pass.
- 2026-10-02 M1.4 this commit — paired plugin packaging/help and canonical hash guard; 4 isolated Stata fixtures and 19 package tests pass; hosted platforms pending.
- 2026-10-02 M1.5 this commit — existing runner binds engine/binary/build/frontend identity; bounded Mac ladder and noise summaries; 14 benchmark guards pass.
- 2026-10-02 M1.5 this commit — smoke pilot passes; generated native manifest now ignored so clean-source timing identity is accurate; formal runs follow.
- 2026-10-02 M1.4 this commit — allocator audit isolation repair; concurrent 56-byte allocation regression and full harness pass; production tokens/counts unchanged.
- 2026-10-02 M1.5 this commit — 34 timing pairs overlap; 214 coefficient/sample pairs exact; Mac/Linux 2813/2808 reached functions; core -223 lines versus M0.
- 2026-10-02 M1.4 5ba506b1 — CI 37093239587: full checks and 73 science pass; all platform builds, 4 staged fixtures and 38-file install/get verification pass.
- 2026-10-02 G-M1 this commit — all gates verified; see [platforms](bench/history/2026-10-02-m1-platform-qualification.json) and [timings](bench/history/2026-10-02-m1-performance.json).
- 2026-10-03 M2.1 this commit — approved scope recorded; parent Mac 49a597c4 preserved; fast/low validation differences identified before extraction.
- 2026-10-03 M2.1 35d1720e — fresh parent smoke/scaling: 34 groups, 214 commands; norm/error/deferred-intercept regression passes before extraction.
- 2026-10-03 M2.2 this commit — one classifier; core -3 lines, QR functions 497/486 to 455/448; full/30 long oracles, 73 exact/five-thread science, 3 Stata pass.
- 2026-10-03 M2.3 7b1bae2b — clean Mac ABI/73 replay pass; 34 timing ranges overlap, 214 pairs exact; CI 37118979350 full/Linux replay pass.
- 2026-10-03 G-M2 this commit — checks and compact [evidence](bench/history/2026-10-03-m2-classification.json) complete; M3 completion-path proposal remains unimplemented.
- 2026-10-03 M3.0 this commit — next five milestones scoped; M2 remains the qualified comparison; no solver-policy or legacy changes authorized.
- 2026-10-03 M3.1 this commit — six completion sequences to one; host -5 lines; full checks, 73 exact/scientific replay, Mac ABI and 4 Stata pass.
- 2026-10-03 M4.1 this commit — shared QR pivot/solve/status; core -3 lines, dispatch inventory -2; analytic regression, full/long tests, 73 replay and 2 Stata pass.
- 2026-10-03 M5.1 this commit — common Wald solve/lookup; core -118 lines, large files 20 to 19; full/long tests, 73 exact/scientific replay and 2 Stata pass.
- 2026-10-03 M6.1 this commit — common deferred DoF/totals; core -32 lines; identity, slope and nesting checks retained; full/long tests, 73 replay and 3 Stata pass.
- 2026-10-03 M7.1 b3fa58a7 — clean Mac 73/five-thread replay, 46 Stata and CI 37121923471 pass; 158 production lines removed; legacy unchanged.
- 2026-10-03 G-M3–M7 this commit — timings/phase diagnosis and [evidence](bench/history/2026-10-03-m3-m7-refactoring.json); PPML follow-up ranges overlap; M8 discussion only.

- 2026-10-03 M8.0 this commit — scope authorized; share Schur level sums/projection and probe preparation; preserve validation order and routes.
