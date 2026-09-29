# Optional PPML coefficient mean correction

This is the accepted implementation contract. Public availability and completed
evidence are recorded in STATUS.md; this document alone does not certify a feature.

## Statistical target

On the unchanged retained sample, let a be the effective fitting weight, mu
the accepted fitted mean including exposure/offset, x the complete identified
FE-plus-control design, and g the explicit independently sampled score unit.

```
S_g = sum_g a x (y-mu)       J_g = sum_g a mu x x'
J = sum_g J_g               K = sum_g S_g S_g'
V = J^-1 K J^-1
c_D = J^-1 sum_g J_g J^-1 S_g
c_C = (1/2) J^-1 sum_rows a mu x (x' V x)
beta_bc = beta_hat + c_D + c_C
```

There is no HC1, degrees-of-freedom, multiway-cluster, PSD, or coefficient-cap
adjustment. The public VCE is not substituted for this full-system calculation.
Both increments include all identified nuisance controls and both FE sets.

This is an unprojected second-order approximation for interior fits under
independent unit scores and the maintained observation-process assumptions.
Exposure syntax does not establish those assumptions. Small unit counts, high
leverage, weak support and sparse outcomes can limit the approximation. It
does not provide corrected confidence intervals, unbiased rates, or unbiased
variances of estimated effects. Split Branch and FEVC remain external.

## Independent units and FE support

`biasunit()` identifies independent **score units**, not deletion units: the
correction uses the accepted fit without leave-one-unit-out refits. Nesting
within an FE set is neither required nor a sufficient validity condition.
Choose units from the sampling/dependence assumptions, independently of the
VCE cluster option. Finer units are appropriate only if their independence is
substantively defensible.

Every retained level in **both** FE sets must contain at least two distinct
bias units. Otherwise both exact and randomized requests fail, even when
`biasfe()` is omitted, because both nuisance FE sets enter the full-system
correction. The diagnostic error reports single-unit level counts in absorb
order. This is a conservative restriction on supported configurations, not a
claim that every individual slope increment must be wrong. Ordinary estimation
without correction remains available.

For example, absorbing individual `workerid` while supplying
`biasunit(workerid)` fails. At the fitted solution the worker indicator has
score `sum_worker a*(y-mu)=0`. If its support lies entirely in that unit, the
corresponding coordinate of each unit score is zero. The empirical score
covariance can therefore miss estimation noise in that FE. Cross-coupling with
other coordinates can still generate nonzero corrections; zero increments are
not a necessary symptom. Absorbing worker **groups** with several independent
workers per group can pass this screen.

A simple analytic example makes the problem visible without a solver: with
k independent completed exponential durations per person and an individual
log-rate FE, the estimate is `log(k/sum T)`. Its exact mean bias is
`log(k)-digamma(k)>0`. If the entire person is treated as one score unit, its
fitted score is zero and both empirical contractions vanish in this diagonal
model. Independent duration units produce nonzero contractions, but that finer
choice is justified only under independent-duration sampling. This example
explains the limitation; it does not authorize splitting dependent spells.

The screen counts actual distinct IDs on the final retained sample, including
ordinary exclusions and singleton processing. It is invariant to relabelling
IDs and ignores frequency multiplicity as a source of independence. More rows
within one unit, more probes, or exact computation cannot repair unsupported
levels. Exact refers to evaluation of the empirical contraction, not exact
removal of statistical bias.

Passing is necessary for the supported implementation, not sufficient for
accurate bias reduction. Few units, concentrated information, graph bottlenecks,
and contrasts identified by a single unit can still undermine the approximation.
The implementation does not certify independence, test all graph/rank conditions,
or impose an arbitrary minimum such as 20 or 50 units.

## Requested interface and normalization

`biascorrection(exact|randomized)` requires `biasunit(numeric_variable)` and exactly
two categorical intercept FE sets with ordinary numeric controls. Existing
unweighted, frequency and probability fitting semantics apply. Absorbed slopes
and other FE topologies are unsupported. `biasprobes(256)`, `biasseed(739391)`
and `biasstream(0)` govern randomized mode; probes are fixed before computation.
Seed and stream are unsigned 32-bit integers. Randomized mode requires at least
two probes. Exact mode rejects explicitly supplied randomization options.

The feature never uses correction-ID missingness to select observations.
Missing IDs on retained rows are errors. Any outcome-dependent separation
deletion rejects the entire requested transaction. Other exclusions retain the
ordinary sample ledger; the correction describes that retained sample and
does not establish an unconditional target for outcome-related missingness.
At least two distinct retained score units per level of both FE sets are required.

