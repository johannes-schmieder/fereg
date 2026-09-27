# Bounded performance rotation: partial 14-pair snapshot

This is a partial development snapshot of job 7760466 on scc-wo4, not the
campaign conclusion. Source and artifact identities are in identity.json and
the parent submission receipt. The candidate is the frozen Schwarz-memory
correction (e4317d8e computational inputs); the subsequent separation repair is
not included. All seven completed configurations have both registered seeds;
the remaining sixteen, including the independent oracle cells, are pending.

The completed native/baseline scientific checks all pass. Four OLS stock
comparisons fail coefficient/RSS and/or inference checks and remain NR. Among
completed comparable PPML cells, candidate/stock medians are 0.501 (K10/T8),
0.749 (K10/T28), and 0.373 (K50/T4). Candidate K10/T28 is still 3.8–6.8% slower
than candidate T8, so monotone scaling is not achieved. Two seeds do not establish
a statistical guarantee or satisfy the unfinished full-grid targets.

Memory is a tradeoff: K10/T28 candidate process RSS is about 2.82 GiB versus
1.02 GiB baseline and 1.26 GiB stock. The corrected admission enables a faster,
more memory-intensive solver; it does not reduce that solver's allocation.
Command clocks, process clocks and sampled RSS remain separate in summary.json.

Ratios use command clocks. Case medians require all registered seeds to pass. Scaling is diagnostic on native-certified same-input/sample fits. Two seeds do not establish a statistical guarantee.

Completed 14/42 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| ppml-three_fe_sparse-n983040-k10-t8-main-auto | 0.991 / 1.012 / 1.033 | 0.492 / 0.501 / 0.509 |
| ppml-three_fe_sparse-n983040-k10-t28-main-auto | 0.149 / 0.179 / 0.210 | 0.735 / 0.749 / 0.763 |
| ols-strong_d2-n983040-k10-t4-main-auto | 0.977 / 0.987 / 0.998 | NR |
| ols-strong_d2-n983040-k10-t28-main-auto | 0.266 / 0.274 / 0.282 | NR |
| ols-three_fe_sparse-n983040-k50-t4-width-auto | 0.993 / 0.996 / 0.999 | NR |
| ols-three_fe_sparse-n983040-k50-t28-width-auto | 0.199 / 0.204 / 0.209 | NR |
| ppml-three_fe_sparse-n983040-k50-t4-width-auto | 0.928 / 0.960 / 0.992 | 0.369 / 0.373 / 0.377 |
| ppml-three_fe_sparse-n983040-k50-t28-width-auto | NR | NR |
| fresh-ppml-three_fe_sparse-n245760-k6-t4 | NR | NR |
| fresh-ppml-three_fe_sparse-n245760-k6-t8 | NR | NR |
| fresh-ppml-three_fe_sparse-n245760-k6-t16 | NR | NR |
| fresh-ppml-three_fe_sparse-n245760-k6-t28 | NR | NR |
| fresh-ols-strong_d2-n491520-k10-t4 | NR | NR |
| fresh-ols-strong_d2-n491520-k10-t8 | NR | NR |
| fresh-ols-strong_d2-n491520-k10-t28 | NR | NR |
| fresh-ols-three_fe_sparse-n491520-k20-t4 | NR | NR |
| fresh-ols-three_fe_sparse-n491520-k20-t28 | NR | NR |
| fresh-ppml-two_fe_dense-n61440-k10-t4 | NR | NR |
| fresh-ppml-two_fe_dense-n61440-k10-t28 | NR | NR |
| oracle-ols-strong_d2 | NR | NR |
| oracle-ols-three_fe_sparse | NR | NR |
| oracle-ppml-three_fe_sparse | NR | NR |
| oracle-ppml-two_fe_dense | NR | NR |

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

## Native thread scaling

| Lower → higher setting | Higher/lower min / median / max | Slower on every seed |
|---|---:|---|
| ppml-three_fe_sparse-n983040-k10-t8-main-auto → ppml-three_fe_sparse-n983040-k10-t28-main-auto | 1.038 / 1.053 / 1.068 | True |
| ols-strong_d2-n983040-k10-t4-main-auto → ols-strong_d2-n983040-k10-t28-main-auto | 0.750 / 0.781 / 0.811 | False |
| ols-three_fe_sparse-n983040-k50-t4-width-auto → ols-three_fe_sparse-n983040-k50-t28-width-auto | 0.582 / 0.582 / 0.583 | False |
| ppml-three_fe_sparse-n983040-k50-t4-width-auto → ppml-three_fe_sparse-n983040-k50-t28-width-auto | NR | None |
| fresh-ppml-three_fe_sparse-n245760-k6-t4 → fresh-ppml-three_fe_sparse-n245760-k6-t8 | NR | None |
| fresh-ppml-three_fe_sparse-n245760-k6-t8 → fresh-ppml-three_fe_sparse-n245760-k6-t16 | NR | None |
| fresh-ppml-three_fe_sparse-n245760-k6-t16 → fresh-ppml-three_fe_sparse-n245760-k6-t28 | NR | None |
| fresh-ols-strong_d2-n491520-k10-t4 → fresh-ols-strong_d2-n491520-k10-t8 | NR | None |
| fresh-ols-strong_d2-n491520-k10-t8 → fresh-ols-strong_d2-n491520-k10-t28 | NR | None |
| fresh-ols-three_fe_sparse-n491520-k20-t4 → fresh-ols-three_fe_sparse-n491520-k20-t28 | NR | None |
| fresh-ppml-two_fe_dense-n61440-k10-t4 → fresh-ppml-two_fe_dense-n61440-k10-t28 | NR | None |

Independent-oracle failures: 0.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.
