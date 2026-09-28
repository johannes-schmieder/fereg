# Bounded performance rotation

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 30/30 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| one-small-t1 | 0.546 / 0.847 / 0.860 | 1.728 / 2.654 / 2.747 |
| one-small-t2 | 0.852 / 0.986 / 1.015 | 1.699 / 2.189 / 2.491 |
| one-small-t4 | 0.793 / 0.916 / 0.932 | 2.395 / 3.000 / 3.080 |
| one-low-t4 | 0.941 / 0.967 / 1.153 | 2.750 / 3.473 / 3.709 |
| one-wide-t4 | 0.821 / 0.887 / 0.990 | 0.855 / 0.930 / 1.052 |
| control-profiled-small | 0.956 / 1.024 / 1.097 | 1.950 / 2.088 / 2.167 |
| control-three-fe-core-pair | 0.913 / 0.923 / 0.991 | 0.412 / 0.533 / 0.533 |
| control-excluded-slope | 0.970 / 1.039 / 1.123 | 0.360 / 0.396 / 0.425 |
| control-excluded-ols | 0.974 / 0.979 / 0.984 | 1.376 / 1.415 / 1.500 |
| one-larger-t8 | 0.993 / 1.087 / 1.110 | 1.194 / 1.228 / 1.239 |

one-small-t1: selected native baseline / candidate teams = 1 / 1.

one-small-t2: selected native baseline / candidate teams = 2 / 2.

one-small-t4: selected native baseline / candidate teams = 4 / 4.

one-low-t4: selected native baseline / candidate teams = 4 / 4.

one-wide-t4: selected native baseline / candidate teams = 4 / 4.

control-profiled-small: selected native baseline / candidate teams = 4 / 4.

control-three-fe-core-pair: selected native baseline / candidate teams = 8 / 8.

control-excluded-slope: selected native baseline / candidate teams = 4 / 4.

control-excluded-ols: selected native baseline / candidate teams = 8 / 8.

one-larger-t8: selected native baseline / candidate teams = 8 / 8.

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

## Native thread scaling

| Lower → higher setting | Higher/lower min / median / max | Slower on every repetition |
|---|---:|---|

Independent-oracle failures: 0.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.
