# fereg behavioral specification

This document governs scientific results and failure behavior of the single
fereg runtime. [Architecture](ARCHITECTURE.md) describes current ownership;
[development](development_workflow.md) describes validation. Pinned external
packages are compatibility references; independent mathematics decides correctness.

In this document N is retained physical rows, N_eff is effective observations,
D is the complete absorbed design, X the finite design, and W positive fitting
weights. All row identities refer to the original dataset. "Tests" names
existing executable checks to carry forward, not unexecuted replacement claims.
Native test names below are relative to `crates/fereg-core` or
`crates/fereg-testkit`; Stata names are relative to `tests/stata`.

## 1. Public syntax and options

Both commands accept `depvar [indepvars] [if] [in] [weight], [options]`.
The dependent variable is plain numeric; the finite list can be empty.
Stata 18 is the minimum host. Installed diagnostic calls remain
`fereg, version`, `fereg, doctor [threads(#) verbose]`, and
`fereg, selftest [threads(#)]`; developer actions are not public commands.

| Option | OLS (`fereg`) | PPML (`feppml`) |
|---|---|---|
| `absorb(terms)`; named saved channels | Yes | Yes |
| `keepsingletons` | Yes; default prune | Yes; default prune |
| Weights | none, aw, fw, pw | none, fw, pw |
| `vce()` | unadjusted/iid/ols, robust, cluster | robust, unadjusted mapped to robust, cluster |
| `dof()` | pairwise (default), firstpair, continuous, none | No public DoF-policy override |
| `noconstant` | Suppress reported constant | Unsupported; structural constant required |
| `residuals(newvar)` | Response residual | Formula selected by `residualtype()` |
| `solver()` | auto (default), map, lsmr, cmg, it, squarem | auto (default), map, lsmr, cmg |
| `threads(#)` | Nonnegative integer; 0 resolves native default | Same |
| `tolerance(#)` | Finite 0 < t < 1; default 1e-8 | Same, outer convergence tolerance |
| `iterate(#)` | Positive integer; default 16,000 | Positive integer; default 10,000 outer iterations |
| `timeit` | Runtime diagnostics | Runtime diagnostics |
| `verbose(#)` | Existing verbosity control; default 0 | No new public verbosity option |
| `offset(var)` / `exposure(var)` | Unsupported | Mutually exclusive |
| `separation(methods)` | Unsupported | Default `fe simplex ir`; optional mu or none |
| `xb()`, `d()`, `eta()`/`xbd()`, `mu()`, `dropreason()` | Through documented postestimation | Saved outputs (§12) |
| `biascorrection(exact|randomized)` | Unsupported | Optional mean correction (§15) |
| `biasunit()`, `biasfe()`, `biasprobes()`, `biasseed()`, `biasstream()` | Unsupported | Correction controls (§15) |

Preserve supported solver/map/iteration options, preconditioner(), poolsize(),
memory() and deterministic. solver(cg) remains rejected with r(198) under the
existing safety repair. Explicit LSMR/CMG and both PPML mean-bias modes are required.

Both commands load one platform-native fereg plugin. The former engine() option,
developer engine selector and engine-identity results are removed. Unsupported
engine() requests return r(198); a missing plugin returns r(601). Diagnostics
identify the native plugin and build.

The model capabilities below are preserved. IV/GMM, new categorical-continuous
or continuous-continuous RHS interactions, group outcomes with individual FEs,
HAC/Driscoll–Kraay, other GLMs and GPU execution remain unsupported.
Hidden separation tuning, allocation injection and plan overrides are test
interfaces, not additions to the public options above.

### Solver support and dispatch

`within` names the pinned modified-LSMR library, not a fourth public solver.
All supported backends must honor the same scientific model and accuracy
contract. Shared internal operations preserve explicit solver and storage support;
unsupported requests fail clearly.

| Request | OLS support to preserve | PPML support to preserve |
|---|---|---|
| auto | No FE, one/multiple intercept or slope-bearing FEs, disconnected designs | Same model grammar with changing positive IRLS weights |
| lsmr | General supported FE grammar, including slopes and multiple RHSs | At least two categorical intercept FEs, including more than three; no absorbed slopes |
| cmg | Categorical intercept graph: two largest effects as bipartite core, additional effects with at most 64 non-base nuisance columns; no slopes | Exactly two or three categorical intercept effects; third-effect nuisance width at most 64; no slopes |

The OLS CMG adapter also handles zero/one categorical effects by exact
no-work/direct projection; preserve this degenerate support and identify that
no graph iteration occurred. PPML explicit requests outside its supported
envelope return r(498), including zero/one FE. A valid graph formulation,
component anchoring and certified nuisance elimination are required for CMG;
the 64-column bound is an explicit adapter support limit, not an auto-routing
threshold. Retain all saved-output and bias-correction capabilities on supported
solver/model combinations. Tests force each supported backend independently.

The copied engine retains its existing automatic solver policy, iteration
budgets, support checks and numerical fallbacks. The abandoned direct/Schur
candidate and fixed two-attempt policy are not adopted. Future route changes
need a scoped decision and comparative evidence; no new N/width/thread/weight/
memory fences may be introduced to rescue individual performance cells.
Preserve requested/used solver, work accounting and existing reason diagnostics;
unsupported explicit requests must not silently become auto.

Explicit OLS `solver(lsmr)` stops its recurrence at a hundredth of
`tolerance()`, floored at 1e-15 and never looser than `tolerance()`, and
certifies the FE moments at `tolerance()`. Johannes chose these scoped changes
on 2026-10-10: an equal-tolerance stop could leave a certificate just above the
request, both in the main projection
(`development/ols_three_fe_batch_oracle.do`) and in the 1e-14 final residual
projection used for saved outputs. Unlike automatic LSMR (floor 1e-14, with a
MAP fallback), explicit LSMR has no fallback, so it keeps that margin. Explicit
PPML LSMR keeps its scheduled certificate and one tighter continuation.

