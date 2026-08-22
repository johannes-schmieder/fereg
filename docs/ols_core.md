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

The separately gated `absorb_borrowed_intercept_map_certified` path is the only
current FE solver that accepts the prepared-data borrowed intercept view. It is
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

## Certified prepared coefficient checkpoint

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
uncertified rank decisions are fatal. Each canonical coefficient slot returns
exactly one of `Kept`, `SyntaxOmitted`, `CollinearWithFixedEffects`,
`NumericallyRankDeficient`, or `AbsorbedInterceptDeferred`. With no fixed
effects, the intercept is fitted normally. With any absorbed intercept, the
within intercept is annihilated and the normalized reference `_cons` is
explicitly deferred until fixed-effect recovery; it is never reported as zero
or as an identified coefficient.

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
certificate, residual MAP certificate,
dropped-rank certificate, or all-column score certificate returns an error and
no result object. The returned object contains only canonical coefficient
slots, kept/dropped generated-column identities, finite rank, both MAP reports,
rank/score certificates, and final compact-axis residuals.

Independent explicit-dummy tests cover no, one, two, and repeated fixed-effect
dimensions; unit, analytic, frequency, and probability objectives; exact and
clear near-collinearity; factor omissions and vanished levels; random
well-conditioned designs; observation permutations; and common weight scaling
from `1e-200` through `1e200`. Allocation/capacity injection and coefficient
perturbation tests prove that workspace or final-score failures are fatal.
Targeted counterexamples with `1e10` FE-only outcome variation and `1e7`
FE-only regressor variation prove that neither raw scale can mask an injected
coefficient error while the corresponding unperturbed fits remain accepted. A
separate residual-constant injection proves that the zero-within-norm deferred
intercept moment is checked rather than hard-coded to pass.
This checkpoint has no plugin action, performs no SPI write, posts no VCE or
DoF, and does not enable `stfeols`.

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
Prepared IID/robust score construction, weight-specific meat semantics,
finite-sample corrections, cluster intersections/nesting, residual DoF, and
fit-statistic parity remain later checkpoints.

## Current boundary

These checkpoints are scientific-core implementations, not a claim that
`stfeols` is production-ready. Remaining work includes FE coefficient recovery,
prediction, Stata data ingestion and result posting, weight-type parity,
remaining reference DoF/VCE policy, licensed-Stata differential tests, and
large-scale speed and memory gates.
