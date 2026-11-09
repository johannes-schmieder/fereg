# fereg user guide

Version 0.5.0 provides two commands: `fereg` for ordinary and weighted least
squares, and `feppml` for Poisson pseudo-maximum likelihood. Both allow models
with no fixed effects, one fixed effect, or several high-dimensional effects.
They validate the estimates and requested outputs before publishing results.

Start with the [installation guide](installation.md). The same package installs
both commands and their native plugin. Platform requirements and tested coverage
are listed there and in [validation](VALIDATION.md).

## Syntax

```stata
fereg depvar [indepvars] [if] [in] [weight] [, options]

fereg, selftest [threads(#) verbose]

feppml depvar [indepvars] [if] [in] [fweight|pweight] [, options]

fereg [, level(#) display_options]
feppml [, level(#) display_options]
```

Without a variable list, each command redisplays its last results. This replay
also serves `estimates replay`, `bootstrap:` and `jackknife:`; the `by:`
prefix is not supported.

`fereg` accepts analytic, frequency and probability weights. Importance
weights return `r(101)`. PPML weight support is described below.

### PPML models and options

`feppml` fits a log-link Poisson pseudo-maximum-likelihood mean with zero or
more absorbed effects. Robust inference is the default; one- through five-way
clustering is supported. The public model options are `absorb()`,
`keepsingletons`, `offset()` or `exposure()`, and
`separation(fe simplex ir|none)`. Saved outputs include `residuals()`, `xb()`,
`d()`, `eta()`/`xbd()`, `mu()`, `dropreason()`, and named absorbed channels.
Execution uses `solver(auto|lsmr|cg)`, `memory(auto|fast|low)`, `poolsize()`, `threads()`,
`deterministic`, `tolerance()`, `iterate()`, and `timeit`. Frequency and
probability weights are implemented; analytic and importance weights return
the documented `r(101)` rejection. After estimation, `predict` supports
`mu`, `eta`/`xbd`, `xb`, `d`, and the registered residual types when their
required retained state is available. See `help feppml` for the exact syntax,
drop codes, stored results, failure atomicity, and runnable examples.

The PPML output reports convergence iterations, observations, residual degrees
of freedom, the Wald chi-square test and p-value, pseudo-R², log
pseudolikelihood and deviance. It also shows cluster counts and the absorbed
categories, redundancy and coefficient counts. Replay uses the same summary;
bootstrap and jackknife replay retain Stata's inference display.

Leave `solver(auto)` in place unless you need a specific method. It selects
the canonical certified projection family, whose numerical route depends on the
design and available memory. For eligible two-effect categorical designs it
can reuse a CMG preconditioner across IRLS iterations. That internal
acceleration does not mean the command selected explicit `solver(cg)`.
`feppml` uses the `solver()` grammar of `fereg` (below) restricted to `auto`,
`lsmr` (Schwarz-preconditioned) and `cg` (CMG on the full graph); it has no
`solver(map)` and no `accelerate()`.

Explicit `solver(lsmr)` requires at least two categorical intercept effects;
`solver(cg)` supports exactly two or three. Both require the fast-memory
route and support the full PPML output interface. In the three-effect CMG
case, the two largest retained dimensions form the graph core; the remaining
dimension may have at most 64 non-base indicators. Absorbed slopes and explicit
`memory(low)` are outside these two solvers' supported scope. Unsupported
requests return `r(498)`; a supported workspace that cannot fit returns `r(909)`.

All routes retain the rank, convergence, final-Hessian and covariance checks.
A permitted numerical fallback is reported explicitly. `e(fe_route)` and its
parts (`e(fe_reduction)`, `e(fe_method)`, `e(fe_preconditioner)`, ...) describe
the final weighted least-squares solve, which always runs in the certified
canonical session; the requested `lsmr` or `cg` serves the ordinary IRLS
iterations. The rows of `e(fe_solve)` give each session's frames, retries,
fallbacks, iterations, timings and certificate maxima, and `e(fe_fallback)` and
`e(fe_fallback_reason)` the fallbacks. The
[installed help](../stata/feppml.sthlp) gives the complete contract.

