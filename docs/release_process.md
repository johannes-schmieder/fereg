# 0.5.0 beta qualification

1. Freeze the exact beta SHA, ADR-0007 contract, compatibility matrix, and
   pinned reference closure. Development stays on local `main`; do not push.
2. Run repository and packaging checks, Python oracles, strict C/mock SPI,
   formatting, Clippy with warnings denied, all Rust targets, allocation/store
   failure injection, panic/Break/rollback, and sanitizer/Miri/fuzz jobs where
   applicable.
3. Run licensed Stata/MP 18 quick, differential, and full profiles on the exact
   local source. Exercise the arm64 plugin natively and x86_64 slice under
   Rosetta. Record native Intel-host Stata as deferred.
4. Deploy an exact source-SHA snapshot—not Git history—to
   `/projectnb/welfgr/fereg`. Exclude `.git`, local outputs, caches, and
   `docs/references/`. Use run-scoped `input/logs/work/output/receipts`
   directories, project-scoped Rust 1.85.1, Stata 18/19 modules, `-P welfgr`,
   and no queue pin.
5. Run compute-node smoke first, then Linux correctness and clean-install
   profiles. Run paired fereg/reghdfe performance and memory comparisons in the
   same node and job; require qacct, sentinels, logs, hashes, and schemas to
   agree. Enforce ADR-0007 on approved cases of at least 10 million rows. For
   shared-engine or PPML work, also require the direct F0 profile and receipt
   integrity rules in `fereg_regression_contract.md`.
6. Build/audit target-native artifacts: Linux x86-64 glibc 2.28+, Windows x64
   MSVC, and universal macOS x86_64/arm64. Reject `target-cpu=native`,
   unexpected dynamic dependencies, missing exports, checksum mismatches, or a
   plugin that fails its build-host loader/mock tests.
7. Stage deterministic source and binary archives outside the tracked tree.
   The binary archive includes exactly one checksum-verified plugin for each
   supported platform, production Stata files, checksums, release manifest,
   SBOM, dependency audit, licenses/notices, and explicit Windows/native-Intel
   caveats. Exclude developer adapters, raw logs, reference PDFs, generated
   reference libraries, and build caches.
8. Bind every receipt and archive to the exact beta SHA and update
   `STATUS.md`, compatibility matrix, and changelog. Do not tag or create a
   GitHub release.
9. Only when all feasible offline gates are green, push the complete local
   stack once to `origin/main`. Run automatic exact-SHA Mac quick and manually
   dispatch Mac full and hosted platform qualification. Remote failures require
   a repaired, newly offline-qualified stack before another push.
10. Declare 0.5.0 source-qualified only after exact-SHA local, SCC, Mac, Linux
    build/load, Windows build/mock, and archive receipts are green. Keep the
    deferred licensed-Windows and native-Intel caveats visible.

Sanitized qualification receipts may be committed. Generated archives and raw
logs remain outside the repository.

## PPML activation sequence

The public `feppml` stub is not replaced during ordinary PPML development.
First, one exact pre-activation source must pass the private `ppml-hidden`
transaction/science matrix, pinned-comparator and independent-oracle gates,
memory/performance and constrained-resource gates, supported build/ABI and
licensed-platform checks, and the complete direct-F0 `fereg` regression
contract. Every receipt must bind the same source, artifact, dependencies,
datasets, and environment required by its manifest.

Only then may a small separately revertible activation commit replace the
public stub and update public help, package inventory, installation tests, and
postestimation routing. That commit must contain no shared numerical refactor,
solver or memory-policy change, OLS wire/syntax change, or unrelated release
work. The full local, F0, PPML, SCC, platform, clean-install, transaction/Break,
and deterministic-package gates must run again on the exact activated source.
Pre-activation receipts cannot be inferred across the activation commit.
