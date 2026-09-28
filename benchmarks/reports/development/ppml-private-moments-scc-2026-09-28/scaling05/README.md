# Bounded performance rotation

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 24/24 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| one_fe-t1-to-t2 | 0.626 / 0.850 / 0.964 | 1.713 / 2.168 / 2.417 |
| one_fe-t2-to-t4 | 0.810 / 0.814 / 0.837 | 1.906 / 2.065 / 2.568 |
| one_fe-t1-to-t4 | 0.616 / 0.788 / 0.815 | 1.585 / 2.137 / 2.142 |
| two_fe_dense-t1-to-t2 | 0.892 / 0.955 / 1.043 | 1.111 / 1.211 / 1.445 |
| two_fe_dense-t2-to-t4 | 0.778 / 0.833 / 0.904 | 1.623 / 1.700 / 1.712 |
| two_fe_dense-t1-to-t4 | 0.697 / 0.793 / 0.831 | 1.655 / 1.805 / 1.883 |
| one-wide-t4-t1-to-t4 | 0.635 / 0.655 / 0.691 | 0.611 / 0.885 / 0.942 |
| control-three-fe-core-pair-t8-to-t16 | 0.925 / 0.946 / 0.960 | 0.368 / 0.416 / 0.425 |

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
