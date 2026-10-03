# Incremental refactoring — milestones

Plan: [M0–M1](docs/cleanup/PLAN.md). Previous rebuild history: Git revision 15a57ba285a3afede2e3337fe8598e72529a4090.
Comparator source: 15a57ba285a3afede2e3337fe8598e72529a4090; tag pinned; Mac/Linux/Windows canonical assets qualified and stored.
Current task: M1.4 bundled installation and platform qualification; M1.3 local bounded checks pass.
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
- [ ] M1.4 Bundled installation and platform qualification. Selection/packaging work depends on M1.1; final qualification on M1.3.
- [ ] M1.5 Performance/coverage/complexity summaries. Depends on M1.3 and qualified platform artifacts.

## Gates

- [x] G-M0: canonical comparators, copied implementation, real OLS/PPML replay on Linux CI, exact-copy pass, explicit matrix/exceptions and baselines.
- [ ] G-M1: defaults/selection, corrections, exact unaffected outputs, mapped qualification, installation and summaries.
- [ ] G-LICENSE: existing native-distribution gate; draft development assets do not establish release permission.

## Decisions

- Full-matrix OLS multiway PSD repair including the reported constant is approved; it is not an open question.
- Scientific agreement across thread counts; retain fixed-thread deterministic repeatability and exact-change checks.
- Bundle separate plugins; legacy/source policies are historical except retained SPEC/active ADR requirements.

## Blockers


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
