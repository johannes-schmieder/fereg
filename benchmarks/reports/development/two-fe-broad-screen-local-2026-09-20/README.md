# Broad two-way OLS: first complete primary screen

Local Mac development on 2026-09-20. The predeclared 77 primary cells all ran,
with one warmed command per fresh application process and no process failures.
This is a screening block, not the required two independent five-pair blocks.

Native source is `455150af`, plugin
`8ec5df3a1686c9f476dcf2a83b63318ee252f789b1e49b75c624d5300d76efb2`.
All 514 captured source entries match that commit. The artifact identity retains
its original build base plus overlay; `source-audit.json` independently checks
the resulting committed source. The stock fixest library, its source archive,
controller and shared input hashes accompany every call.

## Runtime findings

There are **69 strict wins, one tie and seven losses** against default stock
fixest: seventy cells at parity or better in this single-pair screen. No losing
cell is excluded. Ratios range from 0.038061 to 1.257576.

| Family | Cells | Wins | Ties | Worst fereg / fixest ratio |
|---|---:|---:|---:|---:|
| Ordered dense widths, unit/AW | 12 | 12 | 0 | 0.7872 |
| Dense AW inference/outputs | 14 | 14 | 0 | 0.9044 |
| Dense PW inference/outputs | 8 | 8 | 0 | 0.9048 |
| Shuffled unit | 3 | 1 | 0 | 1.0973 |
| Shuffled AW | 4 | 3 | 0 | 1.2475 |
| Other two-way graphs | 36 | 31 | 1 | 1.2576 |

The seven losses, with command time in milliseconds:

| Design | K | Weight | fereg | fixest | Ratio |
|---|---:|---|---:|---:|---:|
| Degree six | 6 | AW | 83 | 66 | 1.2576 |
| Shuffled dense | 6 | AW | 378 | 303 | 1.2475 |
| Degree six | 6 | Unit | 70 | 62 | 1.1290 |
| Shuffled dense | 5 | Unit | 282 | 257 | 1.0973 |
| Unbalanced | 6 | AW | 90 | 84 | 1.0714 |
| Degree three | 6 | AW | 93 | 88 | 1.0568 |
| Shuffled dense | 1 | Unit | 232 | 224 | 1.0357 |

Unit degree-three K=6 ties at 76 ms. All K=50 primary cases win, as do all
ordered dense cases. Narrow shuffled full outputs are near parity; their
reference accuracy disagreement remains explicit. Single-pair fluctuations
must not be treated as repeated acceptance or used to remove a case.

## Scientific results

The application layer succeeds in all 154 calls. Comparison receipts contain
53 passes, eight default-reference output mismatches and sixteen default-
reference coefficient mismatches. These mismatches are not relabeled passes.

The independent grounded sparse-direct oracle checks all 36 other-graph cells
at both K=6 and K=50, including native and reference coefficients, IID covariance,
RSS and effective N. All native results pass, with maximum coefficient error
about 1.4e-12. The same sixteen default reference results fail; their errors
reach 1.79e-5. All eight independent native saved-output audits pass their
weighted group normal equations, component normalization, reconstruction and
RSS checks. The prior
[graph/science report](../two-fe-broad-graph-science-local-2026-09-20/README.md)
contains the direct-oracle self-check, small explicit-dummy/expansion checks,
reference DoF adjudication, and separate tight-reference graph calls, including
four nonconverged references. Earlier weighted finishing reports retain the
separate matched-accuracy output comparisons. None of those slower or failed
reference times replaces the default benchmark target here.

## Archive and next work

`broad-screen-plan.json` fixes the expanded cells, artifacts, repetitions,
manifest and controller hashes before execution. `broad-screen-progress.json`
and the per-cell series preserve every attempt. `broad-screen-score.json`
contains each cell's times, ratio and scientific comparison status.
`measurements.json`, `summary.json`, `series.json`, `identities.json`,
`references.json` and the independent audit receipts retain the underlying
evidence without licensed logs or machine paths.

The next measured work should address weighted repeated-edge operator cost,
narrow input/RHS passes and the fixed cost of probing/solving well-connected
small-width graphs. Forced solver comparisons are diagnostic only. The 111
native controls, additional weight-distribution/bridge checks, fresh holdouts
and two five-pair final blocks remain necessary. No universal win, release
qualification, installed package change or remote-state change is claimed.

`stata-version-erratum.json` corrects the weighted-input report’s textual
Stata version: the licensed executable was Stata/MP 19.0. The earlier prose
said 18; fixture compatibility directives and historical receipts are unchanged.
