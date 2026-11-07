# fereg: Fast high-dimensional fixed-effects regressions in Stata

`fereg` fits linear models by ordinary or weighted least squares. `feppml`
fits Poisson pseudo-maximum-likelihood models, including nonnegative outcomes
with zeros. Both support multiple high-dimensional fixed effects, robust and
multiway-clustered inference, and saved fitted values and fixed effects.
They share one native Rust runtime and require no separate language runtime.

Use the [installation guide](docs/installation.md) to get started and the
[user guide](docs/user_guide.md) for syntax, examples and limitations.
The source is licensed under MIT; third-party terms are preserved in
[Third-party notices](THIRD_PARTY_NOTICES.md).

## Install in Stata

Install stable version 0.5.2 in a fresh Stata session:

```stata
net install fereg, from("https://raw.githubusercontent.com/johannes-schmieder/fereg/v0.5.2/") replace
fereg, selftest
```

This installs both commands, their help files and the native plugin from the
[0.5.2 release](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.2).
It requires Stata 15 or newer. Linux x86-64 needs glibc 2.28 or newer;
Windows x86-64 and universal macOS binaries are also included. Mac Stata
qualification uses Apple Silicon; native Intel-Mac testing remains outstanding.
Windows x86-64 Stata/MP 19 GUI supports eligible parallel numeric input. Break
during Windows loading may be delayed or unavailable; other Windows hosts use
serial loading. No compiler or extra Stata package is needed. See the
[validation record](docs/VALIDATION.md) for platform qualification.

Exit Stata before updating so that the previous plugin is unloaded. To follow
the current development package, replace `v0.5.2` in the URL with `main`.

