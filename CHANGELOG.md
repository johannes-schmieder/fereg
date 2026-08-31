# Changelog

## 0.5.0 beta — unreleased

- Standardized the installed help with the shared author header and contact
  footer, source acknowledgements, and one-click execution for all twelve
  documented examples.
- Activated public `fereg` OLS/WLS with action protocol 1.4, OLS result wire
  1.4, and model wire 2.1 while retaining receipt-last atomic publication and
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
- Qualified direct Schwarz-preconditioned LSMR for slow-mixing automatic
  routes, tightened its internal tolerance by two decimal orders, removed the
  65,536-coefficient cap, and retained one visible certified-CG fallback from
  the saved MAP pilot. Added a sealed 100-digit weak-graph oracle,
  observation-space projection audit, 100-times-tighter large-case stability
  checks, and qualification-only forced restarted-CG comparison arms.
- Added joint execution/solver memory planning for CG, LSMR, Schwarz, RHS,
  topology, and thread-reduction workspaces. Automatic routing now chooses CG
  if LSMR does not fit and fails before large allocation if neither route fits;
  native model accuracy and leak/preflight checks replace process-RSS ratios as
  hard solver-promotion gates. No public `maxmemory()` syntax was added.
- Added the deterministic AKM benchmark generator and quick/matrix/milestone/
  freeze runner with reghdfe 6.14.1 scientific preflights, gregress 1.11.8
  diagnostics, dataset/dependency hashes, cold/warm dispersion, native timing,
  and explicit recording of failed exploratory arms.
- Added the explicit local-prototype `solver(cmg)` path for sparse AKM models:
  one reusable worker--firm CMG hierarchy, batched PCG right-hand sides, a
  rank-revealing Schur layer for up to 64 nuisance FE columns, and unchanged
  observation-space certification with short CG polishing or visible fallback.
- Promoted CMG into `solver(auto)` for a frozen conservative AKM topology:
  intercept-only two-way graphs with no nuisance Schur block, at least eight
  observations per highest-cardinality level, and a core level-count ratio of
  at least 10:1. Selection occurs before the MAP pilot and reuses one hierarchy
  in fast and low modes. Added upstream conservative/exact CMG memory
  accounting, joint preflight, visible setup/solve fallback, a direct final-
  residual certificate that avoids near-zero PCG work, permanent calibration
  and slowdown gates, and Rust/Stata route-boundary tests. The graph PCG solve
  runs at one-hundredth of the public FE tolerance so the downstream OLS score
  certificate passes without a separate post-QR correction.
- Reworked the public README and Stata help into a complete user-facing guide,
  with explicit absorbed-term grammar, option defaults, weight and
  postestimation semantics, twelve self-contained examples, and a reproducible
  comparison against reghdfe and Stata 19 HDFE areg.
- Reused the QR-reconstructed within residual, refined it only when its fresh
  FE certificate fails, reduced redundant sample transport and moment scans,
  and selected a 65,536-row parallel projection cutoff from matched one-/eight-
  thread profiling. Added fused small-batch projection, reusable CG projection
  storage, deterministic fixed-chunk parallel dot products, and reuse of
  unchanged setup norms during the initial certificate.
- Fixed saved fixed-effect recovery on weak multiway graphs by requiring both
  the FE-moment and observation-space reconstruction certificates before the
  recovery loop stops. The previous moment-only stop could reject a valid
  recovered target with `r(459)` even though additional MAP sweeps certified it.
- Removed the full observation-space MAP recovery rerun from saved fixed-effect
  output. Fast-mode LSMR and CMG fits now retain their solved FE coefficients;
  CG propagates compact FE coefficients through its recurrence; Irons--Tuck
  propagates the same extrapolation in coefficient space; and automatic routing
  combines its MAP-pilot coefficients with the selected LSMR or CG continuation.
  Fast intercept-only MAP now accumulates its level coefficients during the
  original batched residualization and forms the post-fit absorbed product with
  no recovery solve. Routes without a retained product use reconstruction-aware
  CG with a bounded iteration budget, followed by a four-sweep forward-
  Kaczmarz/steepest-descent pilot and an LSMR remainder solve. Slope recovery
  uses strict LSMR, and certified MAP remains the fail-closed final fallback.
  Every route retains the established component normalization plus independent
  observation-space reconstruction and FE-moment certificates.
- Reduced the remaining fast MAP saved-output overhead by storing retained
  coefficients in the level-major order produced by the projection kernel,
  accumulating them inside the parallel group-mean pass, and transposing once
  only after MAP certification. Publication now reuses exact connectedness
  already established by the DoF calculation and reuses the absorbed-target
  buffer when only FE channels are requested. Alternating warm Stata/MP runs
  measured 3.4 percent overhead at 30,000 rows and 2.8 percent at 300,000 rows,
  with the reconstruction and FE-moment publication gates unchanged.
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
