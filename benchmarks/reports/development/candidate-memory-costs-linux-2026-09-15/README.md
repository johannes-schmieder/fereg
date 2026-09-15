# Linux memory-accounting cost check

The fixed development list passes **30 alternating baseline/candidate pairs
(60 fresh-process commands)** and all **11 licensed Linux fixtures**. Samples,
row order, scientific matrices, saved outputs and non-time solver fields match
exactly within every pair. No median slowdown exceeds both 5% and 10 ms.
These are bounded development comparisons, not performance qualification.

Baseline is `9ecf47d`; the accounting snapshot is bound to its complete source
inventory and isolated Linux plugin in `identity.json` and `validation.json`.
Build/ABI job 7586340, comparison job 7586345 and fixture job 7586355 have
failed=0/exit=0, explicit fixture return codes and terminal sentinels. The
comparison was serial on one allocated compute node, with no task-owned build
or fixture process overlapping it. Both arms used the same hashed input.

| Case | Baseline median (s) | Accounting median (s) | Change |
|---|---:|---:|---:|
| small_narrow | 0.025 | 0.025 | +0.00% |
| wide_one_fe | 1.269 | 1.273 | +0.32% |
| strong_two_fe_cmg | 1.059 | 1.060 | +0.09% |
| weak_wide | 6.003 | 5.982 | -0.35% |
| tiny_weighted_bridge | 0.290 | 0.278 | -4.14% |
| three_fe_fw_generated | 0.557 | 0.538 | -3.41% |
| absorbed_slopes | 0.436 | 0.437 | +0.23% |
| saved_generated_aw | 3.568 | 3.548 | -0.56% |
| ppml_german_small_fast | 38.362 | 38.218 | -0.38% |
| ppml_german_small_low | 19.994 | 19.961 | -0.17% |

The small German PPML input preserves all 75,350 original subset rows and
retains exactly 65,503. Its only transport changes are literal variable renames;
the model remains `exp(logrwage2)` with the same controls and year indicators.
`strong_d2` uses the existing dense two-FE input recipe, declared before timing.
Archived generated-column, bridge, slope and saved-output inputs match their
frozen hashes. No external reference estimator was rerun.

`paired-results.json` retains complete command and phase times, native allocation,
base/selected models, sampled RSS, scratch and solver/retry reporting. Discarded
attempt time is included in elapsed times but is not separately observed; it
remains explicitly null. Timing differences this small are not speedup claims.

The original three-FE reporting discrepancy reproduces: the first accounting arm
observes 50,298,672 native bytes against 22,983,168 public base bytes (historical
Mac values: 50,265,168 versus the same base). This does not establish a cap breach:
solver storage is separately admitted and still absent from that public field.
The preserved German OLS base-plus-CG reconciliation remains in the independent
adjudication report. A new German OLS action-memory reproduction is still open.

The [native checkpoint](../candidate-memory-isolated-2026-09-15/README.md)
provides 979 source-bound library checks. Subsequent formatting of owned changes
has identical canonical rustfmt output; that proof does not relabel the tested
source or artifact. The added larger singleton fixture is separately bound.

Complete-action coverage and versioned public coverage/peak diagnostics remain
unimplemented. Mac licensing/universal/Rosetta checks, the proposed German OLS
adjudication decision, final candidate freeze, deterministic review archives and
extracted-package installation gates remain open. These receipts do not promote
the implementation to a production candidate or close roadmap release gates.

Portable copies normalize execution-root paths. Literal executed harness and
log hashes remain recorded; raw licensed Stata logs and data are not committed.
