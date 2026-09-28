# Bounded performance rotation

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 30/30 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| ols-one-t1 | 0.703 / 0.893 / 1.169 | 1.607 / 1.971 / 2.714 |
| ols-one-t4 | 0.837 / 0.949 / 0.974 | 1.309 / 1.762 / 2.452 |
| ols-one-wide-t4 | 0.716 / 0.744 / 0.829 | 1.770 / 1.872 / 1.959 |
| ols-one-low-t4 | 0.765 / 0.800 / 0.807 | 1.846 / 2.844 / 3.000 |
| ols-two-t1 | 0.969 / 0.993 / 1.043 | 2.507 / 2.682 / 2.807 |
| ols-two-t8 | 0.865 / 0.911 / 0.923 | 1.325 / 1.362 / 1.523 |
| ols-three-t8 | 0.903 / 0.915 / 0.952 | NR |
| ols-slopes-t4 | 0.986 / 1.070 / 1.496 | 1.256 / 1.423 / 1.741 |
| control-ppml-one | 0.925 / 1.009 / 1.024 | 1.974 / 2.726 / 2.795 |
| control-ppml-three | 0.903 / 0.947 / 0.971 | 0.447 / 0.457 / 0.469 |

ols-one-t1: selected native baseline / candidate teams = 1 / 1.

ols-one-t4: selected native baseline / candidate teams = 4 / 4.

ols-one-wide-t4: selected native baseline / candidate teams = 4 / 4.

ols-one-low-t4: selected native baseline / candidate teams = 4 / 4.

ols-two-t1: selected native baseline / candidate teams = 1 / 1.

ols-two-t8: selected native baseline / candidate teams = 8 / 8.

ols-three-t8: selected native baseline / candidate teams = 8 / 8.

ols-slopes-t4: selected native baseline / candidate teams = 4 / 4.

control-ppml-one: selected native baseline / candidate teams = 4 / 4.

control-ppml-three: selected native baseline / candidate teams = 8 / 8.

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

## Native thread scaling

| Lower → higher setting | Higher/lower min / median / max | Slower on every repetition |
|---|---:|---|

Independent-oracle failures: 0.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.