### Optional PPML coefficient mean correction

For an interior model with two categorical intercept effects and ordinary
numeric controls, request a separate corrected coefficient vector and saved
effects:

```stata
feppml events x z, absorb(worker_group firm_group) exposure(duration) ///
    biascorrection(randomized) biasunit(worker) biasprobes(256) ///
    biasseed(739391) biasstream(0) biasfe(bc)
matrix list e(b_bc)
```

`e(b)`, `e(V)` and `predict` describe the ordinary accepted fit. The matrices
`e(b_bias_d)` and `e(b_bias_c)` hold the derivative-score and curvature
increments; `e(b_bias_total)` is their sum. `bc_1_bc` and `bc_2_bc` hold corrected
FE contributions. Their `raw`, `d`, `c`, `total`, `sed`, `sec`, `covdc`, and
`setotal` companion channels expose the shared normalization and conditional
numerical uncertainty. Numerical MCSEs are not sampling standard errors.

Use `biascorrection(exact)` for bounded reference calculations. Randomized
mode defaults to 256 fixed probes, seed 739391, and stream 0. Separate branches
should use different streams, such as 1 and 2, under a prespecified common FE
anchor. The RNG does not modify Stata's random state. Frequency copies remain
in their supplied independent unit; common pweight scaling changes no correction.

`biasunit()` defines independent score units, not deletion units. No PPML
refits are performed. Every retained level of both FE sets must contain at least
two distinct units, including when you request only corrected control coefficients.

| Configuration | Support interpretation |
| --- | --- |
| Worker-group FE; individual worker units | Can pass when every group has multiple workers and every firm level also has multiple units |
| Individual worker FE; worker units | Rejected: every worker FE has only one supporting unit |
| Individual worker FE; spell units | Can pass with multiple spells per FE, but requires defensible independence across spells |
| Units crossing FE categories | Allowed when every level has multiple units; nesting is not the criterion |

The check uses actual retained membership, so copied/relabelled IDs and repeated
frequency copies do not change the conclusion. Sample restrictions can make a
previously supported configuration fail. Do not split dependent units merely
to pass the check. More probes or exact calculation do not repair this problem.

Inspect `matrix list e(bias_support)` for each FE set's unit-count distribution,
minimum/median information concentration equivalents, and maximum unit information
share. Small counts or concentrated information merit substantive review; passing
the screen does not certify statistical adequacy. `valid=1` in
`e(bias_diagnostics)` records successful numerical checks and the necessary
support screen. Exact mode evaluates an approximate statistical correction.

A missing unit ID on a retained row or any outcome-dependent separation
deletion rejects the requested transaction. No rows are silently discarded and
no zero-coefficient fallback is substituted. The approximation does not correct
confidence intervals, predicted rates, or variances of estimated effects.
See [the full assumptions, normalization and diagnostics](ppml_bias_correction.md)
and the runnable example in `help feppml`. A
[test-only Split Branch example](../tests/stata/bias/split_branch_example.do)
shows separate branch streams; external moment estimation and FEVC remain the
caller's responsibility.

### OLS model and inference options

The following option tables describe `fereg`. PPML options and their narrower
solver eligibility rules are described above and in `help feppml`.

