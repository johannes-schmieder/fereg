# OLS scientific core

The Stata-independent OLS core backs public `fereg` 0.5.0. ADR-0006 requires
positive residual DoF; signed DoF and negative IID variance are ineligible.

## Pipeline

```text
validate and compact candidate data
→ factorize final FE/cluster topology
→ prune singletons and rebase recipes
→ build fast or scratch-backed low representation
→ certified FE residualization
→ rank-aware finite regression
→ reconstruct and re-absorb residuals
→ score, bread, and inverse certificates
→ centralized DoF and fit statistics
→ iid/robust/multiway-cluster covariance
→ residual, absorbed, and normalized saved-FE outputs
→ validate the complete native result
→ transactionally post in Stata
```

Every failure is terminal. No change-in-iterate or recurrence stop substitutes
for fresh projection, normal-equation, inverse, covariance, and fit
certificates.

## Absorption, rank, and intercept

The prepared model owns one compact sample, shared numeric/string categorical
codes, rebased factor recipes, and deduplicated FE/cluster topology. It supports
intercept, slope-only, intercept-plus-slope, multiple slope channels,
disconnected components, and empty/no-regressor models.

Finite regression preserves a canonical status for every coefficient slot.
Fast execution first attempts guarded diagonal-pivoted Cholesky, including
widths above 64 under ADR 0030. Every pivot must clear the stronger square-root
rank guard, and the result must pass the independent score and inverse checks.
Weak or rank-deficient designs use deterministic scaled column-pivoted
Householder QR, which owns omission decisions. Low execution retains QR.
Only the identified subspace is solved. The normalized reported `_cons`, when structurally
present, is

```text
mean_w(y) - mean_w(X) b
```

It is a normalization coordinate, not an additional rank direction. Pure
all-slope-only absorbed models therefore do not report a structural constant.

For raw matrices wider than 64 columns, ADR 0031 groups adjacent columns in
blocks of eight when enough blocks exist for the native worker team. Raw
maxima, scaled norms and compensated means retain their prior per-column
arithmetic and observation order. Scratch is fixed-size worker stack; narrower
raw matrices retain the ADR 0012 row-partial algorithm.

Low-memory categorical projection reuses the fixed LSMR operator and
preconditioner across response/regressor batches under ADR 0032. Every batch
still receives its independent FE certificate. Retained LSMR state is
released before finite QR and output recovery, or before any CG continuation.

## DoF, inference, and outputs

Centralized DoF combines effective N, finite rank, absorbed rank, nesting, and
the requested default/none/firstpair/continuous policy. Successful inference
requires `df_r > 0`. IID, HC1 robust, and one-/multiway clustered paths have
independent score, correction, subset, PSD/repair, Wald, and statistic
certificates.

Rust returns complete b/V/status, fit, DoF, solver, FE, cluster, subset,
certificate, sample, residual, absorbed, saved-FE, resource, and timing
structures. The public ado makes one substantive plugin call and posts only
after the receipt is complete. `predict`, `test`, and `estat summarize`
consume the posted public result.

## Memory and evidence

Fast and low modes implement the same scientific contract; low uses bounded
scratch-backed columns and scores. Checked dimensions, fallible allocation,
cleanup, and injected failure paths are described in `memory_model.md`.

Independent explicit-dummy/mathematical oracles are required alongside exact
pinned Stata differentials. Samples, statuses, errors, singleton identities,
DoF/VCE policy, and outputs are exact; registered tolerances apply only to
eligible numerical comparisons. Qualification state is recorded in
`STATUS.md`.
