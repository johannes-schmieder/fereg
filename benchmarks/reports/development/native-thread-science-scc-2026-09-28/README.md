# Current-source native consistency across threads

Read-only analysis of the **2026-09-28 16:23:39 UTC** partial sweep capture.
Package **161b9491 / c09eeab3…**, measurement fingerprint **fb2ad540…**.
No fits, compilation, profiling, timing reclassification or source changes.

All **12 available cross-thread comparisons** pass the registered coefficient,
SE, regressor covariance and objective tolerances, exact input/sample identity,
convergence, and native rank/DoF/sample-count consistency checks. This is
reproducibility evidence for completed fits, not an independent optimum oracle.
Missing and failed fits are excluded from this comparison and remain visible.

| Family | N / K | Warm repetitions compared | Selected native teams |
|---|---:|---|---|
| OLS degree two | 3,932,160 / 10 | 1 | 4→28 |
| OLS three FE | 3,932,160 / 10 | 1–4 | 4→28 |
| OLS three FE, wide | 983,040 / 50 | 1–2 | 4→28 |
| PPML three FE | 3,932,160 / 10 | 1–4 | **4→8** |
| PPML sparse two FE | 3,932,160 / 10 | 3 | 4→28 |

Every higher thread request is 28. The four PPML three-FE fits select eight
native workers, so they cannot support a 4→28 native-scaling claim.
The maximum regressor coefficient difference is **1.901e-15** (rounded upward).
All exported regressor SEs are numerically identical. Three coefficient/SE
CSV pairs are byte-identical; none of the full regressor covariance CSV pairs
are byte-identical, although all pass the unchanged numerical envelope.
Seven comparisons are OLS and five PPML.

The frozen controller's comparison is retained verbatim. Its same-node/task
condition remains false where appropriate; a separately named scientific
consistency result permits different execution locations because this audit
compares native output across thread settings. It does **not** make either
timing eligible or reinterpret a failed stock comparison. No ratios or speedup
claims are produced. The audit adds exact native DoF/rank checks and requires
the expected scientific checks to be present.

Scope is limited to the saved exports: requested regressors and their entire
covariance block, RSS/deviance, reported sample hashes and scalar metadata.
The systematic worker does not export the normalized constant, row residuals,
means or saved effects. Their correctness is not established here. Nor does
agreement exclude a common numerical or policy error in both native fits.

## Current failures remain visible

At the same capture: **73/568 groups** have terminal group receipts,
**173 applications pass**, **six time out**, **1,199 are missing**, and **50**
are explicit historical budget exclusions. There are **47 passing comparison
pairs and 48 scientific discrepancies**, plus six unavailable and 709 missing
pairs. These counts include retained pilot work. Live production scheduler
accounting is pending, so no four-repeat timing configuration is yet eligible.
The audit's outstanding integrity findings are missing terminal accounting,
not observed source or input mismatches.

Three native timeouts are the large degree-two OLS T4 case, seeds 8675309,
20260819 and 31415927, all on **scc-wd2** at 720 command seconds. The successful
seed104729 fit was on the larger-memory **scc-wj2**. This broadens the evidence
for the [registered routing hypothesis](../ols-timeout-memory-registration-scc-2026-09-28/README.md)
but does not prove it: the failed fits have no published solver/memory receipt.
No failed native fit was restarted.

Reference timeouts are strict large three-FE PPML (seed20260819, 2,400 seconds),
and default/strict three-FE PPML slopes N61,440/K10/T28 (seed20260819,
120/398 seconds). No unchanged reference was rerun. The 48 scientific
discrepancies are 29 OLS/default-stock and 19 PPML/default-stock pairs; no
completed strict-PPML comparison fails science in this snapshot. This does not
erase strict-stock timeouts, exclusions or pending comparisons, or independently
adjudicate the default-stock disagreements.

## Reproduction and evidence

`analyze.py` consumes the frozen run and its timestamped partial report, checks
the source/runtime/controller bindings, compares only captured successful warm
native outputs, and writes a new evidence file exclusively. Its source hash and
**225 input/export bindings** are in `evidence.json`. No N-sized input is loaded.
Scalar/matrix analysis completed successfully; there was no numerical estimator
test or new SCC compute job. The code and results are also preserved under
`/projectnb/welfgr/fereg/dev/20260928-thread-inventory-01/` with timestamp
`20260928T162339Z`. `partial-state.json` separates current failures from historical
budget exclusions and binds its original remote snapshot.

The full sweep **7766045/tasks 1–4** remains running. Queued diagnostic and
candidate jobs are unchanged. The next acceptance decisions depend on their
terminal evidence, not this partial capture. The broader speed and core-scaling
targets remain open.
