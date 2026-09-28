# Same-artifact PPML thread confirmation

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 24/24 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| small-dense-t1-t4 | 0.632 / 0.724 / 0.989 | 1.192 / 1.314 / 1.586 |
| narrow-sparse-t8-t28 | 1.086 / 1.113 / 1.252 | 0.100 / 0.108 / 0.113 |
| fresh-narrow-t8-t28 | 1.106 / 1.126 / 1.220 | 0.111 / 0.123 / 0.127 |
| fresh-wide-t8-t28 | 0.754 / 0.769 / 0.782 | 0.121 / 0.123 / 0.127 |

small-dense-t1-t4: baseline / candidate / stock threads = 1 / 4 / 4 (explicit thread comparison).

narrow-sparse-t8-t28: baseline / candidate / stock threads = 8 / 28 / 28 (explicit thread comparison).

fresh-narrow-t8-t28: baseline / candidate / stock threads = 8 / 28 / 28 (explicit thread comparison).

fresh-wide-t8-t28: baseline / candidate / stock threads = 8 / 28 / 28 (explicit thread comparison).

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

## Native thread scaling

| Lower → higher setting | Higher/lower min / median / max | Slower on every repetition |
|---|---:|---|

Independent-oracle failures: 0.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.

Both native arms use the identical validated portable-coupling plugin `e9d9f706…`,
computational source c6c1df25, snapshot e07c7ec3…. They differ only in registered
thread requests and corresponding affinity. This is a thread comparison, not a
new binary speed claim. Each command includes robust inference and no optional
saved observation outputs. Stock is pinned, unmodified fixest 0.14.2 `fepois`,
`glm.tol=1e-10`, `fixef.tol=1e-11`, with matching covariance correction. All 72
applications and 24 native plus 24 strict-stock science comparisons pass,
including sample, full covariance, objective and actual native team receipts.
This timing block contains no explicit-design oracle fits; independent science
evidence remains in the preceding portable-kernel validation.

| Case | Lower / higher / strict-stock median seconds | Higher-thread slower pairs | Lower / higher median process peak MiB |
|---|---:|---:|---:|
| Small two-FE N30,720/K6, T1→T4 | 0.650 / 0.452 / 0.334 | 0/9 | 116.2 / 118.5 |
| Three-FE N245,760/K6, T8→T28 | 6.429 / 7.469 / 69.595 | 9/9 | 499.5 / 626.8 |
| Fresh three-FE K6, T8→T28 | 6.729 / 7.943 / 64.836 | 3/3 | 492.7 / 629.7 |
| Fresh three-FE K32, T8→T28 | 19.459 / 14.955 / 119.916 | 0/3 | 1118.1 / 1496.5 |

The small T4 case remains **31.4% slower than strict stock** at the paired median.
Serial execution makes it worse; there is no basis for a T1 cap. Narrow
three-FE PPML is consistently slower at T28: **11.3%** on the prior input and
**12.6%** on fresh seed 202609283. Wide K32 instead improves **23.1%** at T28.
This supports testing a width-sensitive team choice; it does not validate a
particular threshold or generalize beyond the observed workloads.

The small and original narrow inputs reuse their prior seeds and must match
captured byte hashes before fits. Fresh narrow/wide seed 202609283 was declared
before generation. None may now become a fresh tuning/validation seed. The
source candidate must still be compared at equal public thread requests:
requesting eight also changes process affinity here, whereas a selected
eight-worker team under `threads(28)` retains a 28-core affinity in the next
experiment.

Strict fixest is much slower on these sparse inputs than the earlier default
reference. These 0.108–0.123 ratios **cannot replace comparable default-stock
evidence**. Full process clocks, including warmup/extraction, are retained
separately in summary.json and are not command timings.

Job **7762547** passes accounting (`failed=0`, `exit_status=0`), both terminal
sentinels and all 24 registered pairs: 1,786 seconds, 28 isolated slots on
scc-wo4, maxvmem 3.405G. The wall-only extension to 50 minutes changed no case,
repetition or per-command limit. No builds, profiling, archiving or competing
fereg fits ran during this headline block. Candidate validation started only
after its successful termination.

The bounded next step is source-bound validation and fresh performance checks
of the eight-worker ceiling. Small categorical PPML, low-memory OLS, broader
thread scaling and unresolved comparator science remain open.
