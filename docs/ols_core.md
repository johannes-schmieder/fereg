# OLS scientific core

The OLS core is Stata-independent and implements the numerical pipeline that
will eventually sit behind `stfeols`. The Stata command remains fail-closed
until the SPI loading and result-posting protocol is wired and licensed-Stata
parity has passed.

## Pipeline

1. Validate finite outcomes, regressors, positive effective least-squares
   weights, fixed-effect dimensions, clusters, and numerical tolerances.
2. Residualize the outcome and every finite regressor together through the
   certified FE router. A failed final FE certificate aborts the fit.
3. Mark columns whose transformed weighted norm is negligible relative to the
   original weighted norm as FE-collinear.
4. Scale remaining columns to unit weighted norm.
5. Use symmetric diagonal-pivoted Cholesky on the standardized normal equations
   for the well-conditioned path. Automatically switch to Householder
   column-pivoted QR when Cholesky reports weak rank or an unsafe pivot.
6. Preserve a canonical status for every supplied finite column and solve only
   on the identified subspace.
7. Reconstruct structural residuals and fitted values using the FWL identity.
8. Compute centralized degrees of freedom, fit statistics, and the requested
   covariance estimator.

No unpivoted normal-equation inverse is used. The identified bread is recovered
from the accepted Cholesky or QR factor and transformed back to the original
column scale.

## Strict borrowed-intercept MAP checkpoint

The separately gated borrowed-intercept MAP implementation accepts the
prepared-data borrowed intercept view. Its public generic entry point preserves
the empty-design identity contract; prepared OLS calls an internal wrapper that
treats empty explicit topology as a virtual structural constant. It is
deliberately narrower than the generic pipeline above: intercept-only effects,
a nonempty dense RHS matrix, and unit or supplied finite positive objective
weights. It never constructs the owned `FeDesign`, invokes `within`, or copies
observation-scale codes and membership.

The function consumes its matrix. It validates dimensions, values, weights,
and configuration and fallibly reserves every O(K) projection/norm/certificate
buffer before the first mutation. Weighted group means use scaled compensated
accumulation. The final gate recomputes each normalized weighted FE projection
against the maximum of the pre-absorption and residual RHS norms. No recurrence
or change-in-iterate test can substitute for this certificate. An exhausted
budget or failed/nonfinite certificate returns an error; every successful
result has `SolveReport::certified() == true`.

`build_prepared_ols_rhs` is the bounded bridge from prepared OLS data to this
solver. It revalidates the prepared sample/model identities and directly
constructs one fallibly reserved row-major batch with Y first, followed by the
generated model columns in their existing order. Syntax-omitted coefficient
slots are absent and the intercept remains last. The integrated wrapper builds
the borrowed FE view first, then the batch, borrows any prepared weights, and
consumes the batch through the strict solver.

The developer plugin preparation action invokes that wrapper only when its
hidden `mapcheck` option is present. The option defaults off, runs before the
first diagnostic write, discards the residualized batch, and exposes only a
small success summary. Empty input, interruption, allocation/overflow,
prepared-data corruption, solver nonconvergence, or failed certification is
fatal before any SPI output mutation or display. This check does not post a
coefficient or any other numerical result. Public estimators still do not
invoke this path, and the older generic MAP/LSMR/OLS stack has not thereby
become eligible for Stata result posting.

## Certified prepared coefficient and bread checkpoint

`fit_prepared_ols_coefficients_map` is a separate core-only checkpoint over
`PreparedOlsData`. It deliberately uses only the prepared Y/X builder and the
strict borrowed-intercept MAP path; it does not call the older generic OLS,
rank, VCE, DoF, or owned-`FeDesign` implementations. Its currently admissible
model surface is therefore exactly the prepared boundary: continuous columns,
numeric factor-main-effect and all-categorical-interaction recipes, zero or
more single-source intercept-only numeric fixed effects, and structural unit
or prepared analytic, frequency, or probability objective weights.