Ordinary automatic two-FE OLS first targets half of `tolerance()` in its
recurrence and certifies at `tolerance()`. Only a failed certificate permits
one tighter correction solve on the reconstructed residual, using the automatic
LSMR target when strictly tighter. Both attempts retain the original RHS
normalization and share the original PCG budget and MAP reserve. A passing
certificate is accepted even at recurrence exhaustion. This scoped adaptive
policy does not change explicit solvers, PPML or final-residual projection.

Automatic routes can differ at roughly the 1e-5 level in derived statistics
on weakly connected graphs, despite passing their requested FE-moment certificate.

Tests: `tests/model_wire_{roundtrip,adversarial}.rs`, `parser/`,
`parity_ppmlhdfe/feppml_command_contract.do`,
`development/ols_cmg_raw_residual_oracle.do`, `tests/prepared_fit_oracle.rs`.
Retain mapped support/rejection, exhaustion and fallback checks.

## 2. Sample construction, missingness and restrictions

Stata evaluates `if`/`in` once. Candidate exclusion precedes scientific domain
validation: excluded invalid values cannot cause a data-domain failure.
On candidates, apply outcome, finite regressors, weights, FE sources, cluster
sources, then PPML adjustment availability/domain rules in that order. Stata
numeric missing values, including extended missings, and string missingness
retain their host meaning. Missing required sources exclude rows; fatal domain
errors are not converted to exclusions. Cluster availability precedes singleton
pruning; cluster counts never define FE singletons.

Ordinary factor values are validated on the resulting available sample, after
these exclusions but before singleton pruning, separation or rank decisions.
They must be nonnegative integers; otherwise fail with r(452), including for
omitted/base terms. Invalid factor values on excluded rows do not cause an
error. This follows built-in Stata `regress`; it intentionally supersedes the
former candidate-wide ordinary-factor check and differs from pinned reghdfe
6.14.1 in excluded-row cases. FE and cluster identifiers
retain their existing numeric/string domains.

Rust owns classification, collision-safe factorization and stable compaction.
The same original-row map compacts every source, recipe, weight, offset and
topology. After deletions, rebuild final codes and factor recipes; preserve
canonical coefficient slots when levels vanish. Dataset order never changes.
Time-series materialization respects the full declared panel/time settings;
a retained lag/lead can use a source outside `if`/`in`, and gaps remain missing.
Empty or insufficient final samples fail, with no public sample posted.

Tests: `tests/numeric_sample_oracle.rs`,
`tests/categorical_final_sample_rebase_oracle.rs`,
`parity_ppmlhdfe/feppml_native_preparation.do`, `feppml_time_series.do`.
Replacement checks: `tests/categorical_cluster_preparation.rs` and
`tests/stata/development/reference_factor_precedence.do`.

## 3. Weights

| Kind | Candidate rule | N_eff | Reference objective weight |
|---|---|---|---|
| none | Structural unit weight; no pseudo-column | N | 1 |
| aw, OLS only | Zero/missing excludes; negative r(402); otherwise finite positive | N | raw × N / sum(raw) |
| fw | Missing excludes; otherwise positive exact supported integer; invalid r(401) | Checked exact frequency sum | raw multiplicity |
| pw | Missing excludes; otherwise finite positive; invalid r(459) | N | raw × N / sum(raw) |
| iw | r(101) | — | — |

PPML also rejects aw with r(101). Each fweight must be an integer in
1 through 2^32−1. Accumulate effective N with checked integer arithmetic
within the platform's supported index range; reference-statistic conversions
also enforce exact floating-point integer representability. Rounding must
not silently admit an out-of-domain multiplicity.
Raw weight expression/sum remain metadata. Common aw/pw rescaling leaves
estimates, reference-scale bread, covariance and fit statistics unchanged.
Internal common rescaling is permitted only with explicit conversion back to
reference scale. Recompute normalization after sample mutation. Fweights match
literal row replication for objective, singleton multiplicity, N_eff, DoF,
scores and covariance; repeats do not create independent bias units.

OLS pw selects robust when VCE is omitted; explicit IID is rejected. HC1 meat
uses squared normalized aw/pw weights, but fw multiplicity rather than fw².
PPML prior weights q follow this table, while Hessian weights are q×mu.

Tests: `tests/numeric_sample_oracle.rs`, weighted `fereg-oracle/tests/ols.rs`,
`tests/prepared_robust_oracle.rs`,
`parity_ppmlhdfe/feppml_weights_inference.do`, frequency-expansion fixtures.

## 4. Singletons

Prune FE singletons to a fixed point unless `keepsingletons` is requested.
FW uses checked integer multiplicity; other weights use physical membership.
Removal counts remain physical rows. Apply the rule to the final available
model sources and all absorbed partitions, with deterministic original-row
identity. Separation can start another singleton cascade (§14). Singleton and
separated counts/reasons remain distinct. `keepsingletons` does not exempt a
model from positive DoF, finite-rank or convergence requirements.

Tests: `src/fe/singleton.rs`, weighted singleton-cascade Stata fixtures,
`parity_ppmlhdfe/feppml_separation_outputs.do`.

## 5. Absorbed terms and saved normalization

Support numeric/fixed-width-string categorical sources; `g h` for separate
effects; `g#h` for observed-tuple interactions; `g#c.x` and `g#c.(x z)` for
slope-only channels; `g##c.x` and `g##c.(x z)` for intercept plus slopes.
Repeated sources share coding without changing declared term order. `strL`
and categorical `##` expansion inside absorb are unsupported. Disconnected
components, duplicate terms and locally rank-deficient slope channels retain
their exact span and reported redundancy; do not manufacture extra rank.

`name=term` saves normalized observation contributions. An intercept uses
`name`; slope channels use `name_Slope1`, `name_Slope2`, etc. Slope-only terms
have only suffixed channels. These are channel contributions (coefficient
times slope), not bare slope coefficients. Names must be new and distinct
from every other requested output and input binding. `savefes()` is not an alias.
Within each connected component, center every intercept effect after the
first in declared absorb order to weighted mean zero, transferring each
removed mean to the first intercept effect so the total is unchanged. Use
OLS fitting weights or PPML's accepted final Hessian weights q×mu. With zero
or one intercept effect there is no between-effect centering transfer. Slope
channels retain their rank-aware representative; arbitrary nonidentified
contrasts are not accuracy targets. Preserve the reported constant gauge and
compare nonunique FE solutions by fitted sums and identified contrasts. Apply
the same normalization to all backends. The distinct bias anchor in §15 must
not replace this ordinary saved-FE gauge.

