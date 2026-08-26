# fereg

`fereg` is a Rust-backed Stata estimator for linear models with many fixed
effects. It supports OLS and weighted least squares, multiple absorbed effects,
heterogeneous slopes, numeric factors and categorical interactions, singleton
removal, robust and multiway-clustered inference, and saved fixed-effect
outputs. Estimation runs in one native call and publishes results only after
the numerical and covariance checks pass.

The current source is an unreleased 0.5.0 beta. The public `fereg` OLS command
is active; `feppml` is installed only as a fail-closed `r(498)` placeholder.

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

## Command syntax

```stata
fereg depvar [indepvars] [if] [in] [weight], [options]
```

Common options are:

- `absorb(absvars)` for categorical fixed effects, interactions, and absorbed
  slopes;
- `vce(unadjusted|robust|cluster clustervars)` and
  `dof(pairwise|firstpair|continuous|none)` for inference;
- `residuals(newvar)` and names inside `absorb()`, such as
  `absorb(fe_worker=worker fe_firm=firm)`, for saved outputs;
- `solver(auto|map|cg|it|squarem|lsmr|cmg)` and
  `preconditioner(auto|off|diagonal|schwarz)` for the fixed-effect solver; and
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

## Installation and diagnostics

The beta requires Stata 18 or newer and one self-contained plugin matching the
host platform: `fereg_macos.plugin`, `fereg_linux.plugin`, or
`fereg_windows.plugin`. Release artifacts are not yet published. Install from a
qualified staged package rather than copying an arbitrary build into the
adopath.

After installation, run:

```stata
fereg, version
fereg, doctor verbose
fereg, selftest
```

The source tree uses the pinned Rust toolchain and these main development
checks:

```bash
./scripts/run_tests.sh
./scripts/build_macos.sh
./ci/run_stata_ci.sh quick
```

Stronger licensed profiles include `differential` and `full`. Their exact-SHA
acceptance and runner requirements are documented in
[`docs/development_workflow.md`](docs/development_workflow.md) and
[`STATA_CI_RUNNER.md`](STATA_CI_RUNNER.md).

## Documentation

- [`docs/user_guide.md`](docs/user_guide.md) — user-facing syntax, semantics,
  output, and benchmark guidance.
- [`stata/fereg.sthlp`](stata/fereg.sthlp) — installed Stata help and
  self-contained examples.
- [`docs/README.md`](docs/README.md) — documentation map.
- [`docs/compatibility_matrix.md`](docs/compatibility_matrix.md) — implemented,
  deferred, and platform-specific behavior.
- [`STATUS.md`](STATUS.md) — current qualification status and measured evidence.
- [`AGENTS.md`](AGENTS.md) and [`docs/adr/`](docs/adr/) — governing architecture
  and accepted decisions.

Development is consolidated on `main`. Historical receipts are evidence for
the exact source and artifact they name; they are not general release claims.
