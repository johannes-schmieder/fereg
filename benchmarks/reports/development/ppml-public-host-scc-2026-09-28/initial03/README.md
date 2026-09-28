# Bounded performance rotation

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 20/20 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| one-small-t1 | 0.535 / 0.805 / 1.075 | 2.122 / 2.508 / 2.894 |
| one-small-t2 | 0.693 / 0.820 / 0.947 | 2.495 / 2.550 / 2.606 |
| one-small-t4 | 0.799 / 0.811 / 0.823 | 1.377 / 1.957 / 2.536 |
| one-low-t4 | 0.806 / 0.853 / 0.899 | 2.412 / 2.550 / 2.688 |
| one-wide-t4 | 0.990 / 1.011 / 1.032 | 1.030 / 1.059 / 1.088 |
| control-profiled-small | 0.997 / 1.010 / 1.022 | 1.676 / 1.797 / 1.917 |
| control-three-fe-core-pair | 0.949 / 0.975 / 1.002 | 0.382 / 0.425 / 0.468 |
| control-excluded-slope | 0.860 / 0.906 / 0.952 | 0.400 / 0.403 / 0.405 |
| control-excluded-ols | 1.005 / 1.011 / 1.017 | 1.269 / 1.471 / 1.673 |
| one-larger-t8 | 1.023 / 1.059 / 1.095 | 1.040 / 1.145 / 1.250 |

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