Output recovery reconstructs the declared FE sum from coefficients, checks
each row, and certifies at the requested tolerance. Output recovery and automatic
tighter re-solves require declared affected scope before changes. Independent tighter test oracles remain required; reducing output work cannot relax their accuracy
envelopes. No output request may change the sample, rank, VCE or coefficient
status. Unsupported explicit backend structures fail before publication.

Tests: `tests/intercept_slope_oracle.rs`, `tests/fe_certificate_oracle.rs`,
`parity_reghdfe/estimate_ols_saved_outputs_parity.do`,
`parity_ppmlhdfe/feppml_saved_slopes.do`, `bias/final_recovery_precision.do`.

## 6. Regressors and canonical status

Support numeric continuous columns, numeric factor main effects,
all-categorical factor interactions, and Stata lag/lead/difference operators
on continuous variables. Stata supplies authoritative base/omission flags and
canonical names through `fvexpand`/`_ms_parse_parts`; Rust must not infer these
flags from spelling. Do not broadly expand design columns in ado/Mata.
Explicit RHS base overrides (`ib#`, `fvset`) with interacted or sloped
absorption remain unsupported until a separately approved extension.

Preserve input-order slots and stripes for kept, base, explicitly omitted,
FE-collinear and numerically rank-deficient columns. A vanished level becomes
an all-zero/omitted recipe without shifting subsequent slots. Omitted slots
and covariance coordinates remain consistently zero. No-regressor models use
an explicit empty graph, never the dataset's implicit full variable list.
Unknown operators, orphaned/gapped metadata, invalid flags, nonzero reserved
fields and pending time-series work are errors before observation access.

Tests: `parser/ordinary_factor_terms.do`, `parser/model_wire_v2_capture.do`,
`tests/model_wire_adversarial.rs`, final-sample rebase and TS dummy oracles.

## 7. OLS estimation

Minimize `(y-Xb-Dalpha)'W(y-Xb-Dalpha)` on the retained sample. Project y
and X using the same FE span/weights, solve finite WLS on the identified
coordinates, and reconstruct residuals from original sources. Independently
certify FE orthogonality, finite score, bread/inverse, covariance and fit.
No iterative recurrence status alone permits publication.

Where an absorbed intercept exists, the reported constant is
`mean_w(y) - mean_w(X)'b`. It is a normalization coordinate, not an extra
finite/absorbed rank direction. With within bread B its extended bread has
side `-B mean_w(X)` and corner `1/N_ref + mean_w(X)'B mean_w(X)`, using the
reference weights and their N_ref=N_eff. Pure slope-only absorbed models do
not automatically have a structural OLS constant. No-FE models retain the
ordinary constant unless suppressed. `noconstant` preserves its accepted
model/reporting semantics; it does not delete an intercept already in D.

Tests: `fereg-oracle/tests/ols.rs`, `tests/prepared_fit_oracle.rs`,
`tests/categorical_iid_oracle.rs`, normalized-intercept Stata parity fixtures.

## 8. Rank and collinearity

Use deterministic scaled rank decisions and canonical tie breaking. Guarded
diagonal-pivoted Cholesky is permitted only when pivots clear the stronger
square-root rank guard and independent score/inverse checks pass. Otherwise
scaled column-pivoted Householder QR owns omission decisions; never invert
an unpivoted singular cross-product. Solve only identified coordinates.
Local FE block rank is the weighted span rank, including constant/zero or
dependent slope channels; a singular local Gram block is not automatically
an invalid model. Weak-graph tests retain identified combinations even where
individual FE coordinates are not unique.

PPML freezes canonical finite rank/status after the final separation/singleton
sample. Positive changing IRLS weights cannot silently change public omission
decisions. Revalidate that basis against the accepted final Hessian; a final
rank loss or inconsistent rank decision is fatal. Starts, warm states and
ordinary iteration-specific factorizations have no canonical-rank authority.

Tests: `src/ols/finite_wls_kernel.rs`,
`tests/intercept_slope_oracle.rs`, `tests/prepared_fit_oracle.rs`,
`parity_ppmlhdfe/feppml_scientific_contract.do`.

## 9. Degrees of freedom and nesting

Preserve the pinned reference's absorbed-category/redundancy rules, not a
generic count of FE labels. `pairwise` applies pairwise mobility reductions;
`firstpair` adjusts only the first eligible pair; `continuous` applies slope
redundancy without categorical mobility searches; `none` disables those
adjustments but still charges absorbed coefficients. Two-way graph components
give exact rank; multiway approximations remain explicitly marked. Constant
and locally redundant slope channels follow the accepted continuous rules.
Report categories, redundancy, rank, exactness/approximation and nesting per
declared channel. Nesting uses complete collision-safe final cluster tuples.

For OLS let K_f be finite model DoF and A the nesting-adjusted absorbed rank.
Unclustered residual DoF is N_eff−K_f−A and must be positive. With clusters,
at least two groups per dimension are required, reported residual DoF is
`min(N_eff−K_f−A, G_min−1)`, fit-statistic DoF subtracts the nested absorbed
category count as well, and the covariance correction denominator is
`N_eff−K_f−A−1{nested categories > 0}`. Each required denominator must be
positive. The normalized absorbed constant is not charged twice. Retain the
48-row positive-DoF slope fixture and the original 24-row failure regression;
negative IID variance is never a successful result.

PPML has its own policy: structural DoF is N_eff minus finite within rank
minus absorbed rank; clustered reported DoF additionally caps at G_min−1.
Positive reported DoF and at least two groups per declared cluster dimension
are required. PPML does not borrow OLS finite-sample corrections or t/F laws.

