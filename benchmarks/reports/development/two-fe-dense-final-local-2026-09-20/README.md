# Dense two-FE local target met

Both independent five-pair blocks pass all 24 cells: twenty primary inference/
output cases and four fresh-seed/order holdouts. Every cell wins ten of ten
pairs (240/240 total). The per-cell, per-block median paired fereg/stock-fixest
ratios range from **0.356625 to 0.891304**, meeting the unchanged 0.90 limit.
The minimum measured reduction is 10.9%; primary-case reductions are 32.5–64.3%.
No run is removed and no regression confirmation is needed.

This is a local Mac development result on the M2 Ultra, N=983,040, K=10/50,
and sixteen requested native workers. The dense generator has five observations
per worker and 20% mobility. The stock fixest reference is pinned at 0.14.2.
Data-file import and audit export are outside the timers; Stata-to-native input,
estimation, inference and the requested outputs are inside. Separate instrumented
profiles and tightened-reference timings never replace headline stock timings.

## Final timings

Milliseconds below are medians over all ten final pairs per primary cell.
Acceptance uses the median of paired ratios separately in each five-pair block,
not the ratio of these pooled medians.

| Request | fereg K=10 | fixest K=10 | fereg K=50 | fixest K=50 |
|---|---:|---:|---:|---:|
| IID | 265.0 | 425.5 | 658.0 | 1499.0 |
| Robust | 281.0 | 453.5 | 776.5 | 1905.0 |
| FE-level cluster | 277.5 | 464.0 | 756.5 | 1989.5 |
| Crossing cluster | 290.5 | 470.0 | 776.0 | 1869.5 |
| Two crossing clusters | 316.5 | 539.0 | 846.5 | 2348.0 |
| FE-level + crossing clusters | 302.5 | 529.5 | 852.5 | 2339.5 |
| IID + saved FE | 383.5 | 619.0 | 777.5 | 1710.5 |
| FE cluster + saved FE | 416.5 | 657.5 | 889.0 | 2188.0 |
| Two clusters + saved FE | 448.0 | 743.0 | 960.5 | 2554.5 |
| IID + full outputs | 415.0 | 618.5 | 828.5 | 1686.0 |

The previously failing shuffled K=10 holdout passes with median paired ratios
**0.891304** and **0.876133**. Its command medians are 285/322 ms and 283/324 ms.
The ordered holdout maxima are 0.621496 and 0.619718. Exact per-pair values,
reference fingerprints and all block decisions are in `acceptance-audit.json`.

## Regression and scientific evidence

All 41 three-pair controls against committed one-FE endpoint `637f2407` pass:
maximum cell ratio **1.034483**, geometric mean **0.995921**. They cover twenty
one-FE inference/output cells, eleven one-FE size/thread/layout cells, eight
lower-thread dense two-FE cells and two sparse two-FE cells. All 123 checked
native result bundles are byte-exact and all 123 native allocation peaks equal
the baseline. All 24 final cells repeat their checked native outputs exactly
across ten calls. See the numerical/memory and repeatability receipts.

The archive contains **742 successful application calls**: 556 in the main
measurements, 120 in `one-fe-breadth/`, and 66 in `one-fe-controls/`. Scientific
comparison status is recorded separately from application success. Default
fixest saved outputs miss the unchanged output tolerance in all forty saved/
full-output comparisons in each primary block. Those 80 discrepancies remain
explicit; they are not labeled passing output parity. The earlier investigation
localizes this to reference estimation precision. Separate fixest estimation at
1e-11 passes all eight output cases, while an independent native audit verifies
constant-within-group FE, component normalization, reconstruction, group residual
means and finite-regressor scores. Maximum native group residual mean is
2.602e-13 and maximum score per observation is 1.707e-16. Native output precision,
reference timing tolerances and all statistical policies remain unchanged.

The final artifact passes twelve Schur tests, seven outcome-overlap tests and
two graph-recovery tests; these include the final-projection overlap boundary.
The actual-plugin ABI/panic containment check and all three completed-artifact
Stata fixtures pass. The fixtures include two-/three-FE dummy oracles, inference,
saved outputs, alternate loading, missingness, cascading singletons, string
fallback, supplied weights, restrictions, low memory and transaction rollback.
The unit checks and local fixtures are focused development evidence.

## Source and implementation

Plugin SHA-256:
`4e8e640ec2fb50ec1d7ca8b07066281a3be531ebdda2887fa1d72ccdcc31599e`.
All 513 captured source entries match the frozen implementation. The build uses
baseline `637f2407` plus the explicitly listed overlays; its embedded development
build date is 2026-09-19, while these measurements occurred on 2026-09-20. One
post-freeze comment word was restored to the built source before the campaign.
No executable source changed between the final blocks or regression controls.

The retained changes are described in
[ADR 0075](../../../../docs/adr/0075-dense-two-fe-ols-data-and-solver-passes.md):

- Extend checked numeric loading, loaded RHS/raw moments and independent outcome
  overlap to eligible ordinary/intercept multi-FE OLS.
- Admit narrow unit-weight Schur fits only behind the existing contraction probe,
  memory limits and certificates; reuse accepted graph admission for a fresh,
  strict final one-RHS solve.
- Reuse idle scratch for counted neighboring edges, parallel construction and
  bounded certificate reductions; preserve the independently checked operator.
- Recover eligible two-effect outputs by certified graph propagation and permit
  existing final-projection/bread/score overlap when its owners are already admitted.

There is no new heap owner for counted edges or the overlap scheduling. Scientific
checks remain independent, and weighted, low-memory, explicit-solver, slope and
out-of-envelope paths retain their respective guards and fallback policies.
These measurements do not establish speedups for every graph, width, weight,
thread count, PPML or platform. No installed package, remote state, release or
cross-platform qualification changed.

## Preserved development history

The [initial investigation](../two-fe-dense-initial-local-2026-09-20/README.md),
[strict final projection](../two-fe-dense-finish-local-2026-09-20/README.md),
[failed shuffled holdout](../two-fe-dense-shuffled-holdout-local-2026-09-20/README.md),
and [operator/overlap increments](../two-fe-dense-operator-local-2026-09-20/README.md)
retain earlier artifacts, failures and attempts. In particular, the preceding
`densecore` candidate failed the shuffled narrow target at 1.038690 with zero
wins. Its stopped campaign and the initial overly tight absolute unit comparison
remain visible. The successful final campaign does not rewrite that evidence.
