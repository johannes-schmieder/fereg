# fereg

`fereg` is a Rust-backed Stata estimator for linear models with many fixed
effects. It supports OLS and weighted least squares, multiple absorbed effects,
heterogeneous slopes, numeric factors and categorical interactions, singleton
removal, robust and multiway-clustered inference, and saved fixed-effect
outputs. Estimation runs in one native call and publishes results only after
the numerical and covariance checks pass.

`fereg` and `feppml` share one maintained native runtime. The former next engine
is now the only engine; there is no engine selector or fallback. Scientific
requirements live in [SPEC](docs/SPEC.md), with current validation scope in
[VALIDATION](docs/VALIDATION.md). Public native distribution remains subject to
the [licensing decision](docs/licensing.md).

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
prediction contracts, separation methods, failure classes, and six
self-contained examples. A simulated bilateral-trade comparison against
`ppmlhdfe` is in `tests/manual/simple_PPML.do`.

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

The package requires Stata 18 or newer and one platform plugin:
`fereg_macos.plugin`, `fereg_linux.plugin`, or `fereg_windows.plugin`.
A missing plugin fails with `r(601)`. The removed `engine()` option fails with
`r(198)`; the old `FEREG_ENGINE` global has no effect.

Use an internally qualified development package. Native release assets are not
published. Current qualification is Mac-only; Windows/Linux build support does
not imply fresh licensed-platform qualification.

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
[`ci/README.md`](ci/README.md).

## Documentation

- [`docs/user_guide.md`](docs/user_guide.md) — user-facing syntax, semantics,
  output, and benchmark guidance.
- [`stata/fereg.sthlp`](stata/fereg.sthlp) — installed Stata help and
  self-contained examples.
- [`docs/README.md`](docs/README.md) — documentation map.
- [`AGENTS.md`](AGENTS.md) — concise operating rules for repository work.

Development is consolidated on `main`. Historical receipts are evidence for
the exact source and artifact they name; they are not general release claims.

## Working paper

The manuscript, publication assets, and frozen paper evidence live in the
sibling [fereg-paper repository](../fereg-paper/README.md). Build it there with
`make paper`; the paper build does not require this implementation checkout.
Historical programs and receipts remain in Git history; the maintained local benchmark is in [bench](bench/README.md).
