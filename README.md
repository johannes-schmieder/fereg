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

## Install in Stata

Install the current package from `main`, using the same community-contributed
[`github` installer](https://github.com/haghish/github) as `fevc`:

```stata
* Install the github command once, if needed
net install github, from("https://haghish.github.io/github/")
github install johannes-schmieder/fereg
fereg, selftest
```

Or install directly without the `github` command:

```stata
net install fereg, from("https://raw.githubusercontent.com/johannes-schmieder/fereg/main/") replace
```

Both routes install `fereg`, `feppml`, help files and the current native plugin
from `main`, which carries the
[v0.5.0-beta.9](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.9)
runtime. The beta requires Stata 18 or newer and includes native binaries
for Linux x86-64, Windows x86-64 and macOS (Apple Silicon/Intel universal).
No Rust, Python or extra Stata packages are needed to use the binary package.
Exit Stata before updating, then install in a fresh session so the previous
plugin is unloaded.

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

Descriptive Linux comparisons on whole 28-core SCC nodes are in
[Comparing fereg with fixest](docs/fixest_benchmarks.md) (50 synthetic OLS and
PPML configurations up to ten million rows) and
[Veneto AKM comparisons](docs/veneto_benchmarks.md) (real worker-firm panels,
with and without saved effects). Speed ratios count only when results agree,
exactly or up to how absorbed degrees of freedom are counted. In the beta.8
campaign `fereg` was faster than `reghdfe`, `ppmlhdfe`, `areg` and default and
strict `fixest` in every agreeing configuration, and faster than
FixedEffectModels.jl in the AKM designs with 4 or 16 controls. PyFixest's `within` demeaner was faster
on sparse and dense three-FE designs, dense two-FE PPML and one-FE designs;
FixedEffectModels.jl was faster with one FE, dense three-FE designs and 64
controls. On the real long Veneto panel `fereg` was faster than every comparator
at one to sixteen cores, and when saving worker and firm effects every comparator
timed out or failed a scientific check. In sparse three-FE OLS at ten million
rows `fereg` did not finish within the 600-second limit. These are fixed-panel
descriptive results, not universal rankings.

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
