# Bounded performance rotation

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 18/18 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| one-small-t1 | 0.699 / 0.838 / 0.977 | 2.407 / 2.451 / 2.495 |
| one-small-t2 | 0.962 / 1.002 / 1.043 | 2.000 / 2.026 / 2.052 |
| one-small-t4 | 0.900 / 0.957 / 1.014 | 1.907 / 2.638 / 3.368 |
| one-low-t4 | 0.854 / 0.873 / 0.892 | 3.329 / 3.465 / 3.600 |
| one-wide-t4 | 0.883 / 0.932 / 0.981 | 0.819 / 0.948 / 1.077 |
| control-profiled-small | 1.063 / 1.065 / 1.068 | 2.120 / 2.179 / 2.239 |
| control-three-fe-core-pair | 0.978 / 1.060 / 1.143 | 0.457 / 0.477 / 0.496 |
| control-excluded-slope | 0.873 / 0.967 / 1.061 | 0.412 / 0.416 / 0.419 |
| control-excluded-ols | 1.026 / 1.044 / 1.061 | 1.630 / 1.660 / 1.690 |

one-small-t1: selected native baseline / candidate teams = 1 / 1.

one-small-t2: selected native baseline / candidate teams = 2 / 2.

one-small-t4: selected native baseline / candidate teams = 4 / 4.

one-low-t4: selected native baseline / candidate teams = 4 / 4.

one-wide-t4: selected native baseline / candidate teams = 4 / 4.

control-profiled-small: selected native baseline / candidate teams = 4 / 4.

control-three-fe-core-pair: selected native baseline / candidate teams = 8 / 8.

control-excluded-slope: selected native baseline / candidate teams = 4 / 4.

control-excluded-ols: selected native baseline / candidate teams = 8 / 8.

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

## Native thread scaling

| Lower → higher setting | Higher/lower min / median / max | Slower on every repetition |
|---|---:|---|

Independent-oracle failures: 0.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.