Tests: `src/ols/{topology_dof,prepared_dof}.rs`,
`tests/categorical_clustered_oracle.rs`, positive-DoF slope Stata fixtures,
`parity_ppmlhdfe/feppml_weights_inference.do`.

## 10. Covariance

Let B be the identified bread and s_i the within-regressor score. OLS IID is
`RSS/df_r × B`; HC1 is `N_eff/df_r × B meat B`. OLS cluster scores are
`sum_g omega_i r_i x_tilde_i`; combine every nonempty declared-cluster subset
by inclusion–exclusion and apply one common correction
`(N_eff−1)/correction_df × G_min/(G_min−1)` from §9. Support up to ten
OLS cluster dimensions. `#` defines one interaction dimension; `##` is invalid.
FW robust meat uses multiplicity, while cluster scores use full weighted sums.

PPML supports one through five cluster dimensions. Its bread is the inverse
of `sum q_i mu_i x_tilde_i x_tilde_i'` at the accepted Hessian.
Robust scores use `q_i(y_i−mu_i)x_tilde_i`; FW meat uses f_i times the
unweighted score outer product, not f_i². Robust correction is
`N_eff/(N_eff−1)`. Cluster scores sum the weighted scores within groups;
one common `G_min/(G_min−1)` multiplies the complete inclusion–exclusion
meat. No OLS residual-DoF or subset-specific multiplier applies.

Grouping is collision-safe and deterministic. Apply the registered symmetric
CGM PSD repair only after the complete multiway covariance has been formed;
retain repair diagnostics, symmetry/finite/diagonal checks and independent
covariance reconstruction. Johannes approved full-matrix OLS repair including
the reported constant; never restore a separately clipped slope block afterward.
This intentional difference from pinned reghdfe has an independent regression. Multiway repair sets negative covariance
eigenvalues to zero and independently checks the eigensystem and reconstructed
matrix. Robust and one-way clustered covariance must already satisfy the
strict PSD policy; they do not receive that multiway repair. Do not clip individual scores, coefficients or
subset meats. Wald rank and missing/undefined test statistics preserve the
canonical identified coordinates. OLS uses t/F; PPML uses z/Wald chi-squared.
The PPML model Wald leaves absorbed effects and the structural constant
unrestricted: it tests whether `eta−offset` lies in `span(D,1)`. Its null and
degrees of freedom are invariant to which identified recipes represent that
constant, including when the explicit `_cons` slot is numerically omitted.
Preserve original coefficient slots, base/omission flags and the accepted basis;
use identified contrasts when a nuisance constant spans several kept columns.

Tests: `tests/prepared_{robust,clustered}_oracle.rs`,
`tests/categorical_{iid,robust,clustered}_oracle.rs`, `src/ppml/psd.rs`,
`tests/python/ppml_multiway_cluster_oracle.py`, Stata weight/VCE fixtures.

## 11. Fit statistics and posted results

OLS RSS is the reference-weighted residual sum of squares; TSS uses centered
y when a structural intercept exists and raw y otherwise. Within TSS is the
FE-projected outcome norm. R² and within R² are `1−RSS/TSS` using their
respective denominators. Adjusted R² is
`1−(RSS/fit_df)/(TSS/(N_eff−1{structural intercept}))`; adjusted within R²
uses within TSS and baseline DoF `fit_df+K_f`. RMSE is `sqrt(RSS/fit_df)`.
Preserve the corresponding Gaussian ll/ll_0, model F/Wald, MSS and undefined
statistic behavior; zero denominators yield documented missing/absent fields,
never invented zeros or infinities. The cluster fit DoF is distinct from the
reported test DoF (§9). The OLS model F keeps two Cholesky pivot policies on
purpose. IID covariance is `s²B` on rank-certified coordinates, so a
nonpositive slope pivot contradicts the rank certificate and is an error.
Robust and clustered covariance may be legitimately singular (for example
G < K); a pivot within the certificate tolerance of the largest slope variance
leaves F missing, and a clearly negative pivot is an error.

PPML deviance is `2 sum q_i[mu_i−y_i+y_i log(y_i/mu_i)]`, with zero-outcome
contribution `2q_i mu_i`. Log likelihood is
`sum q_i[y_i eta_i−mu_i−lgamma(y_i+1)]`, including fractional outcomes.
The reference null mean is `sum(qy)/sum(q)` irrespective of FE/offset;
ll_0 uses that mean and pseudo-R² is `1−ll/ll_0` when defined. Publish outer
and inner counts, halvings and convergence. Do not post OLS RSS/TSS/MSS,
R²/adjusted R², RMSE, F or e(df_r) for PPML.

Both estimators preserve e(b), e(V), e(sample), coefficient/equation stripes,
model/command/weight/cluster metadata, effective and physical N, candidate/full
counts, ranks, absorbed metadata, singleton accounting, prediction binding
and requested output names. Sample/status/count/DoF fields are exact.
Required OLS families include `fereg_status`, `fereg_dof`, `fereg_fe`,
`fereg_cluster`, `fereg_subset`, `fereg_solver`, `fereg_certificate` and
`fereg_runtime`. Required PPML information includes:

- N_physical, N_effective, N_candidate, N_full; rank, feppml_model_rank,
  df_m, df_resid, df_structural, df_a, N_hdfe, N_hdfe_extended;
- deviance, ll, ll_0, r2_p, mu_0, sum_y, ic, inner_iterations,
  step_halvings, converged; chi2/df_chi2 when defined;
- num_singletons, drop_singletons, separated, sumweights and applicable
  raw_sumweights/weight_multiplier and logarithms; N_clustervars,
  N_clust1 through N_clust5, N_clust and G_min when clustered;
- feppml_status, fit, dof, fe, cluster, subset, separation, inference,
  drop_counts, separation_sequence, certificate, solver, runtime and receipt
  matrices (all names after the first have the `feppml_` prefix);
- `dof_table` when applicable; requested/selected/effective/checkpoint solver
  macros, concrete engine identity, fallback count and reason;
