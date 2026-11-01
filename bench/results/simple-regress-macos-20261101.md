# Simple OLS: fereg versus Stata regress, 1 November 2026

On this Mac, fereg is faster at one core with 1–16 RHS variables; regress is faster at two or more cores throughout this grid. At eight cores, regress is 3.46–5.04 times faster. Giving fereg sixteen workers produces little additional benefit. These are descriptive desktop timings of the current published binary, with no runtime changes or activation decision.

Apple M2 Ultra, 24 physical/logical cores, 192 GiB RAM; Stata/MP 19.0 for Apple Silicon, revision 12 Aug 2026. The Stata license permits eight cores. fereg 0.5.0-beta.12, runtime source `864960f1a6c2a2d1d58747d63a4475843c9c4e28`, published macOS plugin SHA-256 `28aed23eb1c8b8575fb1e1d3d49e7a2513066507353cd694f8c5269afc94f75c`. Repository HEAD at measurement: `4f89bdddf13647774eb82a72f58e5fdc49bf0ac5`.

## Complete command times

Each cell is **fereg / regress**, in seconds, the median of five full-command wall-clock timings. Both receive the indicated processor budget through eight cores. The last column gives fereg sixteen native workers and regress eight Stata processors, its licensed maximum; it is not a matched sixteen-core comparison.

| RHS variables | 1 core | 2 cores | 4 cores | 8 cores | fereg 16 / regress 8 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 0.207 / 0.314 | 0.199 / 0.151 | 0.192 / 0.087 | 0.192 / 0.053 | 0.196 / 0.052 |
| 2 | 0.242 / 0.377 | 0.229 / 0.188 | 0.222 / 0.106 | 0.216 / 0.062 | 0.227 / 0.062 |
| 4 | 0.307 / 0.469 | 0.282 / 0.208 | 0.271 / 0.117 | 0.274 / 0.068 | 0.273 / 0.070 |
| 8 | 0.374 / 0.512 | 0.330 / 0.221 | 0.315 / 0.123 | 0.308 / 0.072 | 0.318 / 0.072 |
| 16 | 0.546 / 0.622 | 0.464 / 0.271 | 0.448 / 0.149 | 0.434 / 0.086 | 0.423 / 0.086 |
| 32 | 0.954 / 0.832 | 0.776 / 0.437 | 0.738 / 0.245 | 0.662 / 0.140 | 0.637 / 0.138 |
| 64 | 2.021 / 1.763 | 1.711 / 1.082 | 1.556 / 0.567 | 1.378 / 0.297 | 1.418 / 0.297 |

At one core, fereg is 1.14–1.56 times faster for 1–16 RHS variables; regress is about 1.15 times faster for 32–64 RHS variables. From one to eight cores, regress accelerates by 5.93–7.23 times, while fereg accelerates by only 1.08–1.47 times. Sixteen fereg workers range from a 4% speed improvement to a 5% slowdown relative to eight; this small desktop difference is descriptive.

## Model and schedule

- Exactly 1,000,000 observations, all double precision; an intercept, no explicit FE categories, no weights, conventional IID covariance, default public options. The resident dataset contains y and all 64 x variables throughout; each regression uses a prefix of the same RHS list.
- NumPy 1.26.4 PCG64 seed 739391 draws `X = rng.standard_normal((1000000,64))` in C order, followed by `noise = rng.standard_normal(1000000)`. The response is `y = 1 + 0.5*X[:,0] + noise`. RHS widths are 1, 2, 4, 8, 16, 32 and 64. Requested worker counts are 1, 2, 4, 8 and 16.
- Commands: `quietly fereg y x1-xK, threads(T)` and `quietly regress y x1-xK`, with `set processors min(T,8)` outside timing for both. fereg requested and resolved native budgets agree in every call; these budgets do not assert that every stage occupies all workers.
- One warm-up per arm and width/worker setting, followed by five shuffled complete blocks. Within each block, each pair runs adjacently and order alternates; Python `random.Random(739391)` freezes the order. Both 8/8 and 16/8 comparisons have their own regress observations. In total: 70 warm-ups and 350 measured fits in one Stata process.
- The repository command-clock plugin uses the monotonic Mach clock. Timing surrounds the entire quiet command, including estimation, covariance and Stata frontend/publication overhead. Generation, loading, processor changes and scientific export/checks are outside timing. No build, test or other estimation workload ran concurrently; occasional progress-log reads were lightweight. Ordinary desktop activity is retained, with no exclusions or retries.
- A warm-up harness assertion initially treated fereg rank/absorbed-DoF conventions as identical to regress. fereg reports slope rank K and df_a=1 for the implicit global intercept; regress reports full rank K+1. The check was corrected before any measured repetition; N, residual degrees of freedom and the full coefficient/covariance coordinates agree. Initial sandboxed Python wrappers stalled before work and were stopped; the actual 420-call schedule then completed directly once. No measured observations were removed or repeated.
- This is a direct installed-command comparison, not candidate qualification through `qualify.py`; no numerical algorithm, source, binary or routing policy changed. No external package environment is reused, so campaign package pins are not prerequisites.

## Numerical agreement

An independent SciPy 1.13.1 Householder QR of `[1, X64, y]` supplies nested-model coefficients, residual sums of squares and full IID covariance matrices through triangular solves. Every one of the 420 fits passed the existing campaign coefficient, covariance and objective tolerances. N and residual degrees of freedom matched exactly. Maximum absolute coefficient error was 3.25e-14, maximum absolute covariance error 1.59e-19, and maximum relative RSS error 9.79e-14. All scientific vectors repeated exactly at fixed worker counts; all worker counts agreed scientifically (maximum cross-worker coefficient difference 3.36e-14).

## fereg phase diagnostics at eight workers

The outer clock above is the comparison score. fereg runtime diagnostics do not cover the whole Stata command. Component medians below need not sum to the median outer time. The remainder includes frontend/posting work; these measurements alone do not identify its individual causes.

| RHS variables | Outer command | Input | Native | Runtime total |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 0.192 | 0.045 | 0.101 | 0.146 |
| 2 | 0.216 | 0.066 | 0.107 | 0.170 |
| 4 | 0.274 | 0.087 | 0.139 | 0.229 |
| 8 | 0.308 | 0.088 | 0.175 | 0.262 |
| 16 | 0.434 | 0.102 | 0.284 | 0.386 |
| 32 | 0.662 | 0.160 | 0.448 | 0.608 |
| 64 | 1.378 | 0.310 | 0.995 | 1.305 |

The intercept is internally an absorbed constant in fereg even without explicit FE categories. Both data transfer and native work remain material at eight workers, while the outer-command overhead is roughly 46–73 ms for these medians. Both transfer and native work are potential profiling targets; these phase summaries do not identify the routines that limit scaling.

These observations apply to well-conditioned unweighted Gaussian designs on this Mac. They do not establish Linux performance or behavior for ill-conditioned, weighted, robust or clustered regressions. Eight- versus sixteen-worker differences have no formal significance claim.

[Structured observations, medians, numerical references, hashes and reproduction sources](simple-regress-macos-20261101.json) retain all 420 timing records, including warm-ups and the frozen schedule. Generated data and logs are not retained.
