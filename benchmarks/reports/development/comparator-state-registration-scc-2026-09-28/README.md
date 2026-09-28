# Comparator state: bounded diagnostic registration

This registers a diagnostic of two unresolved reporting issues from the original
SCC campaign: PyFixest PPML standard errors and gtools' missing exact samples.
It does not change an estimator, a maintained worker, a scientific envelope or
the live current-source sweep. No new timing comparison is authorized by this
receipt. Numerical findings belong in a separate completion report.

## Source findings and hypotheses

The audit uses the exact original pinned runtime: PyFixest 0.60.0 at
`b6f2d284adf033f4b05a40db45a383784f73afd2` and gtools at
`9dcbb835e9c8a7b30f037cdf1ec7de7f8c6ad30f`. Each inspected file is checked against
the original frozen runtime inventory. The source bindings and locations are
recorded in `source-audit.json`.

PyFixest's heteroskedastic SSC with `k_adj=False` is one. Its `G_adj` branch applies
to CRV/HAC, not heteroskedastic inference. The existing worker's explicit
`N/(N-1)` correction therefore does not duplicate that factor in this pinned
implementation. Do not change the normalization based on the earlier hypothesis.

The IRLS loop updates the fitted mean after its weighted solve, saves that solve's
weights and projected design, and checks convergence before another solve. It
returns the updated mean and the preceding solve's weight frame. The model builds
the robust scores and Hessian from those saved weighted-linear objects. This
establishes a possible source of inference error; it does **not** establish the
size or cause of any observed discrepancy without a numerical comparison.

Gtools' pinned `GTOOLS_GREGTABLE=1` branch posts its own `touse` as `e(sample)`
along with `e(b)` and `e(V)`. The existing worker leaves that branch disabled and
exports only the observation count. Enabling it adds e-class publication/table
work inside the command, so successful extraction would require a newly identified
worker and newly measured timings. The internal global is a pinned diagnostic
setting, not a supported cross-version API. Historical count-only receipts remain
unranked. Separation/sample comparability remains a distinct PPML question.

## Predeclared checks

[registration.json](registration.json) binds 26 original exports, the original
fingerprint, generator, unchanged scientific envelopes and all settings.

* Original positive-outcome one-FE and dense two-FE PPML, N61,440/K10, seed104729:
  regenerate the exact original DTA and compare its complete input receipt.
* Fresh counterparts, N15,360/K6, seed2026092837, declared before measurement.
  This seed is reserved for validation and must not become a tuning seed.
* Each input gets three unmodified public `fepois` calls: default; outer
  `iwls_tol=1e-12`; and outer `1e-12` with public
  `MapDemeaner(fixef_tol=1e-11)`. The maximum iteration count stays unchanged.
* An independent full sparse-design Poisson/Hessian oracle and outward-rounded
  coefficient certificate anchor every case. Separate scalar full-dummy and
  known-optimum checks validate the oracle. Covariance checks are numerical point
  comparisons, not certified optimum covariance intervals.
* At each PyFixest result, reconstruct weighted projections directly under both
  stored and current weights. Compare Hessians, old linearized scores, fresh
  current scores, reconstructed covariance and the independent optimum. The
  reconstructed covariance is diagnostic; it never replaces stock output.
* Preserve application failures and every failed scientific check. Assess the
  captured original native/default/strict/PyFixest exports independently without
  rewriting their historical eligibility.

The licensed gtools fixture covers one-FE OLS; two-FE OLS with missing values and
`if`/`in`; clustered OLS with missing cluster IDs; and positive two-FE Poisson
with missing values and `if`/`in`. It requires exact row IDs against explicit
eligibility, agreement among `r(N)`, `e(N)` and the sample count, bitwise unchanged
Mata coefficients/covariance across publication settings, and identical Mata and
e-class matrices. An empty-sample call must leave no posted coefficient matrix.
These are extraction checks, not a new gtools scientific certification.

## Execution and limits

Job **7766146**, four slots, 3G per slot, 30 minutes, using the established pinned
Python and Stata environments. The scheduler dependency is the entire current-source
array **7766045**, so this diagnostic cannot compete with its headline timings.
If that array leaves pending groups, wait for this short diagnostic to finish
before submitting further headline work, or hold that work behind the diagnostic.
No runtime/reference package is installed or modified. Stata uses a private
copy of its reference closure.

`deployment.json` binds all 12 staged files. Python AST and shell syntax checks
pass locally; licensed and scientific runtime validation remain pending behind
the dependency. `submission.json` records the accepted scheduler job and hold.
Acceptance will require accounting, both application sentinels, expected case
outputs, original-input hashes and the licensed fixture receipt. An application
exit of zero alone is insufficient. No performance or bug-resolution claim is
made at registration.
