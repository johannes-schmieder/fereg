# Scientific decision: isolated signed residual DoF for frozen slopes

The scientific/API ambiguity is resolved. Preserve the frozen licensed
`reghdfe` behavior for absorbed-slope cases, including its conservative signed
residual DoF and resulting iid covariance, but isolate that compatibility
behavior from StFixest's ordinary positive-DoF inference types.

The immutable pinned receipt
`benchmarks/reports/captured/darwin-arm64/stata18/ols/20260821T030000Z-a5181f9`
records `df_r=-2` for `intercept_and_slope` and `df_r=-8` for
`slope_only_multiple`. The former is now executable in the hidden projection.

## Required architecture

- Ordinary certified iid, robust, and clustered inference continues to require
  positive residual DoF and its existing nonnegative covariance contracts.
- A distinct signed result/DoF/statistics type may reproduce the pinned
  convention only on an explicitly guarded absorbed-slope route.
- The guarded route must bind `model_rank=finite_rank+absorbed_rank`,
  `df_r=N_effective-model_rank`, RSS, bread, covariance formula, symmetry,
  sample, statuses, and solver certificates before publication.
- A conventional one-slope-per-category count is marked inexact unless a
  continuous-rank search has actually certified it.
- Independent explicit-dummy evidence remains authoritative for numerical
  design rank, collinearity, residuals, and RSS. It does not replace the pinned
  Stata-facing absorbed-DoF and covariance comparison.
- Unsupported weights, VCEs, DoF policies, slope shapes, or solver states fail
  closed and cannot fall back to the ordinary inference type.

The `intercept_and_slope` implementation established this separation at
`c8b2a884159b6c9941c40b27e033437f665442b2`; source
`6bbe04d857a293f0fa1fb0f617683be9ad5b4ea1` corrected and sealed the reference
inexactness metadata. Public estimators remain disabled. `STATUS.md` is the
authority for the current live gate order.