- `cmd=feppml`, `model=poisson`, command line, depvar, indepvars, vce, absvars,
  separation, adjustment, predict=feppml_p, requested output bindings and
  bias-correction results when requested (§15).

Retain the existing result layouts and native protocol. Any
telemetry simplification needs a scoped compatibility change; scientific fields
and solver identity/failure evidence remain required. Preserve runtime input/native/total measurements
needed by the benchmark suite, with defined boundaries. Undefined, unavailable
and absent remain distinct; validators reject stale/malformed matrices.

Tests: `parity_reghdfe/`, `parity_ppmlhdfe/feppml_scientific_contract.do`,
`estimate_ppml_runtime_validator.do`, native result-wire and statistic tests.

## 12. Outputs and prediction

Requested observation outputs use original-row identity and are missing
outside e(sample), except `dropreason()` which covers the whole dataset.
Do not retain hidden O(N) prediction caches when outputs are not requested.
All outputs belong to the same estimation transaction.

OLS predict defaults to xb. xbd, d and residuals require the estimation's
retained residual variable: xbd=y−r and d=xbd−xb. Missing retained state is
r(498); deletion of its named variable is r(111). `test` uses posted b/V;
`estat summarize` uses the estimation sample.

PPML xb includes the current finite predictor and offset/log exposure;
eta=xb+d and mu=exp(eta). Predict defaults to mu and intersects its own
if/in with e(sample). No-FE d is zero and all predictions are reconstructible.
With FEs, d/absorbed requires retained d. Eta/xbd, mu and residual predictions
require compatible retained d, eta or mu, otherwise r(498) before target
creation. Retained d is authoritative and combines with current xb; absent d,
eta then mu are snapshot fallbacks and do not track edited covariates.
Deleting the authoritative variable is r(111). Duplicate alias pairs or
multiple statistics are r(198). Estimation eta()/xbd() are aliases too.

| PPML residual type | Unweighted per-observation value |
|---|---|
| response, score | y−mu |
| pearson | (y−mu)/sqrt(mu) |
| working | (y−mu)/mu |
| deviance | 2[mu−y+y log(y/mu)], with zero-y limit 2mu; nonnegative contribution |
| anscombe | 1.5(y^(2/3)−mu^(2/3))/mu^(1/6) |

`residualtype()` requires `residuals()` and defaults to response.
Full-axis drop codes are 0 retained, 1 if/in, 2 missing outcome, 3 regressor,
4 weight availability, 5 FE, 6 cluster, 7 adjustment, 8 singleton,
9 FE separation, 10 simplex, 11 IR, 12 Mu, 13 other model exclusion.
Invalid retained domains cause errors, not successful drop codes. Report all
14 counts; separated sums 9–12. No dropped output can masquerade as a fitted
zero. Named FE channels follow §5; bias channels follow §15.

Tests: `postestimation/fail_closed.do`,
`parity_reghdfe/estimate_ols_postestimation_parity.do`,
`parity_ppmlhdfe/feppml_separation_outputs.do`, saved-output/alias fixtures.

## 13. PPML family, starts and IRLS

The model is eta=offset+Xb+Dalpha, mu=exp(eta), score q(y−mu).
Nonnegative fractional outcomes are valid; negative finite candidates fail
r(459). Empty/all-zero weighted final outcomes fail r(2001). PPML always has
a structural constant; an absorbed constant is not counted twice.
Offset/exposure are mutually exclusive (r(198)); missing excludes, finite
offset is required, and exposure must be finite positive (r(459)). Log
exposure exactly once. Adjustments affect fit/prediction but not the
homogeneous separation cone.

Start from `c0=log(sum qy)−log(sum q exp(offset))` using stable logarithmic
sums. Analytic/group/coordinate/slope start policies may improve this initial state. A declined
or invalid start proposal restores a finite global state. Starts never certify
rank, optimality or the final Hessian. Do not clip eta/mu to artificial bounds.

At an accepted state h=qmu and z=eta−offset+(y−mu)/mu, with exact zero-y
expression eta−offset−1. Stable evaluation may use exp(log(y)−eta).
Full Fisher steps are tried first; halve coefficient/predictor steps together
until values are representable and deviance nonincreasing within registered
roundoff slack. Exhausted steps or iteration budgets are fatal.
Relative deviance change is `abs(D_new−D_old)/max(min(D_new,D_old),0.1)`.
Require two consecutive accepted passes satisfying this and independently
recomputed normalized finite and FE scores. A small objective change alone
is insufficient, particularly near small coefficients.

Apparent convergence triggers fresh strict WLS at
`min(1e-12,0.1×outer_tolerance)`. If beta, eta, basis/rank or objective moves
beyond the registered publication envelope, resume bounded IRLS. Publish
only after exact accepted-h=qmu Hessian/bread, basis, projection, score,
objective replay and eta reconstruction checks. Refresh/invalidate all
current-weight operators, products and proofs whenever weights change.
Separately typed historical preconditioner state may survive solely as solve
acceleration, with its build identity retained and compatibility, memory and
work limits checked. It carries no current-weight, certificate, canonical-rank
or Hessian authority; the operator, RHS, stopping target and independent
certification still use the current problem. Sample mutation invalidates all
reuse. Warm starts are guesses, never certificates. Final inference and correction
use this same accepted state. Output tolerance (§5) does not remove the
strict PPML/final-Hessian requirements.

Tests: `src/ppml/family.rs`, `fereg-oracle/tests/seeded_dense_ppml_differential.rs`,
`tests/python/ppml_explicit_dummy_oracle.py`,
`parity_ppmlhdfe/feppml_one_fe_dummy_oracle.do`,
`feppml_multipartition_slope_oracle.do`, final-Hessian/line-search fixtures.

## 14. PPML separation and ledger

The ledger alone owns deletion. Detectors read immutable generation views
and propose sorted unique original-row IDs plus method/status/counts and a
certificate. Validate bounds, membership, uniqueness, eligibility and all
certificate fields before accepting any row; one invalid row rejects the
whole proposal. Detector traversal order must not choose public drop reasons.

