# Generated OLS terms: accept unchanged-sample factor-name reuse

The current-source screen finds useful factor/publication overhead alongside
remaining solver losses. Reuse the parser's canonical factor names when the
validated native receipt proves that every candidate row survived. This removes
one repeated `fvexpand`, improving the wide categorical interaction case by
15.5% and its predeclared fresh counterpart by 12.1%. Narrow factor/output gains
are smaller. Accept this bounded, allocation-free metadata change under
[ADR 0108](../../../../docs/adr/0108-unchanged-sample-factor-names.md).

## Source, scope and matching

Baseline native source is the retained `batch1` checkpoint at `56e4ff79`.
`factor_names1` reuses its exact plugin, SHA-256
`a191585c5a156809f8a572bbce37d948b2ede1e0ca8579a85f15e4cb32547d14`,
and changes only `stata/_fereg_estimate_ols.ado`, SHA-256
`407ae24785bc028aef1a3d5b808f0bb210d27017c801546b5bcde835883abc62`.
All 527 captured source inputs match the final working source. The new fixture
is separately executed and hash-bound. No native compilation or Rust test
rerun was needed for this ado-only change. Parent identities, the overlay,
diagnostic source copies and current-source binding are archived.

Stock fixest 0.14.2 remains uninstrumented. Its pinned source archive hash is
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`;
the loaded library hash is
`bece258f6c6fc3fc47accea45c7b004b0c495d46170d78fa2d6302e66de5fbf9`.

The additive `--surface` harness supports unweighted OLS factor main effects,
categorical interactions, IID/robust inference, and one-FE saved effects,
residuals, xb and xbd. Every finite coefficient and covariance entry is
exported. Stock effects use the same observation-weighted zero-mean
normalization. Fit/publication and requested prediction costs are timed
separately; their sum is the headline timer. Dataset import and evidence export
are outside both timers. All data generation, tests and audits are separate
from timed fits; no build, profiling, archive or competing-fit overlap occurred.

This does not add weighted stock comparisons, time-series terms, multi-FE
effect normalization or a new supported RHS feature. The original continuous
product probe fails native warmup with r(498), as required by the compatibility
matrix. Its successful stock fit and independent oracle remain recorded. An
amendment made before large timing omits this known unsupported case; the
original declaration and failure are preserved.

## Opportunity and mechanism

Initial production/stock medians in ms are 17/15 for the numeric control,
37/35 for one-FE factors, 338/411 for wide interactions, 75/42 for two-FE
factors, 50/41 with saved outputs and 104/80 for three-FE factors. All 18
scientific comparisons pass. These measurements supersede historical timings
only for these cases and this artifact.

Separate profiles locate about 30 ms in initial absorption for the two-FE
case and 63 ms for three FE. For the wide interaction case, ado boundary timers
record 66 ms before the plugin, 142 ms inside it, 52 ms through post-plugin
validation/names, and 72 ms around result posting. Repeated final `fvexpand`
alone costs 48 ms; it costs about 4 ms in the narrow cases. The first diagnostic
copy's displays were suppressed by quiet execution; its attempts are retained,
and the second copy writes timers to files. These diagnostics are excluded from
headline medians.

Existing native loading, score retention and inference improvements already
reach these fits. General factor recipes do not inherit the numeric-only RHS
binding paths, and retained PPML generated-predictor tiles are deliberately
separate from one-shot OLS. The rejected September 14 generated-residual tile
prototype was not reopened. The new change generalizes the existing continuous
name reuse through a sample invariant, without removing the final-sample guard.
Receipt candidate/retained counts are physical counts, including with FW;
equality proves identity because the retained sample is a subset. Any deletion
continues through the existing categorical expansion and omission processing.

## Alternating pairs and fresh confirmation

Three rotated baseline/candidate/stock repetitions per row. Screen seed
202609231 and fresh seed 202609232 were declared before candidate timings;
reserved validation seeds are untouched. `K` below counts numeric regressors;
factor widths are stated separately. Ratios are candidate/baseline per pair,
not ratios of independent medians.

| Block and case, N / K / threads | Factor levels | Baseline / candidate / stock medians, ms | Paired min / median / max |
|---|---:|---:|---:|
| Screen numeric robust, 61440 / 4 / 4 | — | 17 / 17 / 17 | 1.000 / 1.000 / 1.063 |
| Screen one FE IID, 61440 / 4 / 4 | 4 | 39 / 36 / 35 | .850 / .923 / 1.029 |
| Screen one FE interaction robust, 245760 / 8 / 8 | 8 × 4 | 341 / 288 / 427 | .830 / .845 / .889 |
| Screen two FE IID, 61440 / 4 / 4 | 4 | 73 / 70 / 37 | .945 / .972 / .986 |
| Screen one FE robust + all outputs, 61440 / 4 / 4 | 4 | 55 / 51 / 43 | .836 / .927 / .981 |
| Screen three FE IID, 61440 / 4 / 4 | 4 | 103 / 97 / 74 | .857 / .942 / .971 |
| Fresh one FE IID, unbalanced, 92160 / 6 / 1 | 7 | 114 / 109 / 64 | .940 / .956 / .965 |
| Fresh one FE interaction robust, unbalanced, 307200 / 6 / 4 | 6 × 5 | 480 / 421 / 580 | .870 / .879 / .881 |
| Fresh two FE IID, 92160 / 6 / 8 | 4 | 58 / 56 / 59 | .949 / .965 / .966 |
| Fresh one FE robust + all outputs, 92160 / 6 / 8 | 7 | 75 / 69 / 61 | .907 / .932 / .961 |
| Fresh three FE IID, 92160 / 6 / 8 | 4 | 145 / 141 / 2172, unranked | .945 / .972 / 1.028 |
| Fresh numeric robust, 92160 / 6 / 8 | — | 20 / 19 / 21 | .950 / 1.000 / 1.000 |

Six of eleven stock-comparable rows still lose, four win and one ties at the
timer resolution. The fresh three-FE row fails stock beta/RSS checks in every
repeat: maximum beta difference 5.43e−6 and RSS difference .10965. Its covariance
passes this screen's tolerance. Do not count its large timing difference as a
win or reinterpret earlier three-FE discrepancies. Native pairs remain exact.
The serial factor case and two-FE screen retain substantial losses; this simple
publication change does not repair their dominant costs. Three repetitions
support a bounded local decision, not universal timing claims.

## Science, failures and memory

All 144 headline attempts finish with passing application sentinels, no timeout,
detected overlap or monitor error. All 36 native pairs have byte-identical
scientific exports, including native saved-output exports; repeat comparisons
pass. Direct candidate/stock checks pass 33/36, with the three failures above
preserved. The 14-call small pilot has 13 successes and the intentional product
rejection. Six supported pilot cases pass independently constructed dummy-OLS
coefficient, full covariance, sample and RSS checks; its requested normalized
output vectors also pass. The twelve diagnostic attempts complete separately.

Three licensed candidate fixtures pass: the new ten-scenario dummy-OLS oracle,
the established categorical final-sample rebasing fixture, and the eclass
transaction fixture. The new oracle covers complete/restricted candidates,
missing outcome/regressor/weight/cluster rows, singleton deletion, categorical
tuple level rebasing, and unit/AW/FW/PW robust or clustered inference. FW uses
Stata's independent frequency-weight regression, not ordinary fixest weights.

Keep the development failures visible:

- The first new fixture exposes r(459), coefficient-name count disagreement,
  when missing outcomes remove the entire default factor level under ordinary
  `absorb(g)`. The unchanged baseline reproduces it. The final passing fixture
  exercises complete-level rebasing through the already supported categorical
  tuple path; this is not a repair of the ordinary path. Both failed original
  fixture copies and log hashes are preserved for the next investigation.
- A fixture draft incorrectly treated zero FW as sample exclusion. The
  documented r(401) rejection occurred; the corrected fixture uses missing FW.
  The failed draft and return code remain in the evidence.

The production change adds no allocation and alters no native memory plan.
Tracked native peak deltas are exactly zero outside threaded three-FE cells,
whose unchanged native code fluctuates from −2.89 to +2.97 MB. Per-call sampled
process RSS, sampling counts, fit/prediction timings and native estimates remain
in the receipts. These observations are not a constrained-memory claim. Static
allocation-source drift remains open and was not regenerated or rerun here.

## Disposition

Accept the broad sample-invariant name reuse and close this experiment. Preserve
the 77 primary, 16 holdout and 111 native-control definitions. No SCC retry,
qualification campaign, stopped reference campaign, push or release occurred.

The next bounded task should localize and repair the newly reproduced ordinary
factor-level rebasing failure before further factor optimization. Remaining
two-/three-FE operator/preconditioner costs warrant a separate investigation;
additional RHS-window tuning remains closed. Historical low-memory, generated
TS, multi-partition slope and broad output performance gaps still need their own
matched current-source measurements; this report does not declare them solved.

`evidence.zip` captures plans and amendments, controllers, identities, numerical
exports, all attempts, audits, source overlays, diagnostic timers and fixture
receipts. Original/portable hashes are verified separately. Data, binaries and
raw licensed logs are excluded; full output binaries remain locally hash-bound.
