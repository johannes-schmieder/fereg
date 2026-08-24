# Compatibility matrix

## OLS 1.0 scientific surface

This is a target contract, not a public implementation claim. `fereg` still
returns `r(498)`.

The active option-A source projects 32 executable/8 blocked hidden cases and
binds the newly resealed positive-DoF comparator. Signed-residual-DoF slope
receipts are historical only. Acceptance still requires this source's own
reachable exact-SHA Rust and licensed-Stata receipts; `STATUS.md` is not
advanced by source preparation alone.

| Surface | Accepted state | Remaining gate |
|---|---|---|
| Numeric samples and singletons | Missingness, `if`/`in`, weighted singleton fixed point, `keepsingletons`, compact row identity, and registered failure cases are covered on the hidden surface | Preserve exact behavior for every newly enabled model/weight/output path |
| Ordinary regressors | Continuous columns, bounded numeric factor main effects, and all-categorical numeric interactions with canonical omission statuses | Broader factor policy, mixed interactions, time-series operators, and public syntax |
| Fixed effects | No/one/multiple intercept-only numeric FEs, disconnected/repeated spans, categorical FE interactions, option-A intercept-plus-slope, certified MAP, and final-sample rebasing | Slope-only/multiple slopes, strings, saved FEs, continuous DoF |
| Weights | Unweighted, analytic, frequency, and probability objectives plus domain/scaling/frequency-expansion evidence | `iweight` policy and public metadata/compatibility |
| Inference | Certified iid, HC1, and one-/multiway clustered inference on accepted bounded surfaces; positive-DoF intercept-plus-slope, nesting, and default/none/firstpair DoF are covered | Continuous DoF, every remaining model form, and public activation |
| Native transport | One-load/one-call private result transport, receipt-last logical atomicity, exhaustive store-failure tests, and licensed rollback/Break transaction | Preserve the same boundary for remaining semantics and public dispatch |
| Outputs | Hidden b/V/status/fit/DoF/e(sample)/optional residual posting | `xb`, `xbd`, absorbed sum, saved FE outputs, `test`, `estat summarize`, public predict/postestimation |
| Performance and memory | Architectural bounds and benchmark harnesses exist | Scientific closure, fast/low-memory parity, frozen speed/RSS gates, clean-machine qualification |

The eight frozen blockers begin with `slope_only_multiple`; `STATUS.md` remains
the authority for the latest accepted evidence. Public `feppml`,
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
