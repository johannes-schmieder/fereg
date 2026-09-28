# Bounded performance rotation

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 18/18 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| target-profiled-small | 0.792 / 0.848 / 0.903 | 1.845 / 1.856 / 1.866 |
| target-small-low | 1.024 / 1.027 / 1.030 | 1.468 / 1.489 / 1.511 |
| target-small-serial | 0.549 / 0.570 / 0.591 | 0.980 / 1.166 / 1.353 |
| target-wide-serial-adjoint | 0.798 / 0.820 / 0.842 | 0.653 / 0.661 / 0.669 |
| target-three-fe-core-pair | 0.892 / 0.973 / 1.054 | 0.472 / 0.482 / 0.492 |
| target-excluded-slope | 0.797 / 0.799 / 0.802 | 0.362 / 0.366 / 0.370 |
| target-excluded-ols | 0.952 / 0.979 / 1.005 | 1.635 / 1.699 / 1.762 |
| target-weak | 0.700 / 0.771 / 0.841 | NR |
| target-strong-d2 | 0.712 / 0.714 / 0.716 | 0.328 / 0.338 / 0.347 |

target-profiled-small: selected native baseline / candidate teams = 4 / 4.

target-small-low: selected native baseline / candidate teams = 4 / 4.

target-small-serial: selected native baseline / candidate teams = 1 / 1.

target-wide-serial-adjoint: selected native baseline / candidate teams = 4 / 4.

target-three-fe-core-pair: selected native baseline / candidate teams = 8 / 8.

target-excluded-slope: selected native baseline / candidate teams = 4 / 4.

target-excluded-ols: selected native baseline / candidate teams = 8 / 8.

target-weak: selected native baseline / candidate teams = 4 / 4.

target-strong-d2: selected native baseline / candidate teams = 4 / 4.

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

## Native thread scaling

| Lower → higher setting | Higher/lower min / median / max | Slower on every repetition |
|---|---:|---|

Independent-oracle failures: 0.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.