A numerical recession direction z lies in the finite-plus-FE span, is zero
on positive outcomes, nonpositive on zeros, and strictly negative on every
proposed row. Independently check span distance, normalized equality/sign
margins, nonzero scale and finiteness. Exact zero-only intercept-FE levels
use a combinatorial certificate. Slope-only channels do not prove that rule.
Tiny fitted means, extreme eta and stagnation are not deletion evidence.

Accept nonrepeated method subsets in canonical `fe simplex ir mu` order;
default is `fe simplex ir`, mu is opt-in, and `none` alone disables proactive
separation only. Each round prunes singletons, applies requested exact FE
zero-level pruning to a fixed point, then simplex, IR, optional Mu. After a
certified deletion, apply its singleton cascade and requested FE fixed point,
commit the union as one generation, compact all sources together, rebase
topology once and restart the chain. First certified reason in canonical
method order wins; singleton counts remain separate.

Full-model simplex includes finite columns, every FE level and slope channel
in its explicit width; checked capacity includes design, null-space/tableau,
topology and certificates. A capacity skip is permitted only if later requested
comprehensive IR on the same generation certifies no unhandled separation.
A simplex-only or unresolved over-cap request fails r(909). An IR drop
restarts the chain and cannot certify a prior generation's skipped simplex.
Requested/default IR nonconvergence or uncertified output is fatal. Mu needs
a fitted start and the same independent cone certificate; accepted Mu drops
discard that entire fitted session.

A terminal no-drop result is checked before it is reported. For an IR
fixed point (Johannes, 2026-10-09) the check uses the stored recurrence and
projection residual: both are finite; the recurrence is nonnegative, zero
on positive outcomes and unit-scaled (or identically zero with a zero
residual, the vanishing projection); the stored fitted values reproduce the
reported sign certificate exactly; the reported iteration count is in range
and carries no exact-FE certificate; and every kept regressor and FE
intercept or slope channel has a zero weighted residual moment, relative to
the channel norm and the larger recurrence or residual norm, within the
certificate tolerance. These denominators are at least as large as the
projection's own certificates use, so this is an independent recomputation,
not a stricter test. If
any part fails, the detector is re-executed deterministically, as it is for
simplex and Mu no-drop results. Like the projection's own certificates, this
weighted check only weakly constrains fitted values on zero-outcome rows.

No prior-generation projected design, warm state, rank, cluster subset,
bread or output is reusable. Recompute canonical finite rank on the final
sample and verify positive weighted outcome. Report requested/executed
sequence, method status/iterations, generation identities and deterministic
trace digest independently of timing. Bound public diagnostic tables; no
observation-sized trace crosses the result wire. Separation proves a sample
decision, not estimator convergence.

Tests: `src/ppml/prepared_separation.rs`, `fereg-oracle/tests/ppml_separation_primers.rs`,
`tests/python/test_ppml_recession_cone_primers.py`,
`tests/python/ppml_recession_cone_oracle.py`,
`parity_ppmlhdfe/feppml_separation_methods.do`,
`feppml_separation_outputs.do`, full-model simplex capacity/oracle fixtures.

## 15. PPML coefficient mean bias correction

Retain `biascorrection(exact|randomized)` as an optional calculation on the
unchanged accepted fit. Require exactly two categorical intercept FE sets,
ordinary numeric controls, explicit numeric `biasunit()`, and none/fw/pw
weights; offset/exposure are allowed. Slopes, finite factors, other FE
topologies and multiway bias-unit dependence are unsupported. The score unit
comes from the declared dependence experiment, not VCE cluster settings.

Every retained level of both FE sets must contain at least two distinct units,
even without `biasfe()`. Count actual IDs, never FW copies. A missing unit on
a retained row is an error, not a sample exclusion. Any outcome-dependent
separation deletion rejects the entire correction transaction. Unit relabeling
does not change support; restrictions can change it. More probes, exact
calculation or splitting dependent units cannot establish valid independence.
This necessary support screen does not certify statistical adequacy.

For complete identified FE-plus-control row x, fitting weight a, accepted mu
and independent unit g, compute

```text
S_g = sum_g a x (y−mu)        J_g = sum_g a mu x x'
J = sum_g J_g                K = sum_g S_g S_g'
V = J^-1 K J^-1
c_D = J^-1 sum_g J_g J^-1 S_g
c_C = (1/2) J^-1 sum_rows a mu x (x' V x)
beta_bc = beta_hat + c_D + c_C
```

Both increments include all identified nuisance coordinates. Do not substitute
the public VCE, add HC1/DoF/PSD corrections or cap coefficients. This empirical
second-order mean correction is an approximation for interior fits; it is not
a correction for confidence intervals, predicted rates or effect variances.
Ordinary e(b), e(V), objective, sample, means and predict remain unchanged.
No leave-one-unit-out refits or observation selection enter this calculation.

Exact mode streams units without storing K, V or all scores; retain limits of
10,000 units and one billion observation-unit contributions. Randomized mode
has a fixed prespecified B≥2 (default 256), seed 739391 and stream 0;
seed/stream/probe controls are unsigned 32-bit integers. Exact mode rejects
explicit randomization controls. Ancillary bias options require correction.

Use Philox4x32-10 with key=(seed,stream), counter=(low/high IEEE-754 numeric
unit bits, low/high zero-based probe index). Normalize −0 to +0. The first
output word's low bit maps to −1/+1. One sign per unit/probe is shared across
both increments; no Stata RNG mutation. Signs are stable under row ordering,
grouping, worker count and route; renumbering units changes randomness.
Retain pinned Random123 known-answer vectors from the source contract.
Their source is commit `d8a0c25e49992685c0b9d91b553c1d04f81b6b4b`,
`DEShawResearch/random123/tests/kat_vectors`, SHA-256
`aab5ebabf40003f63d6d87b24cbd2c8a02652e00cf8bad64226fd50586929183`.

