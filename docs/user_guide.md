# fereg user guide

`fereg` fits linear models while absorbing one or more high-dimensional fixed
effects. Stata parses the command, factor variables, time-series operators, and
requested outputs. A single native call constructs the final sample, absorbs
the fixed effects, estimates the finite coefficients and covariance matrix,
checks the result, and writes the requested outputs.

This guide describes the public OLS and PPML commands in the unreleased 0.5.0
beta. `feppml` is active through the same guarded Rust-backed transaction and
shared certified-linear engine as `fereg`. Windows licensed-Stata testing and
formal cross-platform qualification are deferred; public availability permits
full user testing but is not a qualified release claim.

## Syntax

```stata
fereg depvar [indepvars] [if] [in] [weight], [options]

fereg, version
fereg, doctor [threads(#) verbose]
fereg, selftest [threads(#)]

feppml depvar [indepvars] [if] [in] [fweight|pweight], [options]
```

`fereg` accepts `aweight`, `fweight`, and `pweight`; it recognizes `iweight`
only to return the pinned `r(101)` rejection. PPML weight support is described
below.

### PPML surface

`feppml` fits a log-link Poisson pseudo-maximum-likelihood mean with zero or
more absorbed effects. Robust inference is the default; one- through five-way
clustering is supported. The public model options are `absorb()`,
`keepsingletons`, `offset()` or `exposure()`, and
`separation(fe simplex ir|none)`. Saved outputs include `residuals()`, `xb()`,
`d()`, `eta()`/`xbd()`, `mu()`, `dropreason()`, and named absorbed channels.
Execution uses `solver(auto|map|lsmr|cmg)`, `memory(auto|fast|low)`, `poolsize()`, `threads()`,
`deterministic`, `tolerance()`, `iterate()`, and `timeit`. Frequency and
probability weights are implemented; analytic and importance weights return
the documented `r(101)` rejection. After estimation, `predict` supports
`mu`, `eta`/`xbd`, `xb`, `d`, and the registered residual types when their
required retained state is available. See `help feppml` for the exact syntax,
drop codes, stored results, failure atomicity, and runnable examples.

`solver(auto)` selects LSMR for exactly two or three categorical intercept
effects when fast-memory execution and the complete specialized workspace fit
the native memory plan. Otherwise it selects MAP, including low-memory and
absorbed-slope designs. It does not select CMG. This is the development-admitted
policy in ADR 0029, not a formal qualification claim.
Explicit `solver(lsmr)` selects one native fused or joint LSMR implementation
for the fit, and `solver(cmg)` means the prepared fused-FWL CMG implementation.
Concrete LSMR preconditioner and reuse choices remain internal and are not
additional command options.
LSMR initially supports at least two categorical intercept effects and CMG
exactly two or three. Both retain the full PPML output surface, including
saved fixed effects recovered from the accepted canonical final-Hessian state,
and require fast-memory execution; a recognized request outside that envelope
returns `r(498)`. Numerical CMG nonconvergence falls back permanently and
visibly to MAP for ordinary frames. On admitted two- or three-effect LSMR/CMG
fits, including automatic LSMR, the initial
full-width basis-freezing projection and the strict and final-Hessian
projections use current-weight LSMR at every retained sample size,
only after independent canonical recertification. Canonical rank/status
authority remains unchanged. Numerical
failure visibly retries the untouched checkpoint through MAP, while other
errors remain fatal.
`e(feppml_solver)` and the accompanying macros distinguish requested,
selected, concrete, effective, checkpoint, and fallback identities.

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
| `solver(auto)` | `auto` | Select topology-qualified CMG before the bounded MAP pilot; otherwise select a certified direct, MAP, CG, or LSMR route. |
| `solver(map|it|squarem|lsmr|cmg)` | — | Request a specific fixed-effect solver. `cmg` is the sparse AKM graph route for categorical intercept effects. |
| `preconditioner(auto|off|diagonal|schwarz)` | `auto` with `auto`/`lsmr` | Select the LSMR preconditioner. `none` aliases `off`; `additive` aliases `schwarz`. |
| `threads(#)` | `0` | Request a native thread team. Zero uses the native default. |
| `memory(auto|fast|low)` | `auto` | Choose automatically, retain dense workspaces, or use blockwise work and bounded scratch storage. |
| `poolsize(#)` | `10` | Set the regressor block width used by low-memory execution. |
| `tolerance(#)` | `1e-8` | Set the requested numerical tolerance; it must lie strictly between zero and one. |
| `iterate(#)` | `16000` | Set the fixed-effect solver iteration limit. |
| `deterministic` | off | Fix the native work partition and merge order for the same architecture and thread count. |
| `timeit` | off | Print timing, selected memory plan, allocation, scratch, RSS, and thread diagnostics. |
| `verbose(#)` | `0` | Record the requested verbosity level. In the current beta, `timeit` is the printed execution diagnostic. |

