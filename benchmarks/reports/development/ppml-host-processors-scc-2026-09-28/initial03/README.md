# Bounded performance rotation

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 20/20 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| one-small-t1 | 0.948 / 1.041 / 1.134 | 2.683 / 2.688 / 2.692 |
| one-small-t2 | 0.674 / 0.847 / 1.019 | 1.683 / 1.773 / 1.864 |
| one-small-t4 | 0.964 / 0.987 / 1.009 | 2.845 / 2.922 / 3.000 |
| one-low-t4 | 0.981 / 0.984 / 0.987 | 3.466 / 3.657 / 3.848 |
| one-wide-t4 | 1.004 / 1.025 / 1.046 | 0.822 / 0.853 / 0.884 |
| control-profiled-small | 0.872 / 0.955 / 1.038 | 2.014 / 2.106 / 2.198 |
| control-three-fe-core-pair | 0.947 / 0.973 / 0.998 | 0.377 / 0.422 / 0.466 |
| control-excluded-slope | 0.813 / 1.083 / 1.353 | 0.334 / 0.363 / 0.393 |
| control-excluded-ols | 0.952 / 0.976 / 1.000 | 1.539 / 1.695 / 1.851 |
| one-larger-t8 | 0.973 / 0.980 / 0.987 | 1.170 / 1.171 / 1.171 |

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
