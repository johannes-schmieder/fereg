# Bounded slope-pilot adjudication

Johannes requested continuation after pilot 7745818's three comparability
failures. ADR0225 defines a separate diagnostic attempt for its exact captured
three-FE absorbed-slope designs. The original estimator/controller 58284f76,
frozen benchmark identity, rejected pilot, timeouts and discrepancy receipts
remain unchanged. No production gate is waived by this preparation.

The direct oracle passed five local tests and additional N960/K10 OLS/Poisson
controls against the prior independent sparse full-design oracle. The latter
coefficient differences are 5.99e-15/3.53e-12, with identical independent FE rank 703.
A full-size local N61,440/K10 OLS boundary exercise passes with model rank 38,711,
FE rank 38,701, maximum scaled score 5.16e-15 and nuisance projection 2.33e-13.
Its discarded singular values are separated from the smallest retained singular
value 0.02376 by more than 11 orders of magnitude. This is a local generated-input
control, not an exact-byte adjudication of the SCC outputs or a headline timing.
The actual SCC adjudication must verify every original input hash before solving.

The original Python sparse oracle's global dense-rank limit is not removed.
The new instrument is separately bounded and tested; it eliminates the first
partition by group Householder QR and directly SVD-solves the remaining explicit
design. Poisson inference always gets a fresh factorization at final weights.
Tests cover weighted bread, full covariance, row ordering, rank/geometry refusal
and unsupported zero outcomes. Native/public solver source is unchanged.

The diagnostic job runs sequential references and oracles on one reserved node.
Only the two strict stock fixest PPML calls and one unchanged reghdfe OLS call
are repeated, each with 1,800 seconds for estimation. Known failing ppmlhdfe
initialization is not retried. Oracle processes each have a 1,800-second cap.
The total allocation is bounded by 5h on 28 E5-2680v4 cores with 3G/core; references
keep their recorded T28/T4 settings, and the direct SVD oracle uses four BLAS
threads. No numerical comparison combines clocks across jobs; new reference
results are scientific diagnostics only. A diagnostic completion marker alone
is not acceptance: inspect accounting, each independent solve, ranks, sample,
coefficients, full covariance and objectives before deciding a continuation.

Immutable launch/results will be appended as separate receipts. STATUS.md is
the sole mutable handover. This diagnostic neither qualifies the estimator nor
establishes a distributional performance result or a production gate exception.
