# fereg 0.5.0 compatibility matrix

## OLS 1.0 public surface

The implementation surface below is active in public `fereg`. Qualification
claims remain pending until the exact 0.5.0 candidate SHA passes the gates in
ADR-0007 and `STATUS.md`.

| Surface | Implemented 0.5.0 behavior | Qualification still required |
|---|---|---|
| Samples | Missingness, `if`/`in`, zero/invalid weights, singleton fixed point, `keepsingletons`, empty/one-row errors, and exact `e(sample)` identity | Exact-SHA local full and SCC Stata 18/19 parity |
| Regressors | Continuous terms, numeric factor main effects, all-categorical factor interactions, collinearity/omission status, lag/lead/difference operators, and no-regressor models. Categorical-by-continuous and continuous-by-continuous RHS interactions remain unsupported. | Full frozen differential and independent-oracle receipts |
| Fixed effects | Numeric/fixed-width-string/interacted FEs, multiple intercept/slope channels, disconnected components, absorbed sums, and normalized saved FEs. `strL`, categorical `##` expansion, and explicit RHS base overrides with interacted/sloped absorption remain unsupported. | Ill-conditioned/nonunique normalization adjudication across platforms |
| Weights | Unweighted, analytic, frequency, and probability weights; exact pinned `r(101)` rejection for importance weights | Complete exact reference parity and scaling/error receipts |
| Inference and DoF | IID, HC1 robust, one-/multiway clusters, nesting, default/none/firstpair/continuous DoF | SCC scientific matrix and platform repetition |
| Postestimation | `predict` for `xb`, absorbed contribution, `xbd`, and residuals; `test`; `estat summarize` | Public clean-install and rollback/Break repetition |
| Native transport | One substantive plugin call, Stata-thread-only SPI, receipt-last completion, rollback, panic and Break containment | Exact packaged-plugin tests on every supported build target |
| Memory/threading | `memory(fast|low|auto)`, bounded scratch cleanup, allocation/RSS/time reporting, deterministic per-action thread pools | Leak, modeled-allocation, constrained-memory, and SCC RSS gates |
| CMG graph route | Explicit `solver(cmg)` supports two large categorical intercept effects plus at most 64 non-base nuisance-FE columns. `solver(auto)` selects CMG before its MAP pilot only for the frozen no-nuisance, 8-observations-per-high-level, 10:1-cardinality topology in fast and low modes. Setup and solve fallback remain certified and visible; used solver code is 7. | Full 1m/10m route freeze, non-macOS builds, cross-platform memory/performance receipts, and license-compatible distribution decision |
| PPML FE solver | `feppml, solver(auto|map|lsmr|cmg)` is public. Auto remains MAP pending calibration; LSMR reports its fused or joint engine; CMG is prepared fused FWL and only numerical nonconvergence falls back visibly to MAP. LSMR initially requires at least two categorical intercept effects, CMG exactly two or three, and both use fast-memory execution while retaining the full PPML output surface. | Auto routing calibration, broader topology/memory support, and frozen performance qualification |

`feppml` is active as an additive public testing surface. It supports the
implemented PPML sample, fixed-effect and absorbed-slope grammar, robust and
one- through five-way clustered inference, certified separation, saved
outputs, prediction, memory modes, and native threading described by
`help feppml`. IV/GMM, group outcomes with individual FEs,
HAC/Driscoll–Kraay, other GLMs, GPU execution, and undocumented compatibility
modes are not part of 0.5.0.

## PPML activated testing boundary

The public wrapper delegates once to `_fereg_ppml`, which enters the guarded
`_fereg_estimate_ppml` transaction, and additive `estimate-ppml` plugin action.
The three ado files are now part of the package inventory and exercise public
PPML semantics through request wire 1.5 and result wire 1.2 without altering
the OLS action or result layout.

Activation requires exact-source scientific agreement with the pinned
`ppmlhdfe` closure and independent oracles; transaction, interruption,
allocation, memory, performance, and supported-platform qualification; and the
direct F0 `fereg` non-regression gate. The macOS and Linux implementation gates
have been exercised, but Windows licensed-Stata testing and the formal
cross-platform activated aggregate are explicitly deferred. The rows describe
implemented behavior for testing, not a qualified release claim.

## Platform qualification

| Stata | OS / architecture | Artifact | 0.5.0 qualification target |
|---|---|---|---|
| 18 and 19 | Linux x86-64, glibc 2.28+ | `fereg_linux.plugin` | SCC licensed correctness, clean install, scientific parity, performance, memory, build/load, and audit |
| 18+ | Windows x86-64 | `fereg_windows.plugin` | Hosted MSVC build, static CRT/export/dependency audit, and mock-SPI qualification; licensed Windows Stata deferred |
| 18+ | macOS Intel x86-64 | universal plugin x86_64 slice | Rosetta exercise on Apple Silicon; native Intel-host Stata deferred |
| 18 | macOS Apple Silicon arm64 | universal plugin arm64 slice | Native local licensed quick/full and clean-install qualification |
| 18+ | macOS universal | `fereg_macos.plugin` | Both slices audited and packaged under the same verified checksum |

Distributed builds must not use `target-cpu=native`. Evidence on one operating
system, architecture, Stata version, or plugin checksum is not evidence for
another.