The fit first records stable weighted pre-absorption norms, residualizes Y and
all generated X columns together, and classifies columns annihilated by the
fixed effects. Remaining columns are scaled by those reference norms and
solved in place with deterministic column-pivoted Householder QR. Pivot norms
are recomputed from the trailing matrix on every step, ties retain canonical
generated-column order, all dimension arithmetic is checked, and nonfinite or
uncertified rank decisions are fatal. Each returned canonical coefficient slot
is `Kept`, `SyntaxOmitted`, `CollinearWithFixedEffects`,
`NumericallyRankDeficient`, or `NormalizedAbsorbedIntercept`. An empty explicit
FE topology still has the pinned reference's virtual one-level structural
constant, under both default reporting and `noconstant`. It is projected by a
weighted global demeaning pass over the existing RHS with six fallible O(K)
workspaces; no N-row constant codes or membership are materialized. A reported
`_cons` is therefore always outside finite QR, while `noconstant` suppresses
only that reported coordinate. Atomic fit finalization reports the pinned normalization
`_cons = mean_w(y) - mean_w(X)b`. This does not recover individual fixed-effect
coefficients. The reported coordinate adds no additional rank; for empty
explicit topology the virtual structural effect itself contributes `df_a=1`.

The same finalization returns inverse cross-product bread on covariance
coordinates in canonical kept-generated-column order, followed by the
normalized absorbed intercept when present. If `B` is the finite within bread
and `m=mean_w(X)`, the added side is `-Bm` and the corner is
`1/N_ref + m'Bm`. Cross-product weights are unit when unweighted, raw for
frequency weights, and normalized to physical N for analytic and probability
weights. Thus common analytic/probability scaling leaves the bread invariant;
common integer frequency scaling divides it by that integer.

After solving, the checkpoint reconstructs one raw N-vector residual from the
prepared recipes and kept coefficients, residualizes that vector again through
strict MAP, and recomputes weighted score moments for every generated column.
The scores use independent recipe evaluation and weighted post-initial-MAP
within-column and within-outcome norms saved before destructive QR. The final
within-residual norm remains a roundoff-safe response fallback. If a generated
column has exactly zero within norm but nonzero raw norm—such as a deferred
intercept—the raw column norm is used so its redundant score moment remains in
the all-column gate; only a truly zero raw column contributes a structural zero
moment. Raw
pre-absorption X or Y variation explained only by fixed effects therefore
cannot dilute a finite-coefficient score failure. A failed initial MAP
certificate, residual MAP certificate, dropped-rank certificate, all-column
score certificate, finite/full-reported left or right inverse certificate,
bread-symmetry/positive-diagonal certificate, or intercept formula and
normalization certificate returns an error and no result object. The full
reported inverse check is independent: it retains the pre-QR finite
cross-product by reusing a dead O(K^2) buffer and reconstructs
`G=[D+N_ref mm', N_ref m; N_ref m', N_ref]`. The returned object contains
canonical coefficient slots, kept/dropped generated-column identities, finite
rank, both MAP reports, rank/score/bread certificates, covariance-coordinate
bread, and final compact-axis residuals.

Independent explicit-dummy tests cover no, one, two, and repeated fixed-effect
dimensions; unit, analytic, frequency, and probability objectives; exact and
clear near-collinearity; factor omissions and vanished levels; random
well-conditioned designs; observation permutations; and common weight scaling
from `1e-200` through `1e200`. Exact rational cases, literal frequency-row
expansion, sealed Stata 18 no-FE/disconnected/aweight/rescaled/fweight/pweight
anchors, allocation/capacity/checked-arithmetic injection, and coefficient,
finite-bread, reported-intercept, and symmetric-extension perturbations prove
that workspace or certificate failures are fatal. A 64-row FE allocation audit
sweeps every reservation failure and bounds all new Gram/bread/intercept
allocations by O(K^2)/O(K), proving that this checkpoint adds no N-scale
covariance copy.
Targeted counterexamples with `1e10` FE-only outcome variation and `1e7`
FE-only regressor variation prove that neither raw scale can mask an injected
coefficient error while the corresponding unperturbed fits remain accepted. A
separate residual-constant injection proves that the zero-within-norm deferred
intercept moment is checked rather than hard-coded to pass.
Pinned default/`noconstant` pairs prove identical finite coefficients,
statuses, residuals, DoF, statistics, and finite bread for no-absorb and
explicit-FE models; only the normalized `_cons` coordinate differs. Literal
`c.one` is FE-collinear before QR, and full `ibn.g` retains L-1 finite
contrasts. Corrupt structural-intercept metadata is rejected before MAP or DoF.
Independent rereview approves the repaired series through `adbc345`. This
checkpoint has no plugin action, performs no SPI write, computes no variance
scale/meat, posts no VCE or DoF, recovers no individual FE coefficients, and
does not enable `stfeols`.

