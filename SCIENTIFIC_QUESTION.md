# Scientific decision required: negative residual DoF in frozen absorbed-slope cases

## Why development must stop before changing estimator behavior

The next frozen semantic case is:

```stata
reghdfe y c.x1, absorb(firm##c.time worker)
```

The immutable pinned reference receipt
`benchmarks/reports/captured/darwin-arm64/stata18/ols/20260821T030000Z-a5181f9`
records for `intercept_and_slope`:

- `N = 24`;
- `df_a = 26`;
- `df_r = -2`;
- `x1` omitted/FE-collinear;
- a negative iid variance for the reported intercept.

The following frozen `slope_only_multiple` case likewise records negative
residual DoF (`df_r = -8`).

That behavior conflicts with the registered StFixest scientific contract:

- `docs/ols_core.md` defines `df_r = N_effective - model_rank > 0`;
- `PreparedOlsResidualDofReport.residual_dof` is an unsigned count;
- the certified iid composition returns `InsufficientObservations` when residual
  DoF is nonpositive;
- covariance, RMSE, adjusted fit statistics, and standard errors are published
  only with a positive variance scale.

Silently changing either side would be a scientific/compatibility decision, not
an ordinary implementation repair. Public estimators remain fail-closed and no
slope result has been activated.

## Decision alternatives

### A. Preserve the positive-DoF scientific contract and repair the frozen test design

Create a dedicated synthetic absorbed-slope fixture with enough observations
for positive residual DoF, keep the command shape and slope identities, recapture
and reseal the pinned reference, and retain a separate explicit failure test for
nonpositive residual DoF. The other frozen cases and their outputs remain
unchanged.

This is the recommended option because it validates real absorbed-slope
estimation without requiring StFixest to publish a negative variance.

### B. Match pinned reghdfe behavior exactly, including negative DoF and variance

Change residual-DoF types and the iid/statistics/certificate/result-wire
contracts to permit signed residual DoF and negative iid covariance in these
models, then reproduce the current frozen receipt exactly.

This maximizes literal reghdfe compatibility but weakens the existing
fail-closed numerical contract and requires an explicit user-facing policy for
such results.

### C. Implement only the slope solver/preparation layer and leave both frozen slope cases blocked

Add positive-DoF independent slope oracles and certified solver support, but do
not claim either frozen semantic case complete and do not advance their
projection status. Continue only after a later policy decision.

## Required user decision

Choose **A**, **B**, or **C** before any commit changes residual-DoF, covariance,
fit-statistic, frozen-manifest, or public compatibility behavior.
