# Scientific decision: option A for absorbed-slope residual DoF

Johannes explicitly selected **option A** on 2026-08-24. This supersedes the
short-lived isolated signed-residual-DoF compatibility decision recorded after
`c8b2a884159b6c9941c40b27e033437f665442b2` and
`6bbe04d857a293f0fa1fb0f617683be9ad5b4ea1`.

## Governing policy

- StFixest OLS inference continues to require strictly positive residual degrees
  of freedom before publishing covariance, standard errors, RMSE, adjusted fit
  statistics, or a completed estimator result.
- Negative iid covariance is not an eligible StFixest success result, including
  on absorbed-slope routes. A separate signed inference/result type is not part
  of the accepted architecture and must be removed.
- The useful absorbed intercept-plus-one-slope engineering may remain: guarded
  request planning, final-sample source binding, compact topology ownership,
  allocation-bounded certified MAP, and independent explicit-dummy numerical
  oracles.
- The frozen `intercept_and_slope` semantic case must be rebound to a dedicated,
  deterministic synthetic fixture with enough observations and support for
  positive residual DoF while keeping the command shape and identities:

  ```stata
  reghdfe y c.x1, absorb(firm##c.time worker)
  ```

- The repaired fixture must be recaptured and resealed against the exact pinned
  offline `reghdfe` dependency closure. Unrelated frozen semantic cases must not
  change.
- The former 24-row over-absorbed construction must remain covered separately as
  a fail-closed test: nonpositive residual DoF must produce the registered
  insufficient-observations failure and no logically complete covariance,
  statistics, output variable, or Stata `e()` transaction.
- Independent explicit-dummy evidence remains necessary for numerical rank,
  collinearity, residuals, RSS, covariance, and solver certificates; pinned
  Stata evidence remains necessary for the repaired Stata-facing metadata.

The candidate receipt that accepted signed `df_r=-2` and negative iid variance
is historical evidence only and is not the active scientific contract. Public
`stfeols`, `stfepois`, `estimate-ols`, and `estimate-ppml` remain disabled.
`STATUS.md` must be updated after the option-A correction is exact-green and
source-bound.
