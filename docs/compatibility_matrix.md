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
| CMG prototype | Explicit `solver(cmg)` for two large categorical intercept effects plus at most 64 non-base nuisance-FE columns; certified CG polishing/fallback; solver code 7 | Large AKM speed/RSS evidence, non-macOS builds, and any automatic routing claim |

`feppml` is installed only as a fail-closed `r(498)` stub and is outside the
OLS candidate. IV/GMM, group outcomes with individual FEs, HAC/Driscoll–Kraay,
other GLMs, GPU execution, and undocumented compatibility modes are not part of
0.5.0.

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