| Option | Default | Meaning |
|---|---|---|
| `absorb(terms)` | none | Absorb categorical effects, categorical interactions, or group-specific slopes. |
| `noconstant` | constant reported where identified | Suppress the normalized reported intercept. A model containing only slope-only absorbed terms has no constant automatically. |
| `keepsingletons` | remove | Retain observations that the fixed-point singleton rule would otherwise delete. |
| `residuals(newvar)` | none | Save estimation residuals. This also makes `predict, xbd`, `d`, and `residuals` available. |
| `vce(unadjusted)` | unadjusted | Use the IID covariance estimator. `ols` and `iid` are aliases. |
| `vce(robust)` | — | Use HC1 heteroskedasticity-robust inference. This is the default with probability weights. |
| `vce(cluster clustervars)` | — | Use one- or multiway-clustered inference. A cluster dimension may be an interaction such as `state#year`. |
| `dof(pairwise)` | pairwise | Use the reference-compatible pairwise absorbed-degree-of-freedom policy. |
| `dof(firstpair)` | — | Apply graph adjustment to the first pair of absorbed categorical effects. |
| `dof(continuous)` | — | Apply the absorbed-slope continuous-redundancy adjustment without categorical pairwise graph reductions. |
| `dof(none)` | — | Disable graph/continuous redundancy adjustments; absorbed coefficients still count toward residual degrees of freedom. |

If `vce()` is omitted, the command uses IID inference unless probability
weights are supplied. `vce(cluster ...)` must name at least one cluster
dimension. Cluster interactions use `#`, not `##`.

### OLS numerical and execution options

| Option | Default | Meaning |
|---|---|---|
| `solver(auto)` | `auto` | Select a certified projection for the design. Two to five categorical effects first eliminate the largest effect exactly (Jacobi CG, escalating to block-Jacobi); two effects otherwise use Schur PCG and may upgrade to CMG; eligible larger categorical designs use LSMR. |
| `solver(auto, precondition(jacobi))` | — | Automatic, but every Krylov stage stays Jacobi-preconditioned (no exact elimination, CMG or Schwarz). |
| `solver(map` [`, accelerate(irons-tuck\|squarem)`]`)` | — | Alternating projections, unaccelerated or with Irons–Tuck or SQUAREM extrapolation. |
| `solver(lsmr` [`, precondition(auto\|none\|jacobi\|schwarz)`]`)` | — | Modified LSMR on the full system; `auto` is Jacobi for small or simple designs and additive Schwarz otherwise. Explicit `lsmr` iterates to one hundredth of `tolerance()` (at least 1e-15, never less strictly than `tolerance()`) and certifies at `tolerance()`. |
| `solver(cg)` | — | CMG-preconditioned CG on the sparse AKM graph of the categorical intercept effects. |
| `solver(cg, reduce(eliminate)` [`cmgdegree() cmgsamples() cmgseed()`]`)` | — | Experimental CMG-preconditioned two-effect Schur CG. |
| `threads(#)` | `0` | Request a native thread team. Zero uses the native default. |
| `memory(auto|fast|low)` | `auto` | Choose automatically, retain dense workspaces, or use blockwise work and bounded scratch storage. |
| `poolsize(#)` | `10` | Set the regressor block width used by low-memory execution. |
| `tolerance(#)` | `1e-8` | Set the requested numerical tolerance; it must lie strictly between zero and one. |
| `iterate(#)` | `16000` | Set the fixed-effect solver iteration limit. |
| `deterministic` | off | Fix the native work partition and merge order for the same architecture and thread count. |
| `timeit` | off | Print timing, selected memory plan, allocation, scratch, RSS, and thread diagnostics. |
| `verbose(#)` | `0` | Record the requested verbosity level. `timeit` prints the execution diagnostics. |

`solver()` takes the method first and, after a comma, the reduction
(`reduce(auto|full|eliminate)`), the preconditioner
(`precondition(auto|none|jacobi|block|schwarz|cmg)`) and the MAP acceleration
(`accelerate(none|irons-tuck|squarem)`). Sub-choices left at `auto` resolve to
a supported formulation of the requested method. Any other combination returns
error 198 and names an alternative; nothing is silently replaced. For example,
`solver(cg, reduce(eliminate) precondition(jacobi))` is the route Auto runs
internally but is not yet available as an explicit request, and
`precondition()` with `map` or `accelerate()` with another method is rejected.

Automatic two-FE OLS checks an initial solution at the requested tolerance
and spends additional iterations on a tighter correction only if that check
fails. The correction shares the original iteration budget.

