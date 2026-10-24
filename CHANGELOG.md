# Changelog

## 0.5.0 beta.3 — 2026-10-24

This beta publishes the accepted Auto-routing, CMG host-team and ownership/cell
improvements. It includes a universal Mac plugin, tested with licensed Stata on
Apple Silicon. Linux and Windows binaries are not distributed. See
[validation](docs/VALIDATION.md) for evidence and platform limitations.

- Eligible many-FE OLS Auto uses retained preparation and strict unseeded LSMR.
- Eligible two-FE OLS Auto uses the selected structural CMG gates. With Auto
  preconditioning, a global intercept and two categorical-intercept FE terms,
  it omits provably unused moments, retains projected scores and constructs cells
  only after successful CMG commitment.
- Explicit solver requests, memory controls, numerical certificates and atomic
  publication/rollback remain supported. Historical developer injections retain
  their comparison behavior.
- The repository keeps current tools and concise acceptance records; completed
  experimental programmes remain recoverable from Git history.

### Commands

- `fereg`: OLS and weighted least squares with many absorbed fixed effects.
- `feppml`: Poisson pseudo-maximum likelihood with absorbed fixed effects,
  one call over the guarded PPML transaction.
- Postestimation: `predict`, `test` and `estat summarize`, as documented in
  `help fereg_postestimation` and `help feppml`.
- Installation diagnostics: `fereg, selftest [threads(#) verbose]` combines
  version information, ado/plugin compatibility and bounded native checks.
  `verbose` includes build/host details, individual checks and the active
  arithmetic path (`x86_64-fma`, `x86_64-portable` or `aarch64-fma`).
  The former `version` and `doctor` options have been removed.

### Models and inference

- Absorbed terms: numeric and string factors, `#` interactions, absorbed
  slopes (`firm##c.time`, `firm#c.time`, `worker#c.(time tenure)`), and named
  saved effects (`absorb(fe_firm=firm)`). Ordinary regressors support factor
  variables, categorical interactions and time-series operators.
- Weights: analytic, frequency and probability weights; importance weights are
  rejected. Probability weights use robust inference by default.
- Singletons are removed to a fixed point unless `keepsingletons` is given.
- Inference: `vce(unadjusted|robust|cluster ...)` with multiway clustering and
  `dof(pairwise|firstpair|continuous|none)`. Multiway covariance receives a
  certified PSD repair; robust and one-way clustered covariance must already
  be PSD.
- Saved outputs: `residuals()` and saved fixed effects; `predict` then also
  supports `xbd`, `d` and `residuals`.
- PPML: certified separation detection (`separation(fe simplex ir)` by default;
  `mu` is optional and `none` disables it), saved predictions and drop reasons,
  and optional `biascorrection(exact|randomized)` with an explicit
  `biasunit()`. Bias correction requires at least two distinct bias units in
  every retained level of both FE sets and reports `e(bias_support)`.

### Solvers and execution

- `solver(auto|map|it|squarem|lsmr|cmg)` and
  `preconditioner(auto|off|diagonal|schwarz)` preserve explicit choices. Auto
  selects the qualified categorical many-FE and two-FE policies above; zero/one
  FE, slopes and PPML retain their existing paths. Certification and visible
  fallback semantics remain unchanged.
- Results are published only after the fixed-effect, score and covariance
  certificates pass, with receipt-last atomic publication and rollback on
  error, panic, store failure or Break.
- `memory(auto|fast|low)` with `poolsize(#)` plans solver, topology and
  scratch memory before large allocations; low-memory scratch files are
  owner-only and removed automatically.
- `threads(#)` and `deterministic` control execution. Break interrupts
  estimation, derived outputs, saved-effect recovery and long serial
  inference passes.
- x86-64 plugins use hardware fused multiply-add when FMA and AVX are present
  and the portable path otherwise; both are exactly rounded.

### Installation and compatibility

- Requires Stata 18 or newer and one platform plugin (`fereg_macos.plugin`,
  `fereg_linux.plugin` or `fereg_windows.plugin`). A missing plugin fails with
  `r(601)`.
- Restart Stata after installing or updating a package that was already used
  in the session; on Windows, restart before updating. A plugin that does not
  match the installed ado files fails with `r(499)` and asks for a restart.
- The former `engine()` option fails with `r(198)`; `FEREG_ENGINE` has no
  effect. `solver(cg)` is rejected with `r(198)`.
- Plugin and ado files must come from the same build; wire identities are in
  [`docs/plugin_protocol.md`](docs/plugin_protocol.md).

### Fixes during the beta

- Fixed an OLS slowdown at higher worker counts caused by shared scratch
  writes during fixed-effect norm accumulation. Accumulators now stay private
  to each worker; observation order and scientific results are unchanged.
- Windows: the plugin pins its own DLL, so unloading it while a cached worker
  was still running can no longer crash Stata.
- `feppml` with slope fixed effects and zero outcomes no longer stops with
  `r(459)` in the iterated rectifier's score certificate.
- Saved fixed-effect recovery on weak multiway graphs requires both recovery
  certificates before stopping, instead of rejecting valid results with
  `r(459)`.
- `fereg` `vce(cluster ...)` preserves case-sensitive cluster identifiers
  while accepting mixed-case keywords, and `fereg` `solver()` and
  `preconditioner()` names are case-insensitive.
- A Break during `feppml` result validation returns `r(1)` instead of
  `r(3499)`. `feppml` rejects a string outcome, weight or continuous regressor
  before reading data.
- `feppml` admits much less memory for large samples with zero outcomes:
  separation records are charged for at most 256 generations rather than one
  per observation (about 9.6 GB less at ten million rows). Longer cascades grow
  the bank fallibly, including space for exact-FE follow-up reports, without
  imposing a 256-generation limit on otherwise valid estimates.
- The multiway-cluster PSD repair certificate in `fereg` is purely relative,
  as in `feppml`, so an eigensystem whose error the former unit floor hid on a
  tiny-scale covariance is rejected rather than published.
- The internal `_fereg_ols.ado` developer adapter is no longer installed.
- Explicit `solver(lsmr)` in `fereg` now iterates to one hundredth of
  `tolerance()` (at least 1e-15) and certifies at `tolerance()`. It no longer
  stops with `r(459)` when a fixed-effect certificate lands just above the
  tolerance, as happened with weights and three absorbed effects and, for
  large samples with saved residuals or effects, in the final residual
  projection. Explicit-LSMR results change at roundoff level and take somewhat
  more LSMR iterations.

Apart from that explicit-LSMR change, coefficients and standard errors did
not change in the October 2026 cleanup and improvement programmes;
certificate diagnostics changed only within declared roundoff scopes. The development history of this beta, including
benchmark tooling and qualification receipts, is in Git.

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
