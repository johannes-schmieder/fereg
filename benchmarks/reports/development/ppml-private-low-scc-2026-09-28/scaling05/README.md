# Bounded performance rotation

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 18/18 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| one-low-t2-1-to-2 | 0.730 / 0.776 / 0.984 | 1.528 / 1.617 / 2.505 |
| one-low-t4-2-to-4 | 0.854 / 0.878 / 0.901 | 1.553 / 1.756 / 1.803 |
| one-low-t4-4-to-8 | 0.882 / 0.901 / 0.905 | 1.073 / 1.496 / 1.842 |
| one-single-row-t8-4-to-8 | 0.872 / 0.924 / 0.941 | 1.827 / 2.456 / 2.676 |
| one-low-wide-t4-1-to-4 | 0.494 / 0.608 / 0.650 | 0.897 / 0.961 / 0.975 |
| one-low-larger-t8-4-to-8 | 0.810 / 0.851 / 0.857 | 0.915 / 0.921 / 0.923 |

one-low-t2-1-to-2: baseline / candidate / stock threads = 1 / 2 / 2 (explicit thread comparison).

one-low-t2-1-to-2: selected native baseline / candidate teams = 1 / 2.

one-low-t4-2-to-4: baseline / candidate / stock threads = 2 / 4 / 4 (explicit thread comparison).

one-low-t4-2-to-4: selected native baseline / candidate teams = 2 / 4.

one-low-t4-4-to-8: baseline / candidate / stock threads = 4 / 8 / 8 (explicit thread comparison).

one-low-t4-4-to-8: selected native baseline / candidate teams = 4 / 8.

one-single-row-t8-4-to-8: baseline / candidate / stock threads = 4 / 8 / 8 (explicit thread comparison).

one-single-row-t8-4-to-8: selected native baseline / candidate teams = 4 / 8.

one-low-wide-t4-1-to-4: baseline / candidate / stock threads = 1 / 4 / 4 (explicit thread comparison).

one-low-wide-t4-1-to-4: selected native baseline / candidate teams = 1 / 4.

one-low-larger-t8-4-to-8: baseline / candidate / stock threads = 4 / 8 / 8 (explicit thread comparison).

one-low-larger-t8-4-to-8: selected native baseline / candidate teams = 4 / 8.

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

## Native thread scaling

| Lower → higher setting | Higher/lower min / median / max | Slower on every repetition |
|---|---:|---|

Independent-oracle failures: 0.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.