The community [`github`](https://github.com/haghish/github) helper installs the
current package from `main`:

```stata
net install github, from("https://haghish.github.io/github/")
github install johannes-schmieder/fereg
```

See the [installation guide](docs/installation.md) for versioned and offline
installation, source builds and troubleshooting. Changes by release are in the
[changelog](CHANGELOG.md). Native build identity, checksums and
corresponding-source links are retained in [dist](dist/README.md).

## Quick start

The usual two-way fixed-effect specification is

```stata
fereg outcome treatment controls, absorb(worker firm) vce(cluster firm)
```

This complete example generates its own data:

```stata
clear
set seed 20260826
set obs 600

generate long worker = ceil(_n/6)
bysort worker: generate byte period = _n
generate long firm = mod(worker + 7*period, 30) + 1
generate double x = rnormal()

bysort worker: generate double worker_fe = rnormal() if _n == 1
bysort worker: replace worker_fe = worker_fe[1]
bysort firm: generate double firm_fe = .7*rnormal() if _n == 1
bysort firm: replace firm_fe = firm_fe[1]
generate double y = 1 + .8*x + worker_fe + firm_fe + rnormal()

fereg y x, absorb(worker firm) vce(cluster firm)
```

Type `help fereg` in Stata for the complete syntax and twelve copy-ready
examples, including categorical factor-variable models, string and interacted
fixed effects, absorbed slopes, weights, multiway clustering, saved effects,
prediction, low-memory execution, and a coefficient/time comparison with
installed `reghdfe`, `greg`, and Stata 19 `areg`. The comparison uses `etable`
when available and `estimates table` on older Stata releases.

For PPML with high-dimensional fixed effects, use for example:

```stata
feppml trade agreement log_distance, absorb(exporter_year importer_year) ///
    vce(cluster pair_id)
```

Type `help feppml` for the complete implemented PPML syntax, saved-output and
prediction contracts, separation methods, failure classes, and seven
self-contained examples. A simulated bilateral-trade comparison against
`ppmlhdfe` is in `tests/manual/simple_PPML.do`.

## Performance

Version 0.5.0 promotes the qualified beta.15 runtime without changing its native
binaries. The [November 2–3 SCC campaign](bench/results/scc-20261102-beta15.md)
reports synthetic OLS and PPML, Veneto's largest connected set, and separate
Skylake, Ice Lake and Granite Rapids comparisons. Independent native science,
fixed-worker repeatability and cross-worker agreement passed. Comparator
failures and numerical discrepancies remain visible; speed ratios are
meaningful only for scientifically agreeing fits paired on the same host.

The campaign covers different designs, sizes, worker counts and inference
settings; rankings vary across them. It does not isolate the software effect
from hardware changes relative to earlier campaigns. The
[validation report](docs/VALIDATION.md) also retains accepted regressions,
including the Linux no-FE 16-regressor, one-worker slowdown of 7.3% against
beta.14.

A separate [controlled Mac loading test](bench/results/macos-20261103-accessor-ablation-50rhs.md)
uses the same binary and 20 workers with 20 million rows, 50 controls and one
fixed effect. Median loading time falls from 4.60 to 0.96 seconds with parallel
input; whole-command time falls from 10.66 to 6.98 seconds. These are measurements
on the tested Apple M2 Ultra, not a general speed guarantee.

## OLS command syntax

```stata
fereg depvar [indepvars] [if] [in] [weight] [, options]
```

Common options are:

- `absorb(absvars)` for categorical fixed effects, interactions, and absorbed
  slopes;
- `vce(unadjusted|robust|cluster clustervars)` (`iid` and `ols` are
  aliases of `unadjusted`) and
  `dof(pairwise|firstpair|continuous|none)` for inference;
- `residuals(newvar)` and names inside `absorb()`, such as
  `absorb(fe_worker=worker fe_firm=firm)`, for saved outputs;
- `solver(auto|map|it|squarem|lsmr|cmg)` and
  `preconditioner(auto|off|diagonal|schwarz)` for the fixed-effect solver, plus
  the experimental two-FE `solver(cmgschur)`; and
- `memory(auto|fast|low)`, `poolsize(#)`, `threads(#)`, `deterministic`, and
  `timeit` for execution control and diagnostics.

Absorbed-term notation follows these rules:

| Term | Meaning |
|---|---|
| `firm worker` | Two categorical fixed effects |
| `firm#year` | One fixed effect for each firm-year cell |
| `firm##c.time` | Firm intercepts and firm-specific time slopes |
| `firm#c.time` | Firm-specific time slopes without firm intercepts |
| `worker#c.(time tenure)` | Two slope-only channels by worker |
| `fe_firm=firm` | Save a normalized firm effect in `fe_firm` |

The estimator accepts analytic, frequency, and probability weights. Probability
weights use robust inference by default; importance weights are not supported. Unless `keepsingletons` is specified,
singleton observations are removed to a fixed point.

## Prediction and postestimation

After `fereg`, `predict` supports `xb` on the estimation sample. If the model
was fit with `residuals(newvar)`, it also supports `xbd`, `d`, and `residuals`. Standard
`test` and `estat summarize` are available. See
[`stata/fereg_postestimation.sthlp`](stata/fereg_postestimation.sthlp) for the
exact OLS postestimation contract. After `feppml`, use `help feppml` for
`mu`, `eta`, `xb`, absorbed contributions and residuals; predictions involving
fixed effects require retained fitted-state information.

## Diagnostics and development

Normal OLS output names the fixed-effect solver and final residual-check method.
After a fit, `e(fereg_solver_used)` and `e(fereg_solver_residual)` contain these
names; `matrix list e(fereg_solver)` shows iterations, fallback and certificates.
For two-effect Auto fits, `cg` identifies Schur PCG without multigrid and
`cmgschur` identifies CMG-preconditioned Schur PCG. Auto builds multigrid only
when its structural rule admits an upgrade; easy fits may finish without it.
See [solver options and results](docs/user_guide.md#ols-numerical-and-execution-options).

`fereg, selftest` reports versions, checks ado/plugin compatibility and runs
bounded installation checks. Use `fereg, selftest verbose` for build, host and
individual check details. A missing plugin fails with `r(601)`; incompatible ado
and plugin versions fail with `r(499)`. See the
[installation guide](docs/installation.md) for fixes.

The source tree uses the pinned Rust toolchain and these development checks:

```bash
./scripts/run_tests.sh
./scripts/build_macos.sh
./ci/run_stata_ci.sh quick
```

Licensed profiles and platform limitations are documented in
[development workflow](docs/development_workflow.md), [CI](ci/README.md), and
[validation](docs/VALIDATION.md). GitHub testing remains paused.

## Documentation

- [`docs/installation.md`](docs/installation.md) — installation and source builds.
- [`docs/user_guide.md`](docs/user_guide.md) — user-facing syntax, semantics,
  output, and benchmark guidance.
- [`docs/VALIDATION.md`](docs/VALIDATION.md) — accepted evidence, package
  checks and limits.
- [`docs/fixest_benchmarks.md`](docs/fixest_benchmarks.md) and
  [`docs/veneto_benchmarks.md`](docs/veneto_benchmarks.md) — package
  comparisons.
- [`stata/fereg.sthlp`](stata/fereg.sthlp) — installed Stata help and
  self-contained examples.
- [`docs/README.md`](docs/README.md) — documentation map.
- [`AGENTS.md`](AGENTS.md) — concise operating rules for repository work.

Development is consolidated on `main`.

## Methods and evidence

The companion working paper is maintained separately. Public documentation
of the methods and numerical checks is available in the [specification](docs/SPEC.md)
and [validation record](docs/VALIDATION.md). The maintained benchmark tools,
qualification harness and cluster comparison results are in [bench](bench/README.md).

## Upstream software and acknowledgments

fereg builds on established open-source numerical software. Its LSMR solver
and pairwise Schwarz preconditioner use a pinned, adapted
[fork](https://github.com/johannes-schmieder/within) of
[within](https://github.com/py-econometrics/within), developed by Alexander
Fischer, Kristof Schröder, and contributors at the appliedAI Institute for
Europe and in the PyFixest community. Their
[graph-preconditioning paper](https://github.com/py-econometrics/within-paper)
describes the approach. The fork adapts data access, resumable iterations,
allocation, reproducible reductions, and cancellation for fereg; the upstream
LSMR and preconditioning algorithms are shared numerical building blocks.

The CMG backend uses a [Rust port](https://github.com/johannes-schmieder/CMG)
of Koutis and Miller's
[original MATLAB/C implementation](https://github.com/ikoutis/cmg-solver),
based on the combinatorial multigrid method of
[Koutis, Miller, and Tolliver (2011)](https://doi.org/10.1016/j.cviu.2011.05.013).
fereg integrates these components with Stata estimation, automatic solver
selection, and independent convergence checks.

Parallel copying from Stata has precedent in David Roodman's
[`julia` package](https://github.com/droodman/julia.ado), described in his
[2025 Stata Journal article](https://doi.org/10.1177/1536867X251341105).
Its C++ plugin copies data across columns concurrently. fereg uses a qualified
read-only numeric loading phase; this precedent does not establish thread
safety for arbitrary Stata plugin operations.

We also acknowledge [reghdfe](https://github.com/sergiocorreia/reghdfe),
[ppmlhdfe](https://github.com/sergiocorreia/ppmlhdfe), and
[PyFixest](https://github.com/py-econometrics/pyfixest) as development and
compatibility references. See [Selected references](#selected-references)
for the methodological sources and
[Third-party notices](THIRD_PARTY_NOTICES.md) for licenses and code provenance.

## Selected references

These sources describe the numerical and econometric methods used by fereg
and its dependencies, along with prior work on parallel data transfer.

- **Fischer, Alexander, and Kristof Schröder (2026).**
  [*Graph Preconditioning for High-Dimensional Fixed Effects Regression*](https://github.com/py-econometrics/within-paper).
  Working paper. The pairwise graph preconditioner implemented in `within`.
- **Koutis, Ioannis, Gary L. Miller, and David Tolliver (2011).**
  [*Combinatorial Preconditioners and Multilevel Solvers for Problems in Computer Vision and Image Processing*](https://doi.org/10.1016/j.cviu.2011.05.013).
  *Computer Vision and Image Understanding* 115(12): 1638–1646.
  The combinatorial multigrid method underlying the CMG backend.
- **Roodman, David (2025).**
  [*Julia as a Universal Platform for Statistical Software Development*](https://doi.org/10.1177/1536867X251341105).
  *Stata Journal* 25(2): 255–284. Parallel Stata-to-native data copying in `julia`.
- **Fong, David Chin-Lung, and Michael A. Saunders (2011).**
  [*LSMR: An Iterative Algorithm for Sparse Least-Squares Problems*](https://doi.org/10.1137/10079687X).
  *SIAM Journal on Scientific Computing* 33(5): 2950–2971.
  The LSMR iteration used for fixed-effect absorption through `within`.
- **Gao, Yuan, Rasmus Kyng, and Daniel A. Spielman (2026).**
  [*AC(k): Robust Solution of Laplacian Equations by Randomized Approximate Cholesky Factorization*](https://doi.org/10.1137/24M1673577).
  *SIAM Journal on Scientific Computing* 48(3): A1643–A1684.
  Approximate Cholesky factorization and randomized elimination methods used
  in graph preconditioning.
- **Somaini, Paulo, and Frank A. Wolak (2016).**
  [*An Algorithm to Estimate the Two-Way Fixed Effects Model*](https://doi.org/10.1515/jem-2014-0008).
  *Journal of Econometric Methods* 5(1): 143–152.
  The block-elimination reduction for two-way fixed effects.
- **Guimarães, Paulo, and Pedro Portugal (2010).**
  [*A Simple Feasible Procedure to Fit Models with High-Dimensional Fixed Effects*](https://doi.org/10.1177/1536867X1101000406).
  *Stata Journal* 10(4): 628–649.
  Iterative absorption of high-dimensional fixed effects.
- **Gaure, Simen (2013).**
  [*OLS with Multiple High Dimensional Category Variables*](https://doi.org/10.1016/j.csda.2013.03.024).
  *Computational Statistics & Data Analysis* 66: 8–18.
  Alternating-projection methods for multiple fixed effects.
- **Correia, Sergio (2016).**
  [*A Feasible Estimator for Linear Models with Multi-Way Fixed Effects*](https://www.scorreia.com/research/hdfe.pdf).
  Working paper. Graph formulations and computational methods for multi-way
  fixed-effect estimation.
- **Correia, Sergio, Paulo Guimarães, and Thomas Zylkin (2020).**
  [*Fast Poisson Estimation with High-Dimensional Fixed Effects*](https://doi.org/10.1177/1536867X20909691).
  *Stata Journal* 20(1): 95–115.
  PPML with high-dimensional fixed effects and the `ppmlhdfe` reference implementation.
- **Irons, Bruce M., and Robert C. Tuck (1969).**
  [*A Version of the Aitken Accelerator for Computer Iteration*](https://doi.org/10.1002/nme.1620010306).
  *International Journal for Numerical Methods in Engineering* 1(3): 275–277.
  The Irons–Tuck acceleration available through `solver(it)`.
- **Varadhan, Ravi, and Christophe Roland (2008).**
  [*Simple and Globally Convergent Methods for Accelerating the Convergence of Any EM Algorithm*](https://doi.org/10.1111/j.1467-9469.2007.00585.x).
  *Scandinavian Journal of Statistics* 35(2): 335–353.
  The SQUAREM acceleration available through `solver(squarem)`.
