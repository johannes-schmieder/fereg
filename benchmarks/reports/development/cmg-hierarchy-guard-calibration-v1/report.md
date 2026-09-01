# CMG hierarchy-guard calibration

This is development/prototyping evidence. It does not change automatic routing.

## Evidence

- Diagnostic plugin SHA-256: `3ec26149397f3d1c8f91a85c34f0970bcd4259a44300d2ce8ebc13275f00d104`.
- Guard calibration cells: 116; directly comparable certified CMG/LSMR cells: 109.
- The focused holdout recorded 333 runs. 330/330 successful runs certified without fallback; the three failures were deterministic explicit Schwarz-LSMR `r(459)` results in one cell and remain ineligible. All successful arms matched exactly on the exported coefficient/RSS precision, and every measured component count matched construction.
- Every focused timing cell has exactly three recorded repetitions, one unrecorded warm-up per arm, rotated arm order, one generated dataset shared by all arms, tolerance `1e-8`, 16,000 maximum iterations, four controls, and eight threads.
- Existing balanced and controlled timing rows also have exactly three repetitions; only the deterministic hierarchy profile was added.
- Focused graph-plus-hierarchy construction cost had a 0.0037-second median and 0.0046-second maximum.

## Candidate guard

After the existing cheap AKM topology screen accepts CMG, build only the graph and CMG preconditioner. Compute

`W = sum_l visits_l * (vertices_l + matrix_nonzeros_l) / (vertices_0 + matrix_nonzeros_0)`,

where `visits_0 = 1` and each later visit count is the product of upstream effective repeat counts.

The concrete candidate is: if the terminal reason is `Direct`, do not apply a hierarchy safety veto; if the terminal is iterative, continue CMG only when `W <= 8`. Otherwise discard the graph/hierarchy before nuisance-Schur construction and PCG and run the existing direct auto-LSMR route with certified CG continuation. The planned veto is routing, not a numerical fallback.

This rule rejected 17 of 109 comparable cells, retained every measured CMG win, and left one >2x CMG loss; that remaining loss had a direct terminal and nine nuisance columns, so it is a separate RHS/nuisance cost-model issue rather than the recursive hierarchy cliff. Its aggregate route/best ratio was 1.075, versus 1.050 for an unconditional `W <= 6` veto that falsely rejected 11 measured CMG wins.

The separation is unusually clean but still needs a holdout: all measured iterative-terminal CMG wins had `W <= 2.30`, while the first measured iterative-terminal loss had `W = 22.44`. Thresholds from 4 through 20 therefore make the same in-sample hierarchy decision. Eight is the conservative round candidate, not a fitted constant.

## Severe current CMG choices

| Case | Population | W | CMG/LSMR | Terminal | Levels |
|:---|:---|---:|---:|:---|---:|
| guard_n99_one_large_c1000_y10 | focused_holdout | 82.26 | 28.30 | StagnatedVertexReduction | 6 |
| guard_n99_one_large_c1000_y5 | focused_holdout | 82.26 | 22.26 | StagnatedVertexReduction | 6 |
| guard_n99_one_large_c0800_y10 | focused_holdout | 60.01 | 19.99 | StagnatedVertexReduction | 7 |
| guard_n99_two_large_c1000_l2 | focused_holdout | 124.72 | 16.09 | StagnatedVertexReduction | 6 |
| guard_n99_one_large_c0800_y5 | focused_holdout | 60.01 | 15.64 | StagnatedVertexReduction | 7 |
| guard_n99_one_large_c1000 | focused_holdout | 82.26 | 12.19 | StagnatedVertexReduction | 6 |
| comp_one_large_c1000_k4_t8 | controlled_existing | 82.26 | 11.38 | StagnatedVertexReduction | 6 |
| guard_n99_one_large_c0775 | focused_holdout | 65.89 | 8.81 | StagnatedVertexReduction | 7 |
| guard_n99_one_large_c0800 | focused_holdout | 60.01 | 8.42 | StagnatedVertexReduction | 7 |
| guard_n99_one_large_c0750_y10 | focused_holdout | 51.50 | 7.10 | StagnatedVertexReduction | 7 |
| guard_n99_one_large_c0750 | focused_holdout | 51.50 | 7.02 | StagnatedVertexReduction | 7 |
| guard_n99_one_large_c0725 | focused_holdout | 44.77 | 6.64 | StagnatedVertexReduction | 7 |
| guard_n99_one_large_c0750_y5 | focused_holdout | 51.50 | 6.53 | StagnatedVertexReduction | 7 |
| guard_n99_one_large_c0700 | focused_holdout | 38.34 | 6.25 | StagnatedVertexReduction | 7 |
| guard_n99_one_large_c0900 | focused_holdout | 94.37 | 5.97 | StagnatedVertexReduction | 6 |

## Interpretation

A terminal-aware hierarchy-work guard directly targets the repeat-amplified work that caused the disconnected-component cliff. Component count and largest-component share remain useful diagnostics but are not guard variables: all-tiny graphs can have many components, an iterative terminal, and very low hierarchy work. An unconditional work ceiling is also too blunt because several realistic sparse graphs have direct terminals, `W` between 6 and 20, and strong CMG wins.

The one direct-terminal severe loss was `guard_n99_one_large_c0600_y10` at CMG/LSMR 2.28. A possible second-stage nuisance-load screen catches it, but the current sample shows false rejections of balanced nuisance designs. Calibrate that screen separately using graph-solve batches and RHS width rather than weakening the hierarchy guard.

Before implementation, rerun the candidate band `{6, 8, 12, 16}` on fresh component mixtures and seeds, one/eight threads, and 0/4/16 controls. The implementation should retain the existing memory and scientific certificates, count rejected hierarchy setup in runtime, release CMG allocations before LSMR preflight, and report the ultimate used solver without labeling the planned veto as a numerical fallback.
