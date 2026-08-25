# Changelog

## 0.9.0 — release-candidate source, 2026-08-24

- Activated public `fereg` OLS/WLS with action protocol 1.2, OLS result wire
  1.2, and model wire 2.1 while retaining receipt-last atomic publication and
  rollback on error, panic, store failure, or Break.
- Completed the frozen OLS 1.0 surface: ordinary factors/interactions,
  numeric/string/interacted fixed effects, absorbed slopes, singleton and DoF
  policies, accepted weights, time-series operators, iid/robust/multiway-
  cluster inference, saved outputs, prediction, `test`, and
  `estat summarize`.
- Added `memory(fast|low|auto)`, bounded scratch cleanup, allocation preflight,
  native allocation/RSS/timing diagnostics, per-action thread pools, and
  deterministic thread-count evidence.
- Identified the candidate consistently as 0.9.0 and added deterministic
  source/release staging, checksum, dependency, SBOM/license, and platform-
  caveat requirements.
- Kept `feppml` as an explicit fail-closed `r(498)` stub. PPML is not part of
  the 0.9.0 release-candidate acceptance gate.

No tag or GitHub release is created by this checkpoint. Qualification status
and exact accepted receipts are recorded in `STATUS.md`.

## 0.1.0-phase01 — 2026-08-20

- Added the Phase 0–1 Rust workspace, versioned protocol, typed errors, local
  thread-team abstraction, deterministic factor/interaction primitives, memory
  preflight, diagnostics, and native self-test.
- Added the official Stata SPI 3.0 sources, minimal C bridge, panic containment,
  mock-SPI tests, binary audits, and target-native Linux/Windows/macOS build
  scripts and CI.
- Added Stata 18+ loader and diagnostics commands plus a metadata-only factor,
  absorb, slope, and cluster parser prototype.
- Added explicit fail-closed `fereg`, `feppml`, prediction, and `estat`
  surfaces. No estimator is implemented or claimed in this release.
- Added randomized factorization, tuple-interaction, thread-partition, and memory
  preflight oracles. Corrected the zero-finite-column memory edge case so low
  mode reserves no artificial regressor pool.
