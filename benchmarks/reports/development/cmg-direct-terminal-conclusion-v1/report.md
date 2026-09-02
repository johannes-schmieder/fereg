# Direct-terminal CMG routing conclusion

This is focused development evidence, not qualification.

| Stage | Datasets | Recorded runs | Result |
|---|---:|---:|---|
| Reliable CMG/CG calibration | 61 | 366 | Both arms certified without fallback; locked primary selected CG in 20 cells |
| Fresh CMG/CG holdout | 54 | 324 | Both arms certified without fallback; locked primary selected CG in 15 cells and displaced no CMG win above 5% |
| Post-change auto/CMG/CG replay | 54 | 486 | All routes matched; all auto runs certified; zero fallback cells; 1.339x aggregate improvement over all-CMG |
| No-control variance recheck | 2 | 18 | Both locked CG routes repeated; zero fallback; maximum auto regret 1.055 |

Every solver cell used exactly three recorded repetitions. Datasets were
generated once per benchmark case, all arms used the same data, warm-ups were
unrecorded, and arm order rotated.

## Decision

ADR-0023 implements the conservative candidate locked before the fresh
holdout. After the ADR-0021 eligibility screen and ADR-0022 iterative-terminal
guard, a direct terminal uses direct CG only when the earlier deterministic
cost score rejects CMG, hierarchy depth-work exceeds 200 per observation or
nuisance width reaches nine, and execution is serial or the initial RHS width
fits within the thread width. All other direct terminals retain CMG.

The planned CG route accounts for discarded graph/hierarchy setup, reports
requested solver `auto` and used solver `cg`, and has no warning, fallback, or
MAP pilot. Explicit solvers are unchanged.

Schwarz-LSMR was rejected as the destination. Per-RHS traces showed a marginal
certificate mismatch in one case, but tightening to `1e-11` caused a severe
runtime cliff in another common case. See `lsmr_diagnosis.md`.

## Conclusion

Stop adding routing thresholds. The conservative rule materially improves
aggregate time without a measured correctness or fallback regression, while
the more aggressive candidate adds three interacting conditions and is less
likely to generalize. Remaining performance work should target component-aware
CMG and reusable Schwarz setup. Reopen routing only after those implementations
change costs or a new representative workload demonstrates a reproducible
common regression.
