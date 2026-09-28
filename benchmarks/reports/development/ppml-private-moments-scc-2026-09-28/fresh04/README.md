# Bounded performance rotation

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 30/30 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| one-small-t1 | 0.890 / 0.923 / 0.989 | 2.268 / 2.769 / 2.989 |
| one-small-t2 | 0.876 / 0.893 / 0.908 | 1.435 / 1.627 / 1.701 |
| one-small-t4 | 0.861 / 0.874 / 0.942 | 1.835 / 1.855 / 2.237 |
| one-low-t4 | 0.960 / 0.965 / 0.993 | 2.314 / 3.000 / 3.481 |
| one-wide-t4 | 0.779 / 0.842 / 0.843 | 0.909 / 0.913 / 0.964 |
| control-profiled-small | 0.937 / 1.010 / 1.092 | 1.362 / 1.533 / 1.640 |
| control-three-fe-core-pair | 0.860 / 0.881 / 0.960 | 0.299 / 0.306 / 0.371 |
| control-excluded-slope | 0.915 / 0.948 / 1.051 | 0.335 / 0.350 / 0.408 |
| control-excluded-ols | 0.957 / 1.011 / 1.071 | 1.575 / 1.625 / 1.640 |
| one-larger-t8 | 0.729 / 0.774 / 0.890 | 0.914 / 0.927 / 0.932 |

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
