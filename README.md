# fereg: Fast High Dimensional Fixed Effects Regressions in Stata

`fereg` is a Rust-backed Stata estimator for linear models with many fixed
effects. It supports OLS and weighted least squares, multiple absorbed effects,
heterogeneous slopes, numeric factors and categorical interactions, singleton
removal, robust and multiway-clustered inference, and saved fixed-effect
outputs. Estimation runs in one native call and publishes results only after
the numerical and covariance checks pass.

`fereg` and `feppml` share one maintained native runtime. Scientific
requirements live in [SPEC](docs/SPEC.md), with current validation scope in
[VALIDATION](docs/VALIDATION.md). The source is licensed under GPL-3.0-only, with third-party terms preserved in
[THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES.md).

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

We also acknowledge [reghdfe](https://github.com/sergiocorreia/reghdfe),
[ppmlhdfe](https://github.com/sergiocorreia/ppmlhdfe), and
[PyFixest](https://github.com/py-econometrics/pyfixest) as development and
compatibility references. See [Selected references](#selected-references)
for the methodological sources and
[Third-party notices](THIRD_PARTY_NOTICES.md) for licenses and code provenance.

## Install in Stata

Install the current package from `main` in a fresh Stata session:

```stata
net install fereg, from("https://raw.githubusercontent.com/johannes-schmieder/fereg/main/") replace
fereg, selftest
```

This installs `fereg`, `feppml`, help files and the current native plugin
from `main`, which carries the
[v0.5.0-beta.12](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.12)
runtime. The beta requires Stata 15 or newer and includes native binaries
for Linux x86-64, Windows x86-64 and macOS (Apple Silicon/Intel universal).
No Rust, Python or extra Stata packages are needed to use the binary package.
Exit Stata before updating, then install in a fresh session so the previous
plugin is unloaded.

The community [`github`](https://github.com/haghish/github) helper also works:

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
prediction, low-memory execution, and a benchmark against `reghdfe` and Stata
19's HDFE `areg`.

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

Beta.11 retains the reviewed upstream-dependency migration. Its original sparse-three-FE
Mac and near-confounded-three-FE Linux regressions are recovered. The accepted
eight-/sixteen-FE settings remain about 8–11% slower on Linux and 11–14% slower
on Mac; several other settings are inconclusive. The [validation report](docs/VALIDATION.md#upstream-migration-candidate)
retains those results alongside the gains and numerical checks.

Descriptive Linux comparisons on whole 28-core SCC nodes are in
[Comparing fereg with fixest](docs/fixest_benchmarks.md) (synthetic OLS and
PPML configurations up to ten million rows, including clustered-mobility and
gravity designs, and the Stata-comparison cells on an Ice Lake node) and
[Veneto AKM comparisons](docs/veneto_benchmarks.md) (the real long worker-firm
panel on the leave-one-out and largest connected sets). Speed ratios count only
when results agree, exactly or up to how absorbed degrees of freedom are
counted. In the beta.11 campaign all 357 native fits outside two superseded
cells passed; those cells' input left one singleton firm-year and were rerun on
a corrected input. `fereg` was faster than `reghdfe`, `ppmlhdfe` and `areg` in
every agreeing configuration and than default `fixest` in all but PPML with four
controls. PyFixest and FixedEffectModels.jl were faster in several designs. On
both Veneto samples `fereg` was faster than every agreeing comparator at one to
sixteen workers. Sparse three-FE OLS at ten million rows completed in a median
52.21 seconds with an 11.06 GiB kernel peak. The comparisons are within-host
descriptive results, not universal rankings or absolute speedups over beta.10
on different hosts.

## Command syntax

```stata
fereg depvar [indepvars] [if] [in] [weight], [options]
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
weights use robust inference by default; importance weights are rejected to
match the pinned `reghdfe` contract. Unless `keepsingletons` is specified,
singleton observations are removed to a fixed point.

## Prediction and postestimation

After estimation, `predict` supports `xb`. If the model was fit with
`residuals(newvar)`, it also supports `xbd`, `d`, and `residuals`. Standard
`test` and `estat summarize` are available. See
[`stata/fereg_postestimation.sthlp`](stata/fereg_postestimation.sthlp) for the
exact postestimation contract.

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

## Working paper

The manuscript, publication assets, and frozen paper evidence live in the
sibling `fereg-paper` repository. Build it there with
`make paper`; the paper build does not require this implementation checkout.
The maintained benchmark tooling, qualification harness and SCC comparison
campaigns are in [bench](bench/README.md).

## Selected references

These sources describe the numerical and econometric methods used by fereg
and its dependencies.

- **Fischer, Alexander, and Kristof Schröder (2026).**
  [*Graph Preconditioning for High-Dimensional Fixed Effects Regression*](https://github.com/py-econometrics/within-paper).
  Working paper. The pairwise graph preconditioner implemented in `within`.
- **Koutis, Ioannis, Gary L. Miller, and David Tolliver (2011).**
  [*Combinatorial Preconditioners and Multilevel Solvers for Problems in Computer Vision and Image Processing*](https://doi.org/10.1016/j.cviu.2011.05.013).
  *Computer Vision and Image Understanding* 115(12): 1638–1646.
  The combinatorial multigrid method underlying the CMG backend.
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
