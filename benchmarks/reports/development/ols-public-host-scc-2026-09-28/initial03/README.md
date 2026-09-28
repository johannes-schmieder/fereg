# Bounded performance rotation

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 20/20 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| ols-one-t1 | 0.693 / 0.931 / 1.169 | 1.486 / 1.861 / 2.235 |
| ols-one-t4 | 0.818 / 0.826 / 0.833 | 1.440 / 1.481 / 1.522 |
| ols-one-wide-t4 | 0.676 / 0.726 / 0.776 | 1.586 / 1.649 / 1.711 |
| ols-one-low-t4 | 0.800 / 0.815 / 0.830 | 2.233 / 2.484 / 2.735 |
| ols-two-t1 | 1.038 / 1.050 / 1.061 | 3.189 / 3.428 / 3.667 |
| ols-two-t8 | 0.862 / 0.883 / 0.905 | 1.361 / 1.397 / 1.434 |
| ols-three-t8 | 0.874 / 0.879 / 0.884 | NR |
| ols-slopes-t4 | 0.611 / 0.616 / 0.620 | 0.894 / 0.909 / 0.925 |
| control-ppml-one | 0.917 / 1.032 / 1.147 | 2.555 / 2.618 / 2.681 |
| control-ppml-three | 0.934 / 0.938 / 0.943 | 0.404 / 0.432 / 0.460 |

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
