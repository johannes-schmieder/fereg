# Compatibility matrix

## OLS 1.0 scientific surface

This is a target contract, not a public implementation claim. `fereg` still
returns `r(498)`.

The latest accepted candidate under the current option-A policy is the frozen
31-executable/9-blocked hidden projection at
`f61fee923db2bd3544d457dfe50de8117d27adda`. Later signed-residual-DoF slope
receipts are historical only. The option-A positive-DoF slope correction is
prepared but not yet exact-green or resealed.

| Surface | Accepted state | Remaining gate |
|---|---|---|
| Numeric samples and singletons | Missingness, `if`/`in`, weighted singleton fixed point, `keepsingletons`, compact row identity, and registered failure cases are covered on the hidden surface | Preserve exact behavior for every newly enabled model/weight/output path |
| Ordinary regressors | Continuous columns, bounded numeric factor main effects, and all-categorical numeric interactions with canonical omission statuses | Broader factor policy, mixed interactions, time-series operators, and public syntax |
| Fixed effects | No/one/multiple intercept-only numeric FEs, disconnected/repeated spans, categorical FE interactions, certified MAP, and final-sample rebasing | Option-A intercept-plus-slope reseal, slope-only/multiple slopes, strings, saved FEs, continuous DoF |
| Weights | Unweighted, analytic, frequency, and probability objectives plus domain/scaling/frequency-expansion evidence | `iweight` policy and public metadata/compatibility |
| Inference | Certified iid, HC1, and one-/multiway clustered inference on accepted bounded surfaces; nesting and default/none/firstpair DoF are covered | Positive-DoF absorbed slopes, continuous DoF, every remaining model form, and public activation |
| Native transport | One-load/one-call private result transport, receipt-last logical atomicity, exhaustive store-failure tests, and licensed rollback/Break transaction | Preserve the same boundary for remaining semantics and public dispatch |
| Outputs | Hidden b/V/status/fit/DoF/e(sample)/optional residual posting | `xb`, `xbd`, absorbed sum, saved FE outputs, `test`, `estat summarize`, public predict/postestimation |
| Performance and memory | Architectural bounds and benchmark harnesses exist | Scientific closure, fast/low-memory parity, frozen speed/RSS gates, clean-machine qualification |

The nine frozen blockers are listed authoritatively in `STATUS.md`, beginning
with `intercept_and_slope` under ADR 0006. Public `feppml`,
`estimate-ols`, and `estimate-ppml` also remain fail-closed.

## Platform construction and validation

| Stata | OS / architecture | Artifact | Current evidence | Required release validation |
|---|---|---|---|---|
| 18+ | Linux x86-64, glibc 2.28+ | `fereg_linux.plugin` | Build script encoded; no accepted native licensed-Stata evidence | Build, audit, load, doctor, and selftest on supported systems |
| 18+ | Windows x86-64 | `fereg_windows.plugin` | Build script encoded; no accepted native licensed-Stata evidence | Static CRT/export audit and Stata 18/19 load/reload |
| 18+ | macOS Intel x86-64 | universal plugin slice | Loader exercised under Rosetta only | Native Intel-host Stata where available |
| 18+ | macOS Apple Silicon arm64 | universal plugin slice | Native Stata 18 and private-runner evidence | Clean-machine and newest-supported-Stata repetition |
| 18+ | macOS universal | `fereg_macos.plugin` | Both slices built and audited on the validated Mac | Confirm exact slices and release packaging |

Distributed builds must not use `target-cpu=native`. A result on one platform is
not evidence for another.

## Deferred non-goals

The initial release does not include IV/GMM, group-level outcomes with
individual FEs, general HAC/Driscoll–Kraay, other GLM families, GPU execution,
or undocumented compatibility modes. Unsupported requests must fail explicitly.