## Prepared absorbed-DoF checkpoint

`compute_prepared_ols_absorbed_dof` is a separate core-only structural-rank
checkpoint for the currently prepared single-source, intercept-only numeric FE
surface. It takes `&PreparedOlsData` because that object retains the single
authoritative categorical code slice for each unique source. Absorbed-term
order, identities, labels, level counts, and memberships come from
`PreparedOlsFeTopology`; the categorical slices are only borrowed to form
linear-time pair edges. The calculation does not inspect model columns,
outcomes, regressors, weights, or singleton diagnostics, and it allocates no
observation-scale copy.

For wire-ordered effects with category counts `K_j`, the report implements the
pinned reference policies as follows:

- `None` skips mobility searches but still applies the mandatory intercept
  redundancy: `M_1=0` and `M_j=1` for each later nonempty effect.
- `FirstPair` replaces `M_2` with the number of connected components in the
  first two effects' bipartite graph; later nonempty effects retain `M_j=1`.
- `Pairwise`, the default, uses `M_j=max_i C_ij` over every earlier/later
  bipartite pair, with the mandatory one as the initial bound.

The absorbed rank is the checked sum of `K_j-M_j`. Each wire effect reports its
term/source identity, label, category count, redundancy, coefficient count,
exactness flag, and `nested=false`. The first effect is exact; the second is
exact when its first-pair mobility calculation runs; later pairwise bounds are
explicitly approximate. Repeated wire terms keep separate rows but reuse their
single topology source.

For a nonempty no-absorb model, the explicit topology remains empty but the
report prepends a distinct `ImplicitNoAbsorb` row labeled `_cons`, with one
category, zero redundancy, one exact coefficient, and `nested=false`. This
synthetic row contributes `df_a=1` under both default reporting and
`noconstant`; it never invents an `AbsorbTermId` or `SourceId`. Its row and
label reservations have branch-specific fail-at-every and undersize-at-every
coverage.

Pair mobility uses iterative, path-compressing union-find with union by rank.
All production vectors and strings are fallibly reserved and capacity-guarded;
all totals and node offsets use checked arithmetic. Corrupt identity,
membership, dimension, code, cycle, overflow, allocation, or capacity state is
fatal. Independent graph-search tests exercise randomized pair topologies, and
the sealed licensed-Stata receipt anchors `df_a=8` for one firm FE,
`df_a=19` for `dof(none)` on worker plus firm, and `df_a=21` for
`dof(firstpair)` on worker, firm, and year.

This checkpoint deliberately excludes absorbed slopes, FE/cluster nesting,
finite-regressor rank, normalized absorbed-model intercept handling, effective
frequency-weight observation counts, residual DoF, covariance corrections,
and fit statistics. It has no plugin action or SPI write and does not enable
`stfeols`.

## Certified prepared unadjusted-inference checkpoint

`fit_prepared_ols_unadjusted_map` is the only eligible prepared iid composition.
It constructs the certified coefficient/bread fit and the requested structural
absorbed-DoF report inside the same call, so callers cannot accidentally combine
artifacts from different prepared samples. It does not call the older generic
fit, VCE, fit-statistic, or DoF routines.

The fit retains one scalar that would otherwise be destroyed by QR: the
reference-weight sum of squares of Y immediately after the initial certified
MAP. This is within TSS. For empty explicit topology, virtual global demeaning
makes it the centered overall TSS under both default and `noconstant`; future
structural-constant-free model surfaces would instead use uncentered TSS. The
returned rank report centralizes:

```text
q          = kept finite rank
df_m       = q - kept ordinary finite intercept count
model_rank = q + absorbed_rank
df_r       = N_effective - model_rank > 0
```

