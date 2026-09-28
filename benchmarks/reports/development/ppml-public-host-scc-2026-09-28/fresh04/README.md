# Bounded performance rotation

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 30/30 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| one-small-t1 | 0.702 / 0.985 / 1.003 | 2.012 / 2.583 / 4.037 |
| one-small-t2 | 0.908 / 0.941 / 0.971 | 1.823 / 2.032 / 2.538 |
| one-small-t4 | 0.755 / 0.762 / 0.797 | 1.579 / 2.049 / 2.562 |
| one-low-t4 | 0.819 / 0.824 / 0.846 | 1.878 / 2.450 / 2.740 |
| one-wide-t4 | 0.938 / 0.966 / 1.012 | 0.973 / 0.992 / 1.133 |
| control-profiled-small | 0.868 / 0.889 / 0.914 | 1.636 / 1.766 / 1.774 |
| control-three-fe-core-pair | 1.009 / 1.024 / 1.042 | 0.490 / 0.547 / 0.615 |
| control-excluded-slope | 0.925 / 1.002 / 1.056 | 0.384 / 0.407 / 0.410 |
| control-excluded-ols | 0.963 / 0.995 / 1.038 | 1.252 / 1.533 / 1.575 |
| one-larger-t8 | 0.926 / 0.984 / 1.074 | 1.141 / 1.172 / 1.207 |

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