With `solver(auto)`, two to five categorical-intercept effects first try the
exact elimination of the largest effect. When it declines, two
categorical-intercept effects continue with diagonal-preconditioned Schur PCG. If eligible outcome or packed
regressor/outcome work has stalled at iteration 4, 8 or 12, or remains at
iteration 16, the structural rule builds a CMG preconditioner and applies it
on the worker team. The hierarchy is reused across subsequent blocks. Small or easy problems can finish without building it. A failed
optional construction leaves the existing recurrence in place; a numerical
failure can continue with certified MAP within the original budget.

Eligible designs with three or more categorical-intercept effects use LSMR
with reusable preparation. It starts with a diagonal preconditioner and builds
the additive Schwarz preconditioner only when the projected work is large.
With three unit-weight effects, a short MAP pilot or certified structural
shortcut may finish first. Every route must pass the requested FE-moment
certificate. One categorical effect uses direct demeaning. Slopes and explicit
requests retain their own supported routes. Ownership and
repeated-cell optimizations are automatic only for `solver(auto)` with a global
intercept and exactly two categorical-intercept effects. They do not change
the fitted model or require new options.

Automatic routes can differ at roughly the 1e-5 level in derived statistics
on weakly connected graphs, despite passing their requested FE-moment certificate.

CG-accelerated MAP, `solver(map, accelerate(cg))`, is disabled with error 198:
its residual certificate does not establish projection accuracy. Use
`solver(cg)` or `solver(lsmr)`.

`solver(cg)` selects the two absorbed effects with the most levels as a
bipartite graph core, reuses one CMG hierarchy across the fit, and handles
additional absorbed intercept effects through a dense Schur layer capped at 64
non-base columns. Zero- and one-effect models retain exact direct projection.
CMG does not support absorbed slopes. Every result is independently certified;
the existing CG solver polishes a small remaining projection error or is used
as a visible fallback if CMG setup or PCG fails (reported as
`map, accelerate(cg)`). Explicit CMG and automatic Schur-PCG upgrades are
different routes: they report `cg, reduce(full) precondition(cmg)` and
`cg, reduce(eliminate) precondition(cmg)`, respectively. Both memory modes include setup in the memory budget
and elapsed time. The final OLS residual is certified directly and skips graph
PCG when it is already within-transformed. Low-memory OLS instead reconstructs
raw residuals and performs the full CMG projection before final certification.

## Absorbed-term grammar

Terms inside `absorb()` are separated by spaces.

| Syntax | Interpretation |
|---|---|
| `worker firm` | Separate worker and firm intercept effects. |
| `firm#year` | One intercept effect for each observed firm-year cell. |
| `firm##c.time` | A firm intercept and firm-specific slope on `time`. |
| `firm#c.time` | A firm-specific slope on `time`, without a firm intercept. |
| `worker#c.(time tenure)` | Two slope-only channels within worker. |
| `worker##c.(time tenure)` | A worker intercept and two worker-specific slopes. |
| `fe_worker=worker` | Save one normalized worker-effect value for each estimation-sample observation. |

The categorical sources may be numeric or fixed-width strings. A categorical
interaction uses `#`. Categorical `##` expansion inside `absorb()` is not
supported: write the desired main and interaction effects explicitly. The
special `##c.` form is reserved for an absorbed intercept plus continuous
slope.

If a saved absorbed term contains slopes, the intercept uses the requested
base name and slope channels use `_Slope1`, `_Slope2`, and so on. A slope-only
term creates only the suffixed slope variables. Saved fixed-effect
normalizations are not unique; fitted absorbed contributions are the stable
identified object.

Saved names and `residuals()` must refer to new, distinct variables. Use names
inside `absorb()` to save OLS effects; `savefes()` is rejected.

## OLS samples, weights, and fixed effects

Stata evaluates `if` and `in`. The native estimator then applies outcome,
regressor, fixed-effect, cluster, and weight rules and removes singletons to a
fixed point unless `keepsingletons` is specified. `e(sample)` identifies the
final retained rows.

- Analytic weights may be rescaled without changing the estimates. Zero or
  missing values drop candidate rows; negative values are errors.