Point calculation uses B probe solves plus two final solves. Up to eight
predetermined independent groups use the same probes and probe-count weights;
at most fourteen extra final solves estimate numerical variation. For group
sizes m_j and increments d_j,c_j, covariance of the full means is
`sum m_j(d_j−dbar)(c_j−cbar) / [B(groups−1)]`. Analogous squares give each
variance; total variance includes twice covariance. These are conditional
Monte Carlo diagnostics, not sampling standard errors. Exact mode reports
zero probe uncertainty, not zero solver error.

Anchor the smallest canonical second-FE level to zero in each component and
put the component constant in the first FE. Apply that same linear map to raw,
derivative, curvature and corrected channels; do not Hessian-center each
channel differently. Recover raw effects from accepted eta minus offset and
unchanged finite contribution. Independently check row reconstruction,
including tiny means and disconnected branches. Ordinary saved FEs retain
their separate existing normalization.

Publish ordinary-stripe matrices `b_bias_raw`, `b_bias_d`, `b_bias_c`,
`b_bias_total`, `b_bc`, `bias_mcse_d`, `bias_mcse_c`, `bias_cov_dc`,
`bias_mcse_total` under e(). Omitted and normalized absorbed-constant slots
are zero. `biasfe(stub)` creates `stub_1_*` and `stub_2_*` with suffixes
raw,d,c,total,bc,sed,sec,covdc,setotal, missing outside e(sample).
Numerical SE channels are tied to this anchor; omitted cross-level covariance
prevents arbitrary SE renormalization.

Retain the 21×1 `e(bias_diagnostics)` named fields: method, units, probes,
seed, stream, independent groups, solves, maximum solve score, maximum FE
reconstruction error, seconds, workspace bound, maximum absolute total
increment, minimum/maximum mu, minimum/maximum rows per unit, route, validity,
certificate tolerance, minimum level information and maximum FE-leverage
lower bound. The last is max row h divided by its FE-level information, across
both effects; it is not the full leverage or unit-information concentration.

`e(bias_support)` has rows fe1/fe2 in absorb order and columns levels,
min_units,p25_units,median_units,p75_units,max_units,single_unit_levels,
single_unit_fraction,min_info_units,median_info_units,max_unit_info_share.
Quantiles are unweighted nearest rank across levels. With
I_level,g=sum a×mu and p_g=I_level,g/sum_g I_level,g, information equivalents
are `1/sum p_g²`; these are not certified effective sample sizes. Successful
single-unit counts/fractions are zero. `bias_validity=interior-retained-sample`
and numeric valid=1 mean necessary support and numerical checks only.

All solves use the accepted full-system basis and final a×mu, with independent
normal-equation and reconstruction certificates. Reuse bounded workspaces and
one worker team; count all probe/recovery work. Option-off allocates/solves none
of this. Missing IDs, support, rank, allocation, solve or Break failures roll
back the whole estimate. Error diagnostics identify channel, block/probe and
recovery discrepancy without posting partial matrices.

Tests: `tests/python/test_ppml_bias_oracle.py`, `src/ppml/bias*.rs`,
`parity_ppmlhdfe/feppml_bias_correction.do`,
`bias/final_recovery_precision.do`, duration reconstruction and Philox vectors.
Retain both increments, exact/randomized and support-refusal tests.

## 16. Certificates and errors

Every FE backend reconstructs r=v−Dalpha explicitly. Independently validate
finite coefficients, row reconstruction and D'Wr. Use the invariant
weighted local-span certificate: for each level's orthonormal basis Q_g of
sqrt(W)D_g, measure `||Q_g' sqrt(W) r_g|| / s`, where
`s=max(||sqrt(W)v||,||sqrt(W)r||)` for that RHS. A zero scale accepts only
zero projections. The maximum across effects/levels/RHS must meet tolerance;
retain raw moment and aggregate norm diagnostics. Recompute from declared
design/weights, not a recurrence's own residual or stale weighted factor.
Local rank-aware bases prevent slope scaling/rotation from changing the span
test. Residual reconstruction and small D'Wr are necessary, not a universal
forward-accuracy bound; §20 independently tests weak graphs.

OLS additionally certifies finite scores, bread/inverse, fit and covariance.
PPML additionally certifies model-space feasibility, scores, accepted objective,
final Hessian/rank, output reconstruction and any bias calculation. Finite but
uncertified candidates cannot be posted or relabeled as convergence.

| Failure class | Return code |
|---|---:|
| Syntax/removed option or duplicate incompatible syntax | 198 |
| Unsupported/invalid native argument or unavailable prediction state | 498 |
| Protocol mismatch | 499 |
| Unsupported weight | 101 |
| Invalid FW / negative AW | 401 / 402 |
| Invalid ordinary factor on the available sample | 452 |
| Empty/insufficient observations or required residual DoF | 2001 |
| Invalid data | 459 |
| Numerical nonconvergence | 430 |
| Allocation/checked size overflow | 909 |
| User Break | 1 |
| Self-test failure | 3498 |
| Internal failure/caught panic | 3499 |

Preserve more specific Stata binding errors (for example deleted variable 111)
and pinned fixture error precedence; a failed covariance validity check is
not successful missing inference. No errors are converted to a different
estimator, different sample or silently weaker tolerance.

Tests: `tests/fe_certificate_oracle.rs`, `tests/weak_graph_decimal_oracle.rs`,
`tests/prepared_fit_oracle.rs`, native error/wire tests, public rollback fixtures.

## 17. Transaction, transport and Break

One substantive plugin call owns estimation. Stata parses syntax/metadata,
holds prior estimates, creates private shadows and publishes only after native
completion and independent bounded transport validation. Rust loads observations,
prepares the sample, estimates, certifies and validates all N/K/K² products
before the first SPI store. All source/output identities, dimensions and
nonaliasing bindings are checked before observations. SPI calls and Break
polling occur only on Stata's main thread; workers touch Rust-owned memory.
Native work from preparation through derived outputs and saved-FE recovery
runs under Break polling, and long serial row passes checkpoint at a fixed
row stride.
Catch Rust panics at FFI; no unwind crosses C. Safe crates forbid unsafe code.

