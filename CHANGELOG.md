# Changelog

## 0.5.0 beta — unreleased

- Activated public `fereg` OLS/WLS with action protocol 1.3, OLS result wire
  1.3, and model wire 2.1 while retaining receipt-last atomic publication and
  rollback on error, panic, store failure, or Break.
- Completed the frozen OLS 1.0 surface: ordinary factors/interactions,
  numeric/string/interacted fixed effects, absorbed slopes, singleton and DoF
  policies, accepted weights, time-series operators, iid/robust/multiway-
  cluster inference, saved outputs, prediction, `test`, and
  `estat summarize`.
- Added `memory(fast|low|auto)`, bounded scratch cleanup, allocation preflight,
  native allocation/RSS/timing diagnostics, per-action thread pools, and
  deterministic thread-count evidence.
- Replaced the former scalar CG extrapolation with genuine batched weighted
  conjugate gradients over the symmetric-Kaczmarz projection operator,
  including independent final certification, per-RHS convergence handling,
  safe roundoff restarts, borrowed categorical topology, and the
  comparator-equivalent Hestenes energy stop. Ordinary CG publication now
  requires both Hestenes convergence and the independent FE certificate.
- Added the first bounded automatic policy: direct one-FE demeaning, up to four
  symmetric MAP pilot sweeps, early certified MAP completion, LSMR only for a
  frozen strong-contraction/small-coefficient region, and genuine CG otherwise.
  Auto-selected LSMR has at most one visible fallback to certified CG.
- Added the deterministic AKM benchmark generator and quick/matrix/milestone/
  freeze runner with reghdfe 6.14.1 scientific preflights, gregress 1.11.8
  diagnostics, dataset/dependency hashes, cold/warm dispersion, native timing,
  and explicit recording of failed exploratory arms.
- Reused the QR-reconstructed within residual, refined it only when its fresh
  FE certificate fails, reduced redundant sample transport and moment scans,
  and selected a 65,536-row parallel projection cutoff from matched one-/eight-
  thread profiling. Added fused small-batch projection, reusable CG projection
  storage, deterministic fixed-chunk parallel dot products, and reuse of
  unchanged setup norms during the initial certificate.
- Identified the beta consistently as 0.5.0 and added deterministic
  source/release staging, checksum, dependency, SBOM/license, and platform-
  caveat requirements.
- Kept `feppml` as an explicit fail-closed `r(498)` stub. PPML is not part of
  the 0.5.0 beta acceptance gate.

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