Frequency multiplicity repeats rows within the supplied unit; it never creates
new independent units. Probability weights use the ordinary final-sample
normalization. Common weight scaling leaves these contractions invariant.

In each connected component, anchor the smallest canonical second-FE level to
zero and allocate the component constant to the first FE. Apply exactly this
linear map to raw effects, both increments and corrected effects. It depends
on declared support, not fitted Hessian centering. Ordinary saved effects keep
their existing normalization. Raw correction effects are recovered from the
accepted PPML predictor after removing the offset and unchanged finite
contribution. This avoids cancellation in the IRLS working response when
fitted means are tiny; the reconstruction certificate is unchanged.
Re-anchoring subtracts the chosen second-FE
anchor and adds it to the first FE in raw and correction channels alike;
disconnected branches require compatible support before common-anchor comparisons.

Publish `e(b_bias_raw)`, `e(b_bias_d)`, `e(b_bias_c)`, `e(b_bias_total)` and `e(b_bc)` in ordinary
coefficient order. Omitted positions and the normalized absorbed constant stay
zero. `biasfe(stub)` requests named raw, separate increment, total and corrected
observation contributions for both FE sets, plus numerical diagnostics. All
such variables are missing outside the ordinary final sample. Ordinary e(b),
e(V), likelihood, means and predict semantics remain ordinary-fit quantities.

## Computation and numerical uncertainty

Exact mode streams units and forms the two final weighted-linear RHSs without
retaining K, V or all scores. It is bounded by 10,000 units, one billion
observation-unit contributions, and ordinary memory admission.

