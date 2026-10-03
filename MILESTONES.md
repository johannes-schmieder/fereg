# Incremental refactoring — milestones

Plan: [M0–M1](docs/cleanup/PLAN.md). Previous rebuild history: Git revision 15a57ba285a3afede2e3337fe8598e72529a4090.
Comparator source: 15a57ba285a3afede2e3337fe8598e72529a4090; tag pinned; Mac/Linux/Windows canonical assets qualified and stored.
Current task: M0.5 complexity/coverage baseline; M0.4 Linux replay gate is still pending.
Current runtime: working legacy implementation; next selection/default lands in M1.1.

## Tasks and prerequisites

- [x] M0.1 Governance, ADR dispositions, resolved covariance decision and new ledger.
- [x] M0.2 Frozen/tagged baseline; all three canonical platforms built/stored; Mac 34 Stata fixtures pass.
- [x] M0.2a Immutable tag, build/fetch/registration tooling and platform factory.
- [x] M0.2b Hosted Linux/Windows assets, canonical registration and verified storage.
- [x] M0.3 Working core/testkit moved; host retained; abandoned rewrite removed; full local harness/native ABI and long oracles pass.
- [~] M0.4 Mac replay passes 73 exact/repeat cases: 70 scientific passes, 3 explicit defects; matrix fixed; Linux CI qualification pending.
- [~] M0.5 Complexity ratchet measured/tested; instrumented coverage campaign running. Linux replay gate still blocks M1.
- [ ] M1.1 Two-plugin Stata selection, next default and exact public baseline. Depends on G-M0.
- [ ] M1.2a Next-only factor precedence correction. Depends on M1.1.
- [ ] M1.2b Full-matrix OLS multiway covariance correction. Depends on M1.2a acceptance.
- [ ] M1.2c Reproduce/repair slope-DoF validation. Depends on M1.2b acceptance.
- [ ] M1.2d Reproduce/repair frequency-singleton validation. Depends on M1.2c acceptance.
- [ ] M1.3 Bounded mapped feature/scientific/transaction qualification. Depends on M1.2.
- [ ] M1.4 Bundled installation and platform qualification. Selection/packaging work depends on M1.1; final qualification on M1.3.
- [ ] M1.5 Performance/coverage/complexity summaries. Depends on M1.3 and qualified platform artifacts.

## Gates

- [ ] G-M0: canonical comparators, copied implementation, real OLS/PPML replay on Linux CI, exact-copy pass, explicit matrix/exceptions and baselines.
- [ ] G-M1: defaults/selection, corrections, exact unaffected outputs, mapped qualification, installation and summaries.
- [ ] G-LICENSE: existing native-distribution gate; draft development assets do not establish release permission.

## Decisions

- Full-matrix OLS multiway PSD repair including the reported constant is approved; it is not an open question.
- Scientific agreement across thread counts; retain fixed-thread deterministic repeatability and exact-change checks.
- Bundle separate plugins; legacy/source policies are historical except retained SPEC/active ADR requirements.

## Blockers

Platform assets/replay gates remain pending. Historical v39/v24 allocation manifests already predate the frozen source; do not refresh them as proof.
The old source-string assertion for fit_absorb_matrix also fails against unchanged frozen code; retained runtime checks remain required.

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
