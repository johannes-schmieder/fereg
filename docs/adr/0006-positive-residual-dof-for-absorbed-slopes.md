# ADR 0006: Require positive residual DoF for absorbed-slope OLS

**Status:** Accepted  
**Date:** 2026-08-24  
**Decision owner:** Johannes Schmieder

## Context

The original frozen `intercept_and_slope` fixture used 24 observations with:

```stata
reghdfe y c.x1, absorb(firm##c.time worker)
```

Pinned `reghdfe` reported conservative absorbed DoF large enough to produce
`df_r=-2` and a negative iid variance for the reported intercept. The following
`slope_only_multiple` fixture likewise produced negative residual DoF.

A temporary development route introduced a separate signed residual-DoF result
type and reproduced that output. It was useful compatibility evidence, but it
conflicted with the registered StFixest invariant that successful OLS inference
requires positive residual degrees of freedom and a valid variance scale.

Johannes selected option A: repair the test design rather than weaken the
scientific contract.

## Decision

1. Successful StFixest OLS inference requires `df_r > 0`.
2. Negative iid covariance is not an eligible success result. A separate signed
   inference/result type is not part of the accepted architecture.
3. The useful absorbed intercept-plus-one-slope engineering may remain:
   request planning, final-sample source binding, compact topology, certified
   allocation-bounded MAP, coefficient/rank handling, and independent
   explicit-dummy oracles.
4. The frozen `intercept_and_slope` command shape and identities are retained,
   but the case is rebound to the deterministic 48-row fixture
   `tests/fixtures/ols/ols_intercept_slope_positive_dof.csv`.
5. The 48-row fixture repeats the original topology and supplies enough support
   for positive residual DoF under the conservative pinned absorbed-DoF count.
6. The original 24-row construction remains a separate fail-closed regression
   test. It must return the registered insufficient-observations error and leave
   covariance, statistics, requested variables, and Stata `e()` state
   logically uncommitted.
7. The repaired fixture must be recaptured and resealed against the exact pinned
   offline `reghdfe`, `ftools`, and `require` closure. Unrelated frozen cases
   must remain byte- and result-stable except for manifest hashes that
   necessarily bind the changed fixture inventory.
8. Independent explicit-dummy evidence remains mandatory for rank,
   collinearity, residuals, RSS, covariance, and solver certificates.
9. Public estimators remain fail-closed until all later gates close.

## Consequences

- `f61fee923db2bd3544d457dfe50de8117d27adda` remains the latest accepted
  exact-green source under this policy until the option-A correction receives
  new exact-SHA Rust and licensed-Stata evidence.
- The signed-DoF commits and their receipts remain immutable historical
  evidence but cannot be used as active acceptance evidence.
- `ci/source-transforms/apply_option_a_absorbed_slope.py` is a guarded migration
  aid, not an acceptance artifact. It must be reviewed, applied locally, tested,
  and deleted once its coherent source correction is committed.
- Documentation or code that still describes signed residual DoF or negative
  iid variance as an accepted success path is superseded by this ADR.
