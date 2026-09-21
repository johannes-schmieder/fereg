# Broad two-way OLS: final primary timing and science

Local development on 2026-09-20, M2 Ultra, Stata/MP 19.0, sixteen requested
native/R workers. Stata-side work has eight licensed processors. Production
source remains `cdfbcf58`, plugin
`79141637b5252f27d6f86a721fb04cf5648df6b8d0d82ee26ea7e39b9b94de14`.
All 516 captured source files match. The embedded build date is September 19;
these measurements were collected September 20. No estimator changes were
made during final validation.

Both predeclared five-pair blocks beat default pinned fixest in **all 77 cell
medians**. All 770 individual primary pairs also win. Worst median paired
ratios are **0.960159** and **0.964567**, both shuffled unit-weight K=5.
All 1,540 application calls complete successfully. This is bounded local
development evidence, not release, cross-platform or universal superiority.

## Runtime coverage

The table reports geometric means and maxima of cell median paired
fereg/fixest ratios. The command timer includes native Stata input loading,
estimation, inference and all requested outputs/predictions. Dataset generation,
application import, result export and oracle audits are outside timing.
Arms alternate, each in a fresh process with a small warm-up. No build, test,
large audit or other benchmark ran concurrently with these timed calls.

| Family | Cells | Block A geometric mean / worst | Block B geometric mean / worst |
|---|---:|---:|---:|
| Dense widths, unit/AW | 12 | 0.543 / 0.704 | 0.544 / 0.718 |
| Dense AW inference and outputs | 14 | 0.460 / 0.753 | 0.459 / 0.762 |
| Dense PW inference and outputs | 8 | 0.469 / 0.760 | 0.468 / 0.766 |
| Shuffled unit | 3 | 0.802 / 0.960 | 0.816 / 0.965 |
| Shuffled AW | 4 | 0.724 / 0.922 | 0.709 / 0.906 |
| Other two-way graphs | 36 | 0.356 / 0.904 | 0.354 / 0.928 |

The nine graph families are degree two, three and six, sparse mobility,
unbalanced, disconnected blocks, weak degree three, bottleneck ring and cycle.
They use N=245,760, K=6/50 and unit/AW weights. Dense cases use N=983,040,
including widths 1/3/5/6/13/50 and robust, one-/two-cluster, nested-cluster,
saved-effect and full-output requests. The exact inventory is retained in
both block plans; every cell and repetition appears in the scores.

Three primary cells miss the aspirational 0.90 ratio in both blocks: shuffled
unit K=5, shuffled AW K=6 IID and degree-six AW K=6. Their block-B median
command times are respectively 245 versus 254 ms, 271 versus 297 ms, and
65 versus 71 ms. Their native input/compute medians are approximately
32/202, 41/220 and 11/46 ms, respectively. Medians of separate phases need
not sum exactly to the command median. These remain useful optimization
targets; no cell was removed to obtain the win count.

## Independent science and reference discrepancies

All **360 native graph repetitions** pass the independent grounded sparse
direct oracle for coefficients, IID covariance, RSS and effective N. Maximum
native coefficient error is 1.37e-12. All **80 native saved-output audits**
pass weighted normal equations, FE constancy, component normalization,
reconstruction and RSS checks. Every primary cell preserves all six native
scientific CSV bundles, sample hashes and requested output vectors bitwise
across its ten calls.

Each block retains 53 passing comparison cells, eight reference-output
discrepancy cells and sixteen reference-science discrepancy cells. The same
160 default-reference graph repetitions fail the independent oracle, with
maximum coefficient error 1.80e-5. These are not relabeled parity passes.
The independent oracle binds canonical input and column hashes, solves
grounded weighted dummy systems, refines residuals, and verifies fresh normal
equations. Its earlier separate dense-SVD self-check remains in the
[graph science report](../two-fe-broad-graph-science-local-2026-09-20/README.md).

## Evidence

`primary-audit.json` joins both timing blocks, source/artifact checks,
per-family results and native repeatability. `primary-final-*-score.json`
retains each ratio and comparison disposition. `measurements.json`,
`series.json`, `references.json`, `identities.json` and the actual `controller/`
bind calls, samples, plugin and pinned reference. Independent graph/output
receipts and the executed review/archive helpers are included. Raw licensed
logs and machine-specific paths are excluded.

The final primary score complements the separate
[fresh holdouts](../two-fe-fresh-holdouts-local-2026-09-20/README.md),
[111 native controls](../two-fe-broad-native-controls-local-2026-09-20/README.md)
and [small-command precision adjudication](../two-fe-single-command-clock-local-2026-09-20/README.md).
Historical failed attempts and their original dispositions remain immutable.

## Bounded milestone acceptance

`development-acceptance.json` joins the primary results, both sixteen-cell
fresh-seed holdout blocks, all 111 native controls, the separately adjudicated
small-command timing check, final reference accuracy and memory follow-up.
The local development milestone is complete. Every primary and holdout cell
median is below one in both blocks. Across those blocks, 928 of 930 individual
pairs win; the two holdout losses remain visible. No control has an unexplained
confirmed regression above five percent after the documented precision check.
Five final-artifact Stata fixtures and their terminal receipts are linked.

The [final accuracy report](../two-fe-final-reference-accuracy-local-2026-09-20/README.md)
checks all 28 distinct discrepant cells separately at tighter reference
precision: eighteen references agree, ten do not converge, and all native
science/output bundles match their headline results exactly. The
[memory follow-up](../two-fe-memory-followup-local-2026-09-20/README.md)
records all 42 calls within 5.9 percent of the existing whole-action forecast
while preserving strict proof gaps and every earlier peak increase.

This acceptance covers ordinary categorical two-FE OLS on the declared local
matrix. FW uses native-baseline and independent literal-expansion evidence;
ordinary fixest weights are not a frequency-inference comparator. Other
graph-by-inference combinations, slopes, three or more effects, PPML,
cross-platform behavior and release qualification require their own evidence.
The ten-percent margin remains aspirational in the explicitly listed cells.
