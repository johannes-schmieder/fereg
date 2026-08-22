# Compatibility matrix

## OLS 1.0 scientific surface

This is a target contract, not a public implementation claim. `stfeols` still
returns `r(498)`. The frozen reference-only `ols_v1` receipt defines 40 semantic
cases. The developer-only candidate projection currently passes 30 of them and
retains ten exact blockers; `public_activation=false` and performance remains
unmeasured.

| Surface | OLS-v1 reference cases | StFixest state |
|---|---|---|
| No, one, and multiple FEs | no-FE, factor FE, two/three-way FE, disconnected graph | Hidden transport solves the supported repeated single-source intercept-only numeric-FE topology through certified borrowed MAP; general FE forms and the public estimator remain disabled |
| FE interactions/slopes | categorical interactions, intercept-plus-slope, slope-only, multiple slopes | Disabled pending FE certificates |
| Ordinary regressors | continuous, factors, factor interactions, lag/lead/difference operators across panel calendar gaps, no-regressor, FE-collinear column | Hidden transport supports continuous, numeric factor main effects and all-categorical numeric interactions with canonical statuses; mixed/time-series and public estimation remain disabled |
| Samples/singletons | missing outcome/regressor/FE/cluster, `if`/`in`, cascade, `keepsingletons`, empty and one-row errors | The candidate-bound hidden projection passes the numeric sample, cascade/keep, missingness, empty-sample, and one-observation cases with exact identities and pinned 402/401/2001 errors where applicable. Public syntax/identity remains disabled. |
| Weights | unweighted, `aweight`, `fweight`, `pweight`, `iweight` probe, scaling, invalid-domain probes, and invalid-weight-outside-candidate precedence | Candidate-bound parity passes unit/a/f/p objectives, scaling, domain, missing, large-weight, candidate-scope, and weighted-singleton cases. `iweight_probe` remains an explicit blocker; public syntax/metadata remains disabled. |
| Inference | iid, robust, one/two/three-way cluster, cluster interaction, nesting, default/none/firstpair/continuous DoF | Prepared iid, HC1 robust, and one-/multiway clustered compositions are certified for the current intercept-only numeric-FE surface and consumed only by hidden `estimate-ols-test`. One source load and one selected same-call composition write private versioned shadows with `COMPLETE` last; the current candidate-bound sweeps cover 222 Fake and 219 live store ordinals. The hidden eclass helper revalidates and posts iid/HC1/one-/multiway covariance under `_estimates hold`, including exact whole-Mata q2/q3 CGM restored-state preservation and rollback/actual-Break evidence. `dof_continuous`, broader model forms, and public activation remain pending. |
| Outputs | `e()` scalars, `e(b)`, `e(V)`, `e(sample)`, residual, `xb`, `xbd`, absorbed sum, named FEs, `test`, `estat summarize` | The hidden helper transactionally posts bounded b/V/status/fit/DoF/diagnostics/e(sample)/optional-residual outputs and restores prior named/null estimates on every tested failure/Break. It remains developer-only; prediction, absorbed/saved FE outputs, postestimation, broader sealed parity, and public dispatch remain disabled. |
| Benchmarks | dense and sparse two-FE, absorbed slope, clustered VCE, moderate K | 10M+ primary cases at 1/8 threads; scientific parity/certificates precede 1 cold + 5 fresh-process timing/RSS gates; candidate blocked |

The ten frozen blockers are: `multi_fe_interactions` (absorbed categorical
interaction and next priority), `intercept_and_slope`, `slope_only_multiple`,
`string_fe`, `time_series_lag`, `time_series_lead_difference_gaps`,
`iweight_probe`, `dof_continuous`, `saved_outputs`, and
`postestimation_test_summarize`. Broader ordinary factor/interactions remain
activation work even though the bounded numeric candidate supports numeric
factor main effects. Public `stfeols`, `stfepois`, `estimate-ols`, and
`estimate-ppml` remain `r(498)`.

The active developer receipt is
`benchmarks/reports/captured/darwin-arm64/stata18/ols_hidden_numeric/20260822T132813Z-d731b32`
(payload seal
`1558571563006bd3f5a1ce5a39b33f1f552256a83212a865d51cebdbdfe37279`).
Its external identity manifest binds the file SHA-256, git blob, and mode, and
all capture/supporting gates use the identical universal x86_64+arm64 staged
plugin. Older candidate receipts are historical rather than active evidence.

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