- Frequency weights must be strictly positive integers and behave as literal
  row multiplicities.
- Probability weights must be strictly positive. They require robust or
  clustered inference and select robust inference when `vce()` is omitted.
- Importance weights are not supported.

Ordinary right-hand-side variables may be continuous variables, numeric factor
main effects, all-categorical factor interactions, or Stata time-series
operators applied to continuous variables. Categorical-by-continuous and
continuous-by-continuous RHS interactions are not yet supported. Stata
materializes supported time-series terms so that panel gaps, leads, lags, and
differences retain Stata's semantics. Absorbed and cluster interactions are
constructed natively.

## OLS results and postestimation

The output follows the familiar `reghdfe` layout: sample size, model F-test and
p-value, R-squared, adjusted and within R-squared, and Root MSE appear above the
coefficient table. Clustered models also report counts for each cluster
dimension. Below the coefficients, the absorbed-DoF table lists categories,
redundant parameters and coefficient counts; `*` marks effects nested within
clusters and `?` marks potentially understated redundancy. These displays use
the existing stored results, including missing values for undefined statistics.
Replay after `bootstrap:` or `jackknife:` uses Stata's prefix inference display.

The command posts the standard `e(b)`, `e(V)`, `e(sample)`, sample size, rank,
residual and absorbed degrees of freedom, fit statistics, weight metadata,
cluster counts, singleton counts, and absorbed-effect metadata. The following
diagnostics are specific to `fereg`:

- `e(fe_solver)` is the normalized request, for example `cg, reduce(eliminate)`;
- `e(fe_route)` names the route used for outcome/regressor absorption in
  `solver()` syntax, with its parts `e(fe_reduction)`, `e(fe_method)`,
  `e(fe_preconditioner)`, `e(fe_block_solvers)` (block-Jacobi only) and
  `e(fe_acceleration)`, and the scalars `e(fe_iterations)`,
  `e(fe_escalations)` and `e(fe_fallback)`;
- `e(fe_residual_route)` and the other `e(fe_residual_*)` results describe the
  final residual projection;
- `e(fe_solve)` records both phases' iterations, escalations, fallback,
  warnings, pilot work, timings and certificates;
- `e(fereg_runtime)` records memory-plan, timing, scratch, RSS, and thread
  diagnostics;
- `e(fereg_certificate)` records independent scientific checks; and
- `e(fereg_status)`, `e(fereg_dof)`, `e(fereg_fe)`, `e(fereg_cluster)`, and
  related matrices expose the detailed result contract.

The lines above the coefficient table name the absorption route
(`Fixed-effect solver:`), mark an escalation, list any block local solves and
state that the accuracy check passed at the effective tolerance; results that
fail the check are never posted. `e(fereg_version)`
records the release that produced the results.

Solver results use the words of `solver()`: reduction `full`, `eliminate` or
`none`; method `map`, `cg`, `lsmr`, `direct` (an exact one-effect projection),
`derived` (a final residual certified from the absorption without a new solve)
or `none`; preconditioner `none`, `jacobi`, `block`, `schwarz` or `cmg`; MAP
acceleration `none`, `cg`, `irons-tuck` or `squarem`. The exact elimination
reports `cg, reduce(eliminate) precondition(jacobi)`, or `precondition(block)`
with `e(fe_block_solvers)` such as `firm=cmg year=dense` after an escalation;
the guarded two-FE Schur-PCG recurrence reports the same Jacobi route and
`precondition(cmg)` when it actually applied a CMG preconditioner. A MAP
fallback is reported as `map` with fallback set.

