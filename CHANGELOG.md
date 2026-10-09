# Changelog

## 0.5.0 beta — unreleased

- Fix (Windows): the plugin now pins its own DLL on first use. Its cached
  worker threads outlive each command, so unloading the DLL (as the Windows
  loader test does right after the doctor check) could crash the process
  with an access violation when an idle worker was still running. A rebuilt
  Windows plugin now takes effect after restarting Stata. A plugin that does
  not match the installed ado files (`r(499)`) now tells the user to restart
  Stata, and the README and `help fereg` explain when a restart is needed.
- Fix: `feppml` with a slope fixed effect such as `absorb(g##c.x)` and zero
  outcomes could stop with r(459) ("prepared projection finite score
  certificate did not pass"). The iterated rectifier's projection divided the
  absorbed intercept's score moment by a solver-roundoff within norm; it now
  uses the raw intercept norm, as published fits already did. Coefficients
  agree with ppmlhdfe to 1e-14 on the failing design; previously published
  results are unchanged.
- PPML separation: a no-separation result from the iterated rectifier is now
  certified by a direct check of the stored projection instead of re-running
  the detector, which removes one expensive weighted projection; results are
  unchanged and the re-run remains the fallback. A 200,000-row PPML model with
  30% zero outcomes ran 12-22% faster in a bounded Mac check.
- Faster FE certificates (one compensated pass per group mean, a declared
  roundoff-level change to certificate diagnostics only) and faster robust
  covariance for wide models. All coefficients and standard errors are
  unchanged.
- Cleanup program (2026-10-09), phases 2 and 3: OLS estimation now honours
  Break during its numerical phase; checked batched observation stores run on
  Linux and Windows; frontend return codes and messages follow the reviewed
  contract. Hot paths drop repeated validation, duplicate certificates and
  eigensolves, strided column passes and per-cell recipe dispatch while
  keeping every published result exact. On the 18 frozen Mac workloads every
  case passes the max(5%, 1 ms) nonregression allowance; wide and two-FE
  four-worker cases ran 5-9% faster in that bounded check. One declared
  ulp-scope change makes PPML score-certificate normalization overflow-safe.
- Extend guarded hardware-FMA dispatch to standardized Gram tiles with more
  than 64 columns, preserving arithmetic order, portable execution and exact
  scientific outputs. The isolated Linux speedup now has a passing Mac
  nonregression check on all twelve standard and six wide-matrix cases.
- Pin `within`/`schwarz-precond` to the checked-CSR-slice back-substitution
  update. It removes repeated inner-loop index/data bounds checks while
  preserving arithmetic order and exact outputs; Mac nonregression passes.
- Remove the unused standalone OLS pipeline while retaining the prepared runtime,
  PPML reference path and independent explicit-dummy scientific witnesses.
- Reject unavailable model components that claim an available omission flag;
  continuous, plain-factor and interaction omission metadata remain accepted.
- Extend guarded numeric batch loading and the admitted row pipeline to Linux
  and Windows. Bounds, numeric types, main-thread ownership and first-error
  propagation are checked before direct SDK reads.
- Add bounded Stata-free native benchmarks with streamed exact-output checks,
  solver diagnostics and Linux Cachegrind instruction counts. Freeze the small
  PPML bias replay inputs and add independent exact/Philox correction witnesses.
- Preserve case-sensitive cluster identifiers in OLS `vce(cluster ...)` while
  accepting mixed-case VCE keywords. Previously an uppercase identifier could
  incorrectly select a lowercase variable or fail when only the uppercase name
  existed.
- Create Unix low-memory scratch files with owner-only permissions, independent
  of a permissive umask; retain exclusive creation and automatic cleanup.

- x86-64 plugins detect FMA and AVX at runtime and then run the standardized
  OLS moment (Gram) row updates with hardware fused multiply-add instead of a
  library call per element; other processors keep the portable path inline.
  Both paths are exactly rounded, so results match wherever the platform `fma`
  is exactly rounded. Exact-rational rounding witnesses pass on ARM macOS and
  the x86-64 macOS slice under Rosetta and native Windows x86-64; Windows also
  enforces a mandatory build gate. The prior glibc software-FMA check remains
  applicable.
  `fereg, doctor` now reports the active
  arithmetic path (`x86_64-fma`, `x86_64-portable` or `aarch64-fma`) rather than
  a processor capability probe.
- The two borrowed-MAP group-mean passes use the same runtime kernel table,
  so FMA/AVX processors run them vectorized. Controlled native-call comparisons
  on Apple M2 Ultra and Broadwell FMA passed the max(5%, 1 ms) nonregression
  allowance in all twelve workload/worker combinations per host, with exact
  scientific results. Against its immediate parent, this change reduced native
  time by 1.6-7.5% in the large single-worker two-FE x86 cases; four-worker x86
  and Mac results were mostly neutral. These bounded observations do not
  constitute full end-to-end Stata performance qualification.

- Refine finite PPML bias-effect reconstruction failures with the accepted
  weighted native solver, preserving component anchors and the original
  certificate. Report the failing channel, block and measured discrepancy.

- Require at least two distinct bias units within every retained level of both
  FE sets for PPML coefficient mean correction. Add `e(bias_support)` count and
  information-concentration summaries, explain independent-score-unit assumptions
  in the help, and preserve ordinary estimates and atomic rollback. PPML request
  and result wires are now 1.8 and 1.5; the plugin and ado files must match.

- Activated public `feppml` as a one-call wrapper over the guarded PPML
  transaction; packaged its syntax, transaction, and prediction ado files;
  published the reviewed help contract; and added the public scientific and
  staged-installation profiles. This changes no `fereg` command code, OLS
  wire, shared numerical policy, solver route, or memory threshold. Windows
  licensed-Stata testing and the formal cross-platform activated aggregate are
  deferred, so this is a testing activation rather than a qualified release.
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
- Added a private pre-activation PPML qualification path with independent
  request/result 1.1 wires, fixed bounded inference/drop/separation evidence,
  one guarded live-SPI `estimate-ppml` action,
  prepared separation and changing-weight IRLS, shared certified-linear and
  sandwich kernels, PPML-specific inference/result validation, fit-lifetime
  memory planning, and receipt-last rollback-safe publication. Added separate
  public-stub, hidden-transaction, scientific, oracle, F0 non-regression, and
  platform/performance qualification inputs. The installed public `feppml`
  command remains an explicit fail-closed `r(498)` stub until every activation
  gate passes on one exact source; no qualification result is claimed here.
  The stale experimental Rust `fit_ppml*` entry points and legacy result
  wrappers were removed in favor of the prepared internal path; ADR 0015 does
  not make the common kernel a public Rust stability promise.

No tag or GitHub release is created by this checkpoint. Qualification status
and exact accepted receipts remain recorded in the historical `STATUS.md` in Git.

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