`preconditioner()` is valid only with `solver(auto)` or `solver(lsmr)`.
Explicit MAP, Irons--Tuck, SQUAREM, and CMG requests therefore use
`preconditioner(off)`.

Explicit `solver(cg)` is disabled with error 198: its residual certificate
does not establish projection accuracy. Use `solver(lsmr)` or `solver(cmg)`.

`solver(cmg)` selects the two absorbed effects with the most levels as a
bipartite graph core, reuses one CMG hierarchy across the fit, and handles
additional absorbed intercept effects through a dense Schur layer capped at 64
non-base columns. Zero- and one-effect models retain exact direct projection.
CMG does not support absorbed slopes. Every result is independently certified;
the existing CG solver polishes a small remaining projection error or is used
as a visible fallback if CMG setup or PCG fails. `solver(auto)` selects CMG
before the MAP pilot only for the frozen favorable topology: two intercept-only
core effects, no nuisance Schur columns, at least eight observations per
highest-cardinality level, and at least a 10:1 level-count ratio between the
core effects. The route is available in `memory(fast)` and `memory(low)`. CMG
setup is included in memory preflight and timing; setup failure returns visibly
to the existing auto route, while a solve failure selects certified CG for the
rest of that fit. The final OLS residual is certified directly and skips graph
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

Saved names and `residuals()` must refer to new, distinct variables. The
legacy-style `savefes()` option is not a saving interface in this beta; name
each requested effect inside `absorb()` instead.

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

The command posts the standard `e(b)`, `e(V)`, `e(sample)`, sample size, rank,
residual and absorbed degrees of freedom, fit statistics, weight metadata,
cluster counts, singleton counts, and absorbed-effect metadata. The following
diagnostics are specific to `fereg`:

- `e(fereg_solver)` records the selected route, pilot, iterations, fallback,
  and convergence certificate;
- `e(fereg_runtime)` records memory-plan, timing, scratch, RSS, and thread
  diagnostics;
- `e(fereg_certificate)` records independent scientific checks; and
- `e(fereg_status)`, `e(fereg_dof)`, `e(fereg_fe)`, `e(fereg_cluster)`, and
  related matrices expose the detailed result contract.

Use `ereturn list` and `matrix list e(fereg_solver)` after estimation for the
complete set.

`predict newvar` and `predict newvar, xb` return the finite-regressor linear
prediction. Models fit with `residuals(saved_residual)` additionally support:

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
and [`areg` manual](https://www.stata.com/manuals/rareg.pdf). The benchmark in
`help fereg` uses a design without singletons, the same two absorbed effects,
IID inference, the default pairwise policy in `fereg` and `reghdfe`, and
`dfabsorb` in `areg`. It records three warm end-to-end repetitions in one Stata
session and checks coefficients and RSS before printing times. It is a
reproducible local comparison, not release qualification.

Use the [maintained local benchmark](../bench/README.md) for descriptive development
measurements. [Validation](VALIDATION.md) records scientific evidence and timing
limits. Both public commands use the same sole native runtime.

## Examples and limitations

Type `help fereg` for twelve self-contained examples. Each estimation example
creates synthetic data and preserves/restores the caller's dataset. The
benchmark additionally requires Stata 19 and an installed `reghdfe`.

The public 0.5.0 beta does not expose IV/GMM, group outcomes with individual
fixed effects, HAC or Driscoll--Kraay covariance estimators, other GLMs, or GPU
execution. During each call `feppml` temporarily sets Stata's `processors` to 1 so that
Stata's own workers do not compete with the native team; the caller's setting
is restored on success, error and Break. `feppml` does not expose a model-based IID covariance,
preconditioner or DoF-policy controls, analytic/importance weights, or
developer injection.
Fixed-width string categorical sources are supported, but
`strL` sources are not. When `absorb()` contains an interaction or absorbed
slope, explicit RHS base operators such as `ib2.region` and `fvset` base
overrides are not yet supported by final-sample factor rebasing. See
[`SPEC.md`](SPEC.md) for the current public and
platform boundary.