`e(fe_solve)` is a 2-by-12 matrix with rows `absorption` and `residual` and
columns `iterations escalations fallback warnings pilot_iterations
pilot_contraction setup_seconds solve_seconds certificate_absinf
certificate_relinf certificate_rel2 normal_equation`. For automatic
categorical-intercept fits without `residuals()` or named saved fixed effects,
the default memory mode certifies the within residual of the absorbed outcome
and regressors directly (method `derived`, zero iterations) instead of
absorbing it again. Low-memory absorption summarizes all blocks: a fallback
takes precedence; otherwise CMG and block-Jacobi use and escalations are
retained even if another block finished without them. A zero-iteration `map`
route can be an already-orthogonal input check, not an iterative MAP solve.
The reported route describes the certified result, not every attempted build.

```stata
display "Absorption: `e(fe_route)'"
display "Final residual: `e(fe_residual_route)'"
matrix list e(fe_solve)
```

Use `ereturn list` for the complete set. Developer tracing is not needed to
identify the reported route.

`predict newvar` and `predict newvar, xb` return the finite-regressor linear
prediction on the estimation sample. Models fit with `residuals(saved_residual)` additionally support:

```stata
predict fitted, xbd
predict absorbed, d
predict residual, residuals
```

`xbd` is the finite prediction plus the absorbed contribution. `d` is the
absorbed contribution alone. `test` uses the posted coefficient and covariance
matrices, and `estat summarize` reports estimation-sample summaries.

## OLS benchmarking

Benchmark estimates are meaningful only when the commands use the same data,
sample, fixed effects, singleton rule, weights, covariance estimator, degree-
of-freedom policy, tolerance, outputs, and effective thread allowance. Warm the
commands before timing, report repeated runs rather than a single best time,
and verify coefficient, sample, and objective parity before comparing speed.

Stata 19 extends `areg, absorb()` to multiple high-dimensional categorical
variables. Its `dfabsorb` option adjusts absorbed degrees of freedom for
pairwise collinearity; see Stata's
[HDFE overview](https://www.stata.com/new-in-stata/high-dimensional-fixed-effects/)
and [`areg` manual](https://www.stata.com/manuals/rareg.pdf). Example 2 in
`help fereg` simulates 250,000 observations without singletons and compares
the installed `fereg`, `reghdfe`, `greg`, and (on Stata 19 or newer) `areg`.
It reports coefficients, native standard errors, observations, and elapsed
estimation time in one table. It uses `etable` when available in Stata 17 or
newer, with built-in `estimates table` as the fallback. The example uses
`dfabsorb` and a tighter tolerance for `areg`; other inference settings are
command defaults. Each time is a single run including initial loading.
Data generation is excluded. This is a local illustration, not release
qualification.

Use the [maintained local benchmark](../bench/README.md) for descriptive development
measurements. Descriptive Linux comparisons with fixest, PyFixest, reghdfe,
ppmlhdfe and `areg` are in [Comparing fereg with fixest](fixest_benchmarks.md)
and, for OLS and PPML on Veneto's largest connected set, in
[Veneto comparisons](veneto_benchmarks.md). Earlier campaigns are retained
under dated headings. [Validation](VALIDATION.md)
records scientific evidence and timing limits. Both public commands use the
same sole native runtime.

## Examples and limitations

Type `help fereg` for twelve self-contained examples. Each estimation example
creates synthetic data and preserves/restores the caller's dataset. The
comparison skips unavailable commands and supports Stata 15 or newer without
installing a table add-on.

Version 0.5.0 does not support IV/GMM, group outcomes with individual
fixed effects, HAC or Driscoll--Kraay covariance estimators, other GLMs, or GPU
execution. When the dataset in memory has fewer than one million observations,
`feppml` temporarily sets Stata's `processors` to 1 to limit competition with
the native team. At one million observations or more it keeps the caller's
setting. The original setting is restored on success, error and Break.

`feppml` does not expose a model-based IID covariance, a preconditioner other
than its solver's own, DoF-policy controls, analytic/importance weights, or
developer injection.
Fixed-width string categorical sources are supported, but
`strL` sources are not. When `absorb()` contains an interaction or absorbed
slope, explicit RHS base operators such as `ib2.region` and `fvset` base
overrides are not yet supported by final-sample factor rebasing. See
[`SPEC.md`](SPEC.md) for the current public and
platform boundary.