COMPLETE is the last native operation; no allocation, polling, display,
computation or store follows it. A partial physical SPI write has no logical
meaning. The final ado commit protects posting and public output renames.
Any error, Break, store failure, bad receipt or missing helper restores prior
e(), named/null estimates, data, order and output names. No correction or
postestimation path makes an extra substantive estimation call.

Retain independently versioned exact estimation handshakes. Current
versions are action 1.6, model 2.1, OLS result 1.5, PPML request 1.8, PPML
result 1.5, ado identity 0.5.0 and SPI 3.0. Diagnostics accept same-major,
not-newer-minor action callers. Model 2.0 is readable only for the bounded
no-absorbed-metadata shape;
unknown/newer identities and malformed framing fail before observation access.

Use canonical byte-length/count framing inline through 4,000 payload bytes;
larger requests use only `bundle_local=<byte-count>` and fixed caller-local
`fereg_request_bundle` (SPI name `_fereg_request_bundle`). No arbitrary macro
lookup. Cap native metadata at 16 MiB, reserve extra content and NUL bytes,
and validate exact NUL position, UTF-8 and closed decoder before observations.
Drop transport storage after decoding. Metadata-only Mata framing preserves
UTF-8 byte lengths and cleans all temporary externals independently on error.

Bounded compiled Mata validation/publication may remain in the control plane;
no observation-scale design/estimation work enters Mata. If retained, ship
objects compiled by Stata 18 and verify source/object identity. A `version 18`
statement in a newer compiler does not suffice. Missing/stale helpers fail
atomically; no runtime source-compilation fallback.

Tests: `tests/mock_spi/`, native FFI transaction tests, `tests/licensed_stata/`,
`parity_ppmlhdfe/estimate_ppml_hidden_transaction.do`,
`estimate_ppml_runtime_validator.do`, installation and framing failure fixtures.

## 18. Threads and repeatability

Honor threads(); 0 resolves the existing native default. Preserve repeatability
at a fixed thread count with deterministic and fixed random seed/stream.
Across worker counts require scientific agreement under §20 and stable sample,
omission, separation and other discrete model decisions. Bitwise identity
across different worker counts, platforms or processor features is not required.
Processor-dependent fast paths, including runtime FMA dispatch, may select the
fastest supported arithmetic when the same independent certificates and §20
scientific witnesses accept their results and discrete decisions stay stable.
The portable path remains supported and tested. Same-machine repeatability at
a fixed worker count and seed remains required. The deterministic option
remains supported. Low memory mode is one such case: when its scratch moments
are split by observations, the partial Gram matrices follow the planned worker
count, so the last bits of Low results may differ across worker counts while
Fast results do not (accepted by Johannes, 2026-10-09).

threads(n) is an available worker budget. Stages may use fewer workers under
general work/resource rules, with transparent requested, resolved-budget and
effective concurrency. No-numerical-change commits preserve scientific bits and
available solver/route signatures against the parent under equal build settings.
Numerical fixes declare affected cases/fields; unaffected science remains exact.
Exclude timing, build identity and environmental resource observations only.
A tolerance pass cannot silently waive an unexplained exact-change failure.
Preserve Stata processor restoration on success, error and Break.

Scoped changes may deliberately change algorithms, arithmetic order and storage. Declare affected outputs, routes and
errors before implementation; require independent scientific acceptance and
unchanged discrete model semantics. Unaffected outputs remain exact, and pure
refactors retain exact route/bit checks. No post-hoc parity exception is implied.

## 19. Memory behavior

Preserve supported memory modes, admission policy and public controls.
Preserve checked sizing, fallible allocation, r(909), ownership cleanup and
transaction atomicity. Reuse existing allocation/failure tests; no new exhaustive
memory-audit framework is required. Estimates remain estimates, not process RSS
or strict OS bounds. No new route-specific admission fences may be added to
hide a performance regression. Future removal of a mode needs mapped coverage.

## 20. Accuracy tolerances and independent evidence

For well-conditioned comparisons, require coefficients rtol=1e-7/atol=1e-10,
SE/VCE rtol=1e-6/atol=1e-10, and residual/fitted values
rtol=1e-7/atol=1e-9. Preserve stronger named regression thresholds, including
the 2e-8 CMG dense-oracle check. Exact equality is required for applicable success/error, sample, omission/
status, VCE/DoF policy, clusters, singleton accounting and discrete scientific
metadata, except named independently justified bug-fix differences. Runtime tolerance is not permission to weaken these
independent accuracy checks. Adjudicate nonunique FE coordinates through
identified combinations, fitted values, residuals and objectives.

Retain the sealed 100-digit `tests/reference/weak_graph_decimal_oracle_v1`
CSV/JSON with original identity. Test chains, barbells, AKM weak mobility,
disconnected graphs, unequal weights, slopes and multiple RHSs against the
explicit design, with one grounded coordinate per component. Exercise every
supported backend, not just auto. Preserve the CMG raw-residual regression
and the independent witness that FE moment certification alone can accept a
wrong CG result (`fereg-oracle/tests/seeded_dense_ols_differential.rs`). Large LSMR stability comparisons include a 100-times-tighter
solve; reference residual distance is diagnostic when independent accuracy
and runtime certificates pass, while exact fields and coefficient/VCE/fit
envelopes remain binding.

Pinned reghdfe/ppmlhdfe closures in `references.lock.toml` are compatibility
references; dense weighted dummy OLS, Poisson, sandwich and recession-cone
oracles remain independent authorities. For PPML the pinned comparator's
reported ll/deviance are the reference objective. Its predicted state is an
additional witness only if independently feasible and stationary; projection
alone is insufficient. Fereg's own mean must replay its objective and pass
all score/model-space/final-Hessian checks. A reference-error exception needs
an independent optimum enclosure proving the discrepancy; it does not alter
runtime acceptance or rewrite historical failed pairs.

Performance workflows never weaken science. Missing or timed-out comparisons
are not successes. Licensed checks require application return codes and terminal
sentinels in addition to process status. Tests include the sealed weak-graph
witness, independent Python oracles, actual-plugin replay and native/Stata parity
families above. Current qualification and its limits are in [VALIDATION](VALIDATION.md).
