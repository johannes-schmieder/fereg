# ADR 0161: one categorical coordinate sweep for cold multi-FE PPML

Status: accepted for local development, 2026-09-24.

## Decision

For cold publication fits with at least two categorical intercept-only FE
partitions, improve the existing global starting predictor with one cyclic
Poisson coordinate sweep. This supersedes ADR 0016's global-only initialization
for that scope and ADR 0028's multi-FE exclusion. Explicit warm starts, Mu
exploration, slopes, one-FE and no-FE initialization keep their existing behavior.
There is no change to solver routing or a new iteration/tuning parameter.

In declared partition order and deterministic group membership order, compute

    delta_g = log(sum_g q_i y_i) - log(sum_g q_i exp(eta_i))
    eta_i   = eta_i + delta_g, for i in g.

Use separate compensated raw sums. Refuse a positive term lost through
underflow, a nonfinite product/sum, a group with no positive weighted outcome,
or an updated predictor/mean outside the existing finite bounds. On refusal,
overwrite the complete provisional predictor with the original global start,
including groups already changed. Do not clip values or change separation.

## Mathematical and ownership boundary

Holding all other coefficients fixed, the group log-likelihood derivative is
sum_g q_i(y_i-exp(eta_i+delta)). The stated delta sets it to zero; the second
derivative is negative when the fitted group mass is positive. Disjoint groups
within a partition therefore have independent exact conditional updates. Each
update adds a categorical intercept and preserves the declared model space,
including offsets. A complete sweep need not solve the joint FE problem.

This is an initial guess, with nonconstant finite coefficients still zero.
It is not projection, rank or convergence evidence. The first canonical
full-width WLS solve retains rank/basis authority. All changing-weight solves,
line search, two consecutive deviance/independent-score passes, strict witness,
canonical final rank/bread, exact final Hessian, reconstruction, inference and
transactional publication remain mandatory at their unchanged tolerances.

Reuse the already admitted initial predictor and finalized memberships. There
is no extra heap, cache, thread team, observation-sized bank or admission term.
The sweep runs before any accepted IRLS state or warmed allocation window exists.
It does not share state across samples, weights or generations. No geometry from
the initial point authorizes a later solve.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-multife-initialization-local-2026-09-24/README.md)
contains a current-source opportunity inventory, separate shallow/deep profiles,
the exact candidate, independent oracles, full distributions and losses.
All 48 prepared-IRLS tests and four licensed fixtures pass. New tests recover an
independently prescribed separable two-/three-FE Poisson optimum with paired
zero-score deviations and weight scaling from 1e-50 to 1e50; they check objective
improvement, group scores, bounds/overflow refusal and complete restoration.
Existing strict/rank/rollback/dynamic-range tests remain green.

The independent explicit-dummy fixture covers two/three FE, unit/FW/PW,
robust/clustered inference, T1/T4, offsets and saved outputs. The fixed-process
low-memory allocation fixture retains zero events/growth and warm/fresh parity.
Slope and one-FE fixtures protect exclusions. All 144 headline applications,
132 public PPML state checks, 36 native PPML pairs, 36 direct candidate-stock
comparisons and six OLS pairs pass; OLS scientific exports remain exact.

All 15 development categorical pairs improve. Fresh narrow, disconnected,
unbalanced three-FE and wide-low medians improve 14.0%, 19.1%, 5.0% and 12.7%;
all 12 pairs improve. Most cases drop from six outer iterations to four.
The fresh weak case loses 1 ms at its median and one pair loses 6 ms; it still
clears the stock target by a large margin. An excluded development slope
control loses 1 ms at its median. Do not tune sweep counts or topology guards
to these observations. Further work targets canonical construction costs.

All modeled selected/scratch bytes remain unchanged. Tracked peak differences
range from -1,327,056 to +25,136 bytes; 18/36 pairs are exact. Different solve
histories can change temporary peaks; no universal memory reduction is claimed.
Fresh seed 202609374 is consumed/frozen. The scoped 0.90-times-stock goal remains
unfinished, and existing scientific/platform deferrals are not repaired here.
