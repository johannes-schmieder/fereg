# fereg

`fereg` is a Rust-backed Stata estimator for linear models with many fixed
effects. It supports OLS and weighted least squares, multiple absorbed effects,
heterogeneous slopes, numeric factors and categorical interactions, singleton
removal, robust and multiway-clustered inference, and saved fixed-effect
outputs. Estimation runs in one native call and publishes results only after
the numerical and covariance checks pass.

The repository follows an incremental simplification plan for
[issue 28](https://github.com/johannes-schmieder/fereg/issues/28). Both `fereg`
and `feppml` default to `engine(next)`, copied from the complete working core.
A separate `engine(legacy)` plugin is frozen at `legacy-comparator-v1` for
comparison. Legacy is evidence, not the authority on scientific correctness;
next includes focused corrections while retaining the supported functionality.
Follow [MILESTONES.md](MILESTONES.md) and the [approved M0–M1 plan](docs/cleanup/PLAN.md).
Public native release remains subject to the existing licensing gate.

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
- `vce(unadjusted|robust|cluster clustervars)` and
  `dof(pairwise|firstpair|continuous|none)` for inference;
- `residuals(newvar)` and names inside `absorb()`, such as
  `absorb(fe_worker=worker fe_firm=firm)`, for saved outputs;
- `solver(auto|map|it|squarem|lsmr|cmg)` and
  `preconditioner(auto|off|diagonal|schwarz)` for the fixed-effect solver; and
- `engine(next|legacy)` to select the maintained or frozen native plugin; and
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

The beta requires Stata 18 or newer. Development packages bundle separate
next and frozen legacy plugins, with checksums, for each included platform.
Next retains `fereg_macos.plugin`, `fereg_linux.plugin` and
`fereg_windows.plugin`; comparator names insert `legacy_` after `fereg_`.
The package manifest identifies both engines and the shared frontend.

Selection precedence is explicit `engine()`, then the `FEREG_ENGINE` global,
then `next`. Invalid selections fail with `r(198)` and a missing selected
binary fails with `r(601)`; there is no automatic engine fallback. Results
record `e(fereg_engine)` or `e(feppml_engine)`. Legacy prints a note per call.

Release artifacts are not published. Use an internally qualified development
package. Licensed Windows Stata testing remains deferred; hosted native builds,
audits and mock SPI checks have a narrower scope.

After installation, run:

```stata
fereg, version
fereg, doctor verbose
fereg, selftest
fereg, version engine(legacy)
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
- [`docs/compatibility_matrix.md`](docs/compatibility_matrix.md) — implemented,
  deferred, and platform-specific behavior.
- [`MILESTONES.md`](MILESTONES.md) — incremental refactoring progress and checks.
- [`AGENTS.md`](AGENTS.md) — concise operating rules for repository work.
- [`docs/adr/`](docs/adr/) — accepted architectural and scientific decisions.

Development is consolidated on `main`. Historical receipts are evidence for
the exact source and artifact they name; they are not general release claims.

## Working paper

The manuscript, publication assets, and frozen paper evidence live in the
sibling [fereg-paper repository](../fereg-paper/README.md). Build it there with
`make paper`; the paper build does not require this implementation checkout.
Original benchmark execution scripts and qualification receipts remain here.
