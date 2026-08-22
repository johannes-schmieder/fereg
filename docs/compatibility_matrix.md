# Compatibility matrix

## OLS 1.0 scientific surface

This is a target contract, not an implementation claim. `stfeols` still returns
`r(498)`. The sealed reference-only `ols_v1` receipt covers the rows below, but
it proves pinned `reghdfe` behavior rather than StFixest parity.

| Surface | OLS-v1 reference cases | StFixest state |
|---|---|---|
| No, one, and multiple FEs | no-FE, factor FE, two/three-way FE, disconnected graph | Developer boundary prepares repeated single-source numeric FE recipes, their singleton fixed point, and a deduplicated structural topology; no solver `FeDesign`, solve, or public estimator |
| FE interactions/slopes | categorical interactions, intercept-plus-slope, slope-only, multiple slopes | Disabled pending FE certificates |
| Ordinary regressors | continuous, factors, factor interactions, lag/lead/difference operators across panel calendar gaps, no-regressor, FE-collinear column | Parser metadata only; estimator disabled |
| Samples/singletons | missing outcome/regressor/FE/cluster, `if`/`in`, cascade, `keepsingletons`, empty and one-row errors | Safe-core numeric precedence is composed through cluster reason 6; supported numeric FE cascades assign reason 8 and compact all aligned sources, `keepsingletons` is wired, and cluster group sizes never participate in deletion. Plugin final-sample identity and estimator-level insufficient-sample errors remain pending. |
| Weights | unweighted, `aweight`, `fweight`, `pweight`, `iweight` probe, scaling, invalid-domain probes, and invalid-weight-outside-candidate precedence | Every kind disabled pending adjudication |
| Inference | iid, robust, one/two/three-way cluster, cluster interaction, nesting, default/none/firstpair/continuous DoF | Core-only prepared iid, HC1 robust, and one-/multiway clustered compositions are certified for the current intercept-only numeric-FE surface. The clustered checkpoint supports 1--10 numeric declared dimensions, source-tuple interactions, nesting before mobility, distinct reported/fit/correction DoF, the common pinned finite-sample correction, exact two-stage CGM repair, all four implemented weight rules, slope-only Wald, and independent certificates. Strict internal comparisons remain separate from frozen DEFAULT-`reghdfe` receipt-only behavior caps. Legacy generic VCE/DoF code is not eligible for posting. Plugin exposure, atomic result storage, remaining model/FE forms, and estimator-level parity remain pending. |
| Outputs | `e()` scalars, `e(b)`, `e(V)`, `e(sample)`, residual, `xb`, `xbd`, absorbed sum, named FEs, `test`, `estat summarize` | Postestimation remains fail-closed |
| Benchmarks | dense and sparse two-FE, absorbed slope, clustered VCE, moderate K | 10M+ primary cases at 1/8 threads; scientific parity/certificates precede 1 cold + 5 fresh-process timing/RSS gates; candidate blocked |

Deferred 1.0 non-goals remain IV/GMM, group-level outcomes with individual FEs,
general HAC/Driscoll–Kraay, other GLM families, GPU execution, and historical or
undocumented compatibility modes. Unsupported requests must error explicitly.

## Platform construction and validation

The entries below distinguish an encoded build target from a target actually
validated in licensed Stata.

| Stata | OS / architecture | Toolchain | Artifact | Construction-sandbox status | Required release validation |
|---|---|---|---|---|---|
| 18+ | Linux x86-64, glibc 2.28+ | Rust 1.85.1 + GCC/Clang | `stfixest_linux.plugin` | Build script encoded; not compiled here | Load/doctor/selftest on oldest and newest supported Stata |
| 18+ | Windows x86-64 | Rust MSVC + VS 2022 Build Tools, `/MT` | `stfixest_windows.plugin` | Build script encoded; not compiled here | `dumpbin` audit, Stata 18/19 load and repeated reload |
| 18+ | macOS Intel x86-64 | Xcode clang + Rust x86-64 | one slice of `stfixest_macos.plugin` | Loader and slice executed under Rosetta on Apple Silicon; this is emulated evidence only | Run under native Intel Stata where available |
| 18+ | macOS Apple Silicon arm64 | Xcode clang + Rust arm64 | one slice of `stfixest_macos.plugin` | Native Stata 18 version/doctor/selftest and reload fixtures pass | Repeat on clean machine and newest supported Stata |
| 18+ | macOS universal | `lipo` joins both slices | `stfixest_macos.plugin` | Universal audit encoded | Verify `lipo -archs` reports exactly `x86_64 arm64` |

## CPU baseline

Distributed builds do not use `target-cpu=native`. Runtime diagnostics report a
conservative CPU feature path, while all correctness code remains available on
the baseline architecture. Future SIMD kernels must retain a scalar fallback
and be differential-tested.

## Stata interface

The C sources implement Stata SPI 3.0, which is the interface used by Stata 18+
for native C plugins. The ado layer also performs an explicit Stata-major guard
before loading a platform binary.

Stata 18 reports `c(os)==Unix` on both tested macOS execution modes. The loader
therefore uses a `Mac*` match on `c(machine_type)` before treating remaining
Unix runtimes as Linux. Static tests cover Apple-Silicon and Intel Mac labels,
GUI-style `MacOSX`, Unix PC, Windows, and an unknown fail-closed case.
