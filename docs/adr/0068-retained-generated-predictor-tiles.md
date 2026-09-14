# ADR 0068: retained generated predictor tiles

**Status:** Accepted for local development
**Date:** 2026-09-14

## Decision

Fresh and strict retained weighted-linear solves may accumulate generated finite
predictors in 128-row tiles when the final sample has at least 4,096 rows, the
model has at least eight generated columns, and a categorical recipe is present.
Borrow final ordinary or topology-backed factor codes through the shared prepared
view. Continuous, categorical and interaction recipes fill one fixed 128-value
stack bank. There is no upper-width restriction or new retained allocation.

Keep the original coefficient-slot FMA sequence for each observation, including
FMA on zero indicators. Use only the existing installed worker team and native
data. The numeric specialization in ADR 0066 retains precedence. Smaller and
narrower models retain their prior path. This applies to resident and low-memory
retained sessions; ordinary frozen PPML and one-shot OLS are unchanged.

The absorbed predictor remains `(response - residual) - finite_predictor`, with
the original independent reconstruction certificate. Do not share the helper with
ordinary OLS subtraction without preserving its different absent-indicator
arithmetic. Sample, weight-generation validity, rank, final-Hessian checks and
transactional publication remain authoritative.

## Evidence and limitations

The [source-bound report](../../benchmarks/reports/development/generated-predictor-tiles-local-2026-09-14/README.md)
records 5.9%/8.4%/6.6% median paired command savings for generated FW/unit/PW
targets. Separate original-input diagnostics locate 99.3 ms of FW savings in
three finite-predictor accumulations, with independent certification unchanged.
Primary native peaks and budgets are exact; no general memory saving is claimed.

Twenty-three successful untraced pairs preserve exact scientific and non-time
solver fields. Fourteen successful protected controls include low memory,
generated widths, weak graphs, slopes, three FE and saved outputs. The largest
added command cost is 6 ms on a small numeric model.

The original topology-backed, exactly rank-deficient control fails with r(430)
on both artifacts, in both warm and full fits: the final Hessian changes the
canonical basis. Preserve these failures and the unchanged no-publication check.
They provide no successful public coverage or performance result for that case;
the tile's topology/rebased-code arithmetic has a separate raw-level unit oracle.
No rank policy or tolerance is relaxed to admit it.

This is local development adoption. Broader generated OLS residual evaluation,
shared slope/three-plus-FE layouts, scheduling and remaining PPML state work stay
open under issue #27.
