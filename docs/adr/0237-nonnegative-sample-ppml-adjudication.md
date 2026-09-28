# ADR 0237: certify a supplied PPML sample containing zero outcomes

Status: accepted for bounded development, 2026-09-28. Not qualification.

## Decision and scope

Add a separate offline entry, `two_fe_nonnegative_sample_accuracy`, to the
bounded two-categorical-FE accuracy instrument. It admits nonnegative outcomes
with at least one positive observation. The existing connected-ring and general
positive-outcome entries continue to reject every zero outcome. Keep the same
75,000-row/50-control/1,200-second-level limits, memory envelope, 60-digit outward
arithmetic, guarded Gram, positive curvature and exponential Newton-ball tests.
No production estimator, sample policy, acceptance tolerance or comparator
exception changes. `campaign_admission` remains false.

The new entry only certifies the supplied retained sample. It cannot authorize
row removal or declare agreement of two samples. An independent sample witness
and the exact retained original-row identity must accompany any interpretation
of an original sample discrepancy. A numerical full-design solve is an anchor
proposal, not the accuracy authority. Its new nonnegative entry also leaves the
old positive-only entry closed.

## Why the optimum bound applies

On the supplied sample, the negative Poisson objective, apart from constants,
is `sum(exp(d_i theta) - y_i d_i theta)`. Its gradient is `D'(mu-y)` and its
Hessian is `D' diag(mu) D`. Its exponential curvature, Hessian comparison and
convexity do not require strict positivity of y. Zero outcomes enter only the
fresh score residual; their contribution is enclosed as exactly `0 - mu`.
The ADR0029/0194 forest-flow score/projection energies, finite-curvature bound,
full Newton decrement and condition `lambda/sqrt(min(mu)) < log(2)/2` therefore
apply without a changed constant. The same Newton ball establishes a finite
stationary point and bounds finite coefficients and model-space predictors.
Positive profiled curvature establishes identified finite rank; positive FE
edge weights establish grounded FE rank within each component.

This argument must not be replaced by a small unscaled score. On a separated
zero group, decreasing its mean can make that score tiny without producing a
finite optimum. The unchanged curvature condition refuses that construction;
a test exercises both unit and 1e-20 proposed means.

A finite stationary point also excludes any remaining nontrivial Poisson
recession direction on this sample: such a direction has zero prediction on
positive-outcome rows and nonpositive prediction on zero rows, strict somewhere.
At finite positive means its objective directional derivative would be strictly
negative, contradicting stationarity. This is a retained-sample result only.

## Original zero-group witness

For the bounded original categorical two-FE diagnostic, independently identify
levels containing no positive outcomes and assign their intercept coefficients
minus one, with all controls and other effects zero. The resulting integer
observation direction is zero on positive and retained rows and strictly
negative on every removed row. It exactly certifies those zero-row exclusions.
Require at least two retained observations per surviving level; otherwise this
simple diagnostic refuses to claim singleton closure. Require the independent
finite-optimum certificate on the retained sample to exclude further separation.
Neither native nor reference e(sample) supplies the witness.

Connected/disconnected known optima use duplicate design rows with outcomes
`0` and `2*mu`, yielding opposite score residuals and a separately known finite
optimum. Full sparse Newton/sandwich results are also checked against the scalar
explicit-dummy oracle. Domain, invalid-data, perturbed-anchor, separated-group,
budget, row-permutation and singleton-refusal checks preserve fail-closed scope.
The source-bound [report](../../benchmarks/reports/development/ppml-original-zero-scc-2026-09-28/README.md)
records the original input, sample witness and adjudication. Point covariance
checks grant no optimum-covariance exception, and historical NR stays unchanged.
