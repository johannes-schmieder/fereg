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

## Covariance estimators

The current core supports:

- conventional homoskedastic covariance;
- HC1-style heteroskedastic covariance;
- one- through four-way clustered covariance using deterministic score
  aggregation and inclusion-exclusion over cluster intersections.

The core consumes effective least-squares weights. Stata weight types must be
converted according to the frozen weight-semantics contract before this layer.
Cluster intersections are encoded as tuples rather than concatenated strings.
Any required cluster subset with fewer than two groups fails closed.

## Degrees of freedom

The default absorbed-rank count is exact for one- and two-way intercept-only
fixed effects, including disconnected graphs. Higher-way categorical designs
and absorbed-slope designs use a conservative coefficient-slot upper bound and
set `absorbed_rank_is_approximate=true`. Exact reference-compatible higher-way,
nesting, and slope accounting remains a later release gate; callers can inspect
the flag and must not silently present the approximation as exact parity.

## Current boundary

This checkpoint is a scientific-core implementation, not a claim that
`stfeols` is production-ready. Remaining work includes FE coefficient recovery,
prediction, Stata data ingestion and result posting, weight-type parity,
reference DoF policy, licensed-Stata differential tests, and large-scale speed
and memory gates.
