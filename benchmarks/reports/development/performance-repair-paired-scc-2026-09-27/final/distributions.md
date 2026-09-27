# Bounded performance rotation

Ratios use command clocks. Case medians require all registered seeds to pass. Scaling is diagnostic on native-certified same-input/sample fits. Two seeds do not establish a statistical guarantee.

Completed 42/42 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| ppml-three_fe_sparse-n983040-k10-t8-main-auto | 0.991 / 1.012 / 1.033 | 0.492 / 0.501 / 0.509 |
| ppml-three_fe_sparse-n983040-k10-t28-main-auto | 0.149 / 0.179 / 0.210 | 0.735 / 0.749 / 0.763 |
| ols-strong_d2-n983040-k10-t4-main-auto | 0.977 / 0.987 / 0.998 | NR |
| ols-strong_d2-n983040-k10-t28-main-auto | 0.266 / 0.274 / 0.282 | NR |
| ols-three_fe_sparse-n983040-k50-t4-width-auto | 0.993 / 0.996 / 0.999 | NR |
| ols-three_fe_sparse-n983040-k50-t28-width-auto | 0.199 / 0.204 / 0.209 | NR |
| ppml-three_fe_sparse-n983040-k50-t4-width-auto | 0.928 / 0.960 / 0.992 | 0.369 / 0.373 / 0.377 |
| ppml-three_fe_sparse-n983040-k50-t28-width-auto | 0.078 / 0.085 / 0.093 | 0.457 / 0.464 / 0.471 |
| fresh-ppml-three_fe_sparse-n245760-k6-t4 | 0.991 / 1.006 / 1.020 | 0.525 / 0.564 / 0.603 |
| fresh-ppml-three_fe_sparse-n245760-k6-t8 | 0.854 / 0.921 / 0.989 | 0.560 / 0.600 / 0.640 |
| fresh-ppml-three_fe_sparse-n245760-k6-t16 | 1.070 / 1.076 / 1.081 | 0.611 / 0.637 / 0.664 |
| fresh-ppml-three_fe_sparse-n245760-k6-t28 | 1.004 / 1.049 / 1.095 | 0.674 / 0.714 / 0.753 |
| fresh-ols-strong_d2-n491520-k10-t4 | 0.943 / 1.011 / 1.079 | NR |
| fresh-ols-strong_d2-n491520-k10-t8 | 0.886 / 0.923 / 0.960 | NR |
| fresh-ols-strong_d2-n491520-k10-t28 | 1.013 / 1.019 / 1.025 | NR |
| fresh-ols-three_fe_sparse-n491520-k20-t4 | 1.013 / 1.026 / 1.039 | NR |
| fresh-ols-three_fe_sparse-n491520-k20-t28 | 0.993 / 0.999 / 1.005 | NR |
| fresh-ppml-two_fe_dense-n61440-k10-t4 | 1.060 / 1.068 / 1.076 | 1.044 / 1.113 / 1.182 |
| fresh-ppml-two_fe_dense-n61440-k10-t28 | 1.013 / 1.022 / 1.031 | 0.664 / 0.754 / 0.844 |
| oracle-ols-strong_d2 | 1.000 / 1.000 / 1.000 | 0.554 / 0.554 / 0.554 |
| oracle-ols-three_fe_sparse | 0.950 / 0.950 / 0.950 | NR |
| oracle-ppml-three_fe_sparse | 0.889 / 0.889 / 0.889 | 0.208 / 0.208 / 0.208 |
| oracle-ppml-two_fe_dense | 1.271 / 1.271 / 1.271 | 0.466 / 0.466 / 0.466 |

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

## Native thread scaling

| Lower → higher setting | Higher/lower min / median / max | Slower on every seed |
|---|---:|---|
| ppml-three_fe_sparse-n983040-k10-t8-main-auto → ppml-three_fe_sparse-n983040-k10-t28-main-auto | 1.038 / 1.053 / 1.068 | True |
| ols-strong_d2-n983040-k10-t4-main-auto → ols-strong_d2-n983040-k10-t28-main-auto | 0.750 / 0.781 / 0.811 | False |
| ols-three_fe_sparse-n983040-k50-t4-width-auto → ols-three_fe_sparse-n983040-k50-t28-width-auto | 0.582 / 0.582 / 0.583 | False |
| ppml-three_fe_sparse-n983040-k50-t4-width-auto → ppml-three_fe_sparse-n983040-k50-t28-width-auto | 0.499 / 0.499 / 0.500 | False |
| fresh-ppml-three_fe_sparse-n245760-k6-t4 → fresh-ppml-three_fe_sparse-n245760-k6-t8 | 0.865 / 0.879 / 0.892 | False |
| fresh-ppml-three_fe_sparse-n245760-k6-t8 → fresh-ppml-three_fe_sparse-n245760-k6-t16 | 1.017 / 1.071 / 1.125 | True |
| fresh-ppml-three_fe_sparse-n245760-k6-t16 → fresh-ppml-three_fe_sparse-n245760-k6-t28 | 1.100 / 1.139 / 1.179 | True |
| fresh-ols-strong_d2-n491520-k10-t4 → fresh-ols-strong_d2-n491520-k10-t8 | 0.201 / 0.235 / 0.269 | False |
| fresh-ols-strong_d2-n491520-k10-t8 → fresh-ols-strong_d2-n491520-k10-t28 | 0.852 / 0.883 / 0.913 | False |
| fresh-ols-three_fe_sparse-n491520-k20-t4 → fresh-ols-three_fe_sparse-n491520-k20-t28 | 0.754 / 0.761 / 0.767 | False |
| fresh-ppml-two_fe_dense-n61440-k10-t4 → fresh-ppml-two_fe_dense-n61440-k10-t28 | 0.739 / 0.743 / 0.748 | False |

Independent-oracle failures: 0.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.
