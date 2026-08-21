# Changelog

## 0.1.0-phase01 — 2026-08-20

- Added the Phase 0–1 Rust workspace, versioned protocol, typed errors, local
  thread-team abstraction, deterministic factor/interaction primitives, memory
  preflight, diagnostics, and native self-test.
- Added the official Stata SPI 3.0 sources, minimal C bridge, panic containment,
  mock-SPI tests, binary audits, and target-native Linux/Windows/macOS build
  scripts and CI.
- Added Stata 18+ loader and diagnostics commands plus a metadata-only factor,
  absorb, slope, and cluster parser prototype.
- Added explicit fail-closed `stfeols`, `stfepois`, prediction, and `estat`
  surfaces. No estimator is implemented or claimed in this release.
- Added randomized factorization, tuple-interaction, thread-partition, and memory
  preflight oracles. Corrected the zero-finite-column memory edge case so low
  mode reserves no artificial regressor pool.