For the current supported surface, `df_m=q`: the structural constant is owned
by absorbed rank even when `_cons` is reported. The normalized reported
`_cons` covariance coordinate contributes no additional rank. Unweighted and analytic models use physical row count
as effective N; analytic reference weights are raw weights rescaled to sum to
physical N. Frequency models use raw positive integer weights and their exact
checked sum as effective N. The raw supplied weight sum is reported separately.
Probability-weight iid inference is rejected because the pinned reference
changes positive pweights to robust inference.

On the fit's canonical kept-plus-normalized-intercept coordinate axis, the
checkpoint computes:

```text
sigma2       = RSS / df_r
V_iid        = sigma2 * bread
R2           = 1 - RSS / TSS
R2_within    = 1 - RSS / within_TSS
R2_adjusted  = 1 - (RSS / df_r) / (TSS / (N_effective - has_intercept))
R2a_within   = 1 - (RSS / df_r) / (within_TSS / (df_r + df_m))
RMSE         = sqrt(RSS / df_r)
F            = b_s' inv(V_s) b_s / df_m
```

Ratios, F, and Gaussian likelihoods that have no finite mathematical value are
`None`, never NaN sentinels. Here `b_s` and `V_s` contain only kept finite
nonconstant coordinates. F is returned only when `df_m>0` and the iid variance
scale is positive; a certified solve residual and the stable within-SSR identity
are independent cross-checks, not the construction. The null Gaussian likelihood uses the
fixed-effect-only within objective.

Before publication, the composite independently checks the reference-weight
sum, RSS/TSS/within primitives and ordering, residual-DoF identity, covariance
scale, symmetry, diagonals and standard errors, the Wald solve, and every
reported formula without calling the statistic builder again. Mutation tests
independently perturb RSS, TSS, within TSS, residual DoF, variance scale,
covariance, standard errors, both R-squared families, RMSE, F, and both
likelihoods and require every corruption to fail.
For reported covariance dimension P and finite slope dimension q, the
inference layer fallibly reserves P² covariance, P standard errors, q slope
positions, q² Cholesky storage, and q solve storage: P²+q² square elements and
P+2q vector elements in total, with no observation-scale copy. Allocation
sweeps, undersized reservations, corrupt
absorbed-DoF artifacts, and injected RSS/DoF/covariance/within/statistic errors
all fail closed.

Independent explicit-dummy tests cover unit, analytic, and frequency objectives;
no/one/two/repeated and disconnected FEs; random designs and row permutations;
literal frequency expansion; and extreme common analytic scaling. The sealed
Stata 18 receipt anchors no-FE scalar statistics and full iid covariance,
disconnected FE-only covariance, analytic and rescaled-analytic weights,
frequency weights, one-FE within statistics, and `None`/`FirstPair`/`Pairwise`
DoF policies. The checkpoint has no plugin action or SPI write, performs no FE
recovery, and does not enable `stfeols`. Independent final review approves the
complete repair series through `adbc345`.

## Legacy covariance source

The older generic core contains:

- conventional homoskedastic covariance;
- HC1-style heteroskedastic covariance;
- one- through four-way clustered covariance using deterministic score
  aggregation and inclusion-exclusion over cluster intersections.

The core consumes effective least-squares weights. Stata weight types must be
converted according to the frozen weight-semantics contract before this layer.
Cluster intersections are encoded as tuples rather than concatenated strings.
Any required cluster subset with fewer than two groups fails closed.

Those covariance routines and the older generic DoF routine are not certified
for the prepared estimator and must not be used for Stata result posting.
Prepared iid covariance, residual DoF, and fit statistics now have the separate
certified checkpoint above. Prepared robust/cluster score construction,
weight-specific meat semantics, finite-sample corrections, and cluster
intersections/nesting remain later checkpoints.

## Current boundary

These checkpoints are scientific-core implementations, not a claim that
`stfeols` is production-ready. Remaining work includes FE coefficient recovery,
prediction, Stata data ingestion and result posting, weight-type parity,
remaining reference DoF/VCE policy, licensed-Stata differential tests, and
large-scale speed and memory gates.