Randomized mode uses one Rademacher sign per unit/probe, shared between both
increments. Philox4x32-10 maps the seed/stream key and stable numeric-unit/probe
counter to signs without touching Stata's RNG. Probe identities are independent
of row order, probe grouping, threads and numerical route.
The counter words are the low/high 32 bits of the numeric unit's IEEE-754
double representation and the low/high 32 bits of the zero-based probe index.
Negative zero is normalized to positive zero. The key words are seed and
stream; the low bit of the first output word maps to -1/+1. Use distinct
streams for full, A and B fits. Renumbering units changes numerical randomness.
Known-answer vectors are checked against [Random123](https://github.com/DEShawResearch/random123/blob/d8a0c25e49992685c0b9d91b553c1d04f81b6b4b/tests/kat_vectors)
(commit d8a0c25e49992685c0b9d91b553c1d04f81b6b4b, vector-file SHA-256
`aab5ebabf40003f63d6d87b24cbd2c8a02652e00cf8bad64226fd50586929183`). Floating-point results
obey existing route-specific reproducibility guarantees and numerical tolerances.

The point calculation requires B probe solves and two final solves. Numerical
variance uses up to eight predetermined independent groups of those same
probes, with group estimates combined using probe-count weights. This adds
at most fourteen final solves and is included in reported counts and timing.
For groups of sizes m_j and group increments d_j,c_j, estimate the covariance
of the overall means by

```
sum_j m_j (d_j-dbar)(c_j-cbar) / [B (groups-1)]
```

The corresponding squared deviations give the two numerical variances; total
variance includes twice their covariance. These are conditional numerical
variation diagnostics, not sampling standard errors. Exact mode has no probe
variation; solver residuals and reconstruction accuracy remain separate.

Preflight accounts for all ID, coefficient, RHS, probe, recovery and publication
storage. Use fallible reusable bounded workspaces and one admitted worker team.
The current implementation streams one RHS at a time, so its probe batch width
is one. It does not launch a second across-probe thread pool. The independent
explicit-design oracle also checks probe tiles of widths 1, 3, 8 and 32.
Every solve uses final a*mu weights and the accepted basis. The stricter inner
projection policy is unchanged; independent full-system normal-equation and
reconstruction checks use the requested PPML certificate tolerance. Interruption, invalid IDs, allocation,
rank or solve failures publish nothing and restore the prior public state.

Small supports with at most 256 full coordinates may cache a diagonally scaled,
pivoted information Cholesky factor using the shared finite-solve kernel. The
accepted basis, weights and component anchors remain fixed. Weak-factor guards
retain the sparse route. Explicit `memory(low)` uses the sparse route. Admission
reserves two bounded information banks before final support is known; no full
score covariance is constructed. Route 1 denotes sparse reuse and route 2 the
bounded cached factor. Each route certifies the full normal equations.

The diagnostic `max_fe_leverage_lb` is the maximum, across rows and both FE
indicators, of h_r divided by that level's information sum. It is a lower
bound on maximum full-system leverage, not an estimate of all hat diagonals.
`min_level_information` uses the effective final h=a*mu scale. The leverage
lower bound describes rows, not bias-unit information concentration. The
separate support summaries below use unit aggregates.

## Results and reproducible checks

`e(bias_mcse_d)`, `e(bias_mcse_c)`, `e(bias_cov_dc)` and
`e(bias_mcse_total)` follow the finite-coefficient stripes. Saved effects use
`stub_1_*` and `stub_2_*`, with suffixes `raw`, `d`, `c`, `total`, `bc`, `sed`,
`sec`, `covdc` and `setotal`. The exact-mode numerical SE/covariance channels
are zero. They do not include auxiliary-solver approximation error.
These numerical diagnostics apply to the reported normalization. Cross-level
covariances are not published, so they cannot generally be transformed into
SEs for a different FE anchor by shifting the reported SE channels.

The 21 named rows of `e(bias_diagnostics)` report method, units, probes, seed,
stream, independent probe groups, solve count, maximum normalized solve score,
maximum FE reconstruction error, correction seconds, workspace bound, maximum
absolute total increment, minimum/maximum fitted mean, minimum/maximum rows per
unit, numerical route, validity, certificate tolerance, minimum level
information and the maximum FE-leverage lower bound. `workspace_bytes` is a
conservative additional native workspace bound, not measured process RSS.
Validity 1 means the retained interior calculation passed its numerical
checks; it does not certify the observation-process assumptions or statistical
accuracy of the approximation. Failed requests return an error instead of a
partially valid correction.

`e(bias_support)` has two rows, `fe1` and `fe2`, in absorb order, and
11 columns:

| Column | Definition over retained levels of that FE |
| --- | --- |
| `levels` | Number of levels |
| `min_units`, `p25_units`, `median_units`, `p75_units`, `max_units` | Distribution of distinct bias-unit counts |
| `single_unit_levels`, `single_unit_fraction` | Count and fraction with one unit; both zero for successful requests |
| `min_info_units`, `median_info_units` | Minimum and median information concentration equivalents |
| `max_unit_info_share` | Largest unit share of any level's information |

Quantiles are unweighted across FE levels and use nearest rank (the smallest
ordered value whose empirical CDF reaches the requested fraction). For level
ell and unit g, define `I_ell,g = sum_(ell,g) a*mu`, unit shares
`p_ell,g = I_ell,g / sum_g I_ell,g`, and the information concentration equivalent
`1 / sum_g p_ell,g^2`. This equals the unit count under equal information and
approaches one under dominance by a single unit. It is not a certified effective
sample size. Inspect these summaries for small counts and information dominance;
there is no universal cutoff that establishes adequacy. Failed requests publish
no new results, so the error, rather than a partial matrix, supplies the number
of unsupported levels. `e(bias_validity)="interior-retained-sample"` and the
numeric `valid=1` denote numerical checks plus this necessary support screen,
not statistical validity of the selected units or correction.

Install the pinned test requirements in `scripts/requirements-ppml-bias.txt`.
The independent checks are `tests/python/test_ppml_bias_oracle.py`,
`scripts/ppml_bias_validate.py`, `scripts/ppml_bias_retained.py` and
`scripts/ppml_bias_reconstruction.py`. The Stata drivers take an exact candidate
plugin and its pre-build source manifest. The reconstruction regression retains
one synthetic duration draw and checks full/A/B exact and randomized outputs
against explicit-design oracles under four row orders. The
test-only Split Branch adapter is staged by `scripts/ppml_bias_mc.py`; the
campaign planner freezes disjoint fresh-seed jobs, and the analyzer preserves
failure denominators, reports all ten external moments, and separates joint
interior comparisons from all available results. Wage and FEVC operations
remain copied from the pinned external runtime. The report renderer
`scripts/ppml_bias_mc_report.py` requires a complete audit by default and labels
explicitly requested partial reports. It preserves the paired available and
joint-interior populations and includes exact/higher-probe sensitivity.

`scripts/ppml_bias_benchmark.py` compares matched Philox probes and full-system
contractions with current-solve and cached-factor Mata routes. Use
`--refit-reference` for end-to-end timings. The analyzer checks accuracy before
ranking timings. The established process monitor must pass for timing claims;
probe preparation, cold/warm calls, publication and process RSS have separate
measurements. Startup and Mata compilation are excluded from repeated-call
timings; process elapsed also includes all calls and exports, so it is not an
isolated startup measurement. Supply the frozen `--source-manifest` to the
benchmark analyzer to bind every receipt to the tested artifact.
These focused development checks do not qualify a release
or establish performance at unmeasured scales.

See the [development evidence](../benchmarks/reports/development/native-ppml-bias-2026-09-26/README.md)
for completed checks and their artifact identities. The sole current resume
point and any uncompleted gates remain in [STATUS.md](../STATUS.md).
