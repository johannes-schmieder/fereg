# Automatic AKM routing: post-change development check

This is development evidence, not qualification. The tested universal macOS
plugin has SHA-256
`84add0908986f4d36054a85f8550fbfbe742c027b4b928bcfd453aeb9356cfdf`.
The exact final rebuild, which differs only by lint annotations at internal
function boundaries, has SHA-256
`04cde2f5854e513567dd9551286090d42b01268e5748711c6dc7ab8a351e6be5`.
It separately passed the focused licensed route fixture and 12/12 recorded
auto smoke runs under `../solver-routing-final-build-smoke-v2/`.

The implemented categorical intercept-only rule is:

1. choose the two largest FE dimensions as CMG cores;
2. choose CMG when `N / largest_levels >= 3`,
   `largest_levels / second_levels >= 10`, and nuisance non-base columns are
   at most 9;
3. otherwise choose direct auto-LSMR (Schwarz on the tested AKM sizes);
4. retain certified CG only as a visible failure safety continuation.

The matrix contains 26 cases and four arms (auto, CG, explicit
Schwarz-LSMR at `1e-8`, and explicit CMG), with exactly three recorded
repetitions per cell: 312 runs total. Data were generated once per case and arm
order rotated across repetitions.

## Result

- All 78 auto runs returned `rc=0`, certified, and used no fallback.
- All categorical auto runs recorded zero MAP pilot iterations.
- Auto selected CMG in 39 runs and Schwarz-LSMR in 39 runs.
- RSS and the four recorded coefficients matched every eligible explicit arm
  at the precision stored in the summary CSV.
- The largest positive regret against an explicit arm occurred outside the
  intended AKM guard: 65 percent for an 8-period, 5:1 sparse design. A separate
  5-period, 5:1 holdout made explicit CMG much slower, confirming that a simple
  5:1 expansion is not safe.

## Former cliff

| N | third-FE levels | auto route | median seconds | fallback |
|---:|---:|:---|---:|:---:|
| 10,000 | 10 | CMG | 0.016 | no |
| 12,500 | 10 | CMG | 0.018 | no |
| 15,000 | 10 | CMG | 0.020 | no |
| 17,500 | 10 | CMG | 0.023 | no |
| 20,000 | 10 | CMG | 0.026 | no |
| 22,500 | 12 | Schwarz-LSMR | 0.030 | no |
| 25,000 | 13 | Schwarz-LSMR | 0.033 | no |
| 27,500 | 14 | Schwarz-LSMR | 0.036 | no |
| 30,000 | 15 | Schwarz-LSMR | 0.037 | no |

The route transition is visible but does not create a runtime cliff.

## Files

- `solver_routing_calibration_runs.csv`: raw run-level results.
- `solver_routing_calibration_summary.csv`: three-repetition medians.
- `routing_decision_table.csv`: best explicit arm, auto route, fallback, and
  regret by case.
- `cliff_table.csv` and `cmg_boundary_table.csv`: focused tables.
