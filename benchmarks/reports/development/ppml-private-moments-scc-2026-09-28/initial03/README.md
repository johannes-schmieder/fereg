# Bounded performance rotation

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 20/20 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| one-small-t1 | 0.882 / 0.891 / 0.901 | 2.168 / 2.343 / 2.518 |
| one-small-t2 | 0.886 / 1.029 / 1.171 | 1.656 / 1.959 / 2.262 |
| one-small-t4 | 0.813 / 0.861 / 0.909 | 1.973 / 2.021 / 2.069 |
| one-low-t4 | 1.133 / 1.137 / 1.141 | 2.702 / 2.911 / 3.121 |
| one-wide-t4 | 0.845 / 0.894 / 0.943 | 0.889 / 0.916 / 0.943 |
| control-profiled-small | 0.972 / 1.018 / 1.063 | 1.721 / 1.888 / 2.056 |
| control-three-fe-core-pair | 0.878 / 0.935 / 0.992 | 0.392 / 0.418 / 0.445 |
| control-excluded-slope | 0.804 / 0.868 / 0.932 | 0.358 / 0.360 / 0.362 |
| control-excluded-ols | 0.954 / 0.993 / 1.033 | 1.528 / 1.580 / 1.632 |
| one-larger-t8 | 0.735 / 0.738 / 0.741 | 0.885 / 0.898 / 0.911 |

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
