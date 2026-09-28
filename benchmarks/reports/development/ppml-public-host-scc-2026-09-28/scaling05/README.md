# Bounded performance rotation

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 24/24 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| one_fe-t1-to-t2 | 0.871 / 1.080 / 1.097 | 1.910 / 2.070 / 2.934 |
| one_fe-t2-to-t4 | 0.851 / 0.867 / 0.879 | 1.500 / 1.834 / 2.346 |
| one_fe-t1-to-t4 | 0.660 / 0.933 / 1.007 | 1.894 / 2.447 / 2.701 |
| two_fe_dense-t1-to-t2 | 0.800 / 0.878 / 0.949 | 1.477 / 1.628 / 1.636 |
| two_fe_dense-t2-to-t4 | 0.791 / 0.832 / 0.949 | 1.568 / 1.635 / 1.907 |
| two_fe_dense-t1-to-t4 | 0.675 / 0.799 / 0.850 | 1.647 / 1.667 / 1.841 |
| one-wide-t4-t1-to-t4 | 0.669 / 0.680 / 0.820 | 1.071 / 1.125 / 1.147 |
| control-three-fe-core-pair-t8-to-t16 | 0.930 / 0.931 / 0.947 | 0.468 / 0.525 / 0.535 |

one_fe-t1-to-t2: baseline / candidate / stock threads = 1 / 2 / 2 (explicit thread comparison).

one_fe-t1-to-t2: selected native baseline / candidate teams = 1 / 2.

one_fe-t2-to-t4: baseline / candidate / stock threads = 2 / 4 / 4 (explicit thread comparison).

one_fe-t2-to-t4: selected native baseline / candidate teams = 2 / 4.

one_fe-t1-to-t4: baseline / candidate / stock threads = 1 / 4 / 4 (explicit thread comparison).

one_fe-t1-to-t4: selected native baseline / candidate teams = 1 / 4.

two_fe_dense-t1-to-t2: baseline / candidate / stock threads = 1 / 2 / 2 (explicit thread comparison).

two_fe_dense-t1-to-t2: selected native baseline / candidate teams = 1 / 2.

two_fe_dense-t2-to-t4: baseline / candidate / stock threads = 2 / 4 / 4 (explicit thread comparison).

two_fe_dense-t2-to-t4: selected native baseline / candidate teams = 2 / 4.

two_fe_dense-t1-to-t4: baseline / candidate / stock threads = 1 / 4 / 4 (explicit thread comparison).

two_fe_dense-t1-to-t4: selected native baseline / candidate teams = 1 / 4.

one-wide-t4-t1-to-t4: baseline / candidate / stock threads = 1 / 4 / 4 (explicit thread comparison).

one-wide-t4-t1-to-t4: selected native baseline / candidate teams = 1 / 4.

control-three-fe-core-pair-t8-to-t16: baseline / candidate / stock threads = 8 / 16 / 16 (explicit thread comparison).

control-three-fe-core-pair-t8-to-t16: selected native baseline / candidate teams = 8 / 8.

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

## Native thread scaling

| Lower → higher setting | Higher/lower min / median / max | Slower on every repetition |
|---|---:|---|

Independent-oracle failures: 0.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.
