# Portable canonical products: fresh confirmation

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 37/37 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| fresh-fast-t4 | 0.798 / 0.943 / 0.998 | 1.731 / 2.015 / 2.104 |
| fresh-serial-t1 | 0.754 / 0.912 / 1.041 | 1.559 / 2.130 / 2.476 |
| fresh-low | 0.753 / 0.791 / 0.981 | 1.383 / 1.607 / 1.677 |
| fresh-wide | 0.747 / 0.757 / 0.808 | 1.117 / 1.136 / 1.244 |
| fresh-larger | 0.959 / 1.010 / 1.108 | 1.030 / 1.038 / 1.039 |
| fresh-sparse | 0.825 / 0.839 / 1.023 | 0.379 / 0.410 / 0.486 |
| fresh-ols | 1.000 / 1.021 / 1.044 | 1.532 / 1.569 / 1.664 |
| fresh-slope | 0.865 / 0.970 / 1.002 | 0.343 / 0.383 / 0.438 |
| slope-control-confirmation | 0.804 / 1.018 / 1.255 | 0.310 / 0.397 / 0.418 |

fresh-fast-t4: selected native baseline / candidate teams = 4 / 4.

fresh-serial-t1: selected native baseline / candidate teams = 1 / 1.

fresh-low: selected native baseline / candidate teams = 4 / 4.

fresh-wide: selected native baseline / candidate teams = 8 / 8.

fresh-larger: selected native baseline / candidate teams = 8 / 8.

fresh-sparse: selected native baseline / candidate teams = 4 / 4.

fresh-ols: selected native baseline / candidate teams = 8 / 8.

fresh-slope: selected native baseline / candidate teams = 4 / 4.

slope-control-confirmation: selected native baseline / candidate teams = 4 / 4.

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

## Native thread scaling

| Lower → higher setting | Higher/lower min / median / max | Slower on every repetition |
|---|---:|---|

Independent-oracle failures: 0.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.

## Decision and scope

Accept the candidate for bounded development under ADR 0234. All **111 public
applications, 37 native science pairs and 37 stock science pairs** pass. Stock
uses pinned, unmodified fixest 0.14.2: 25 default pairs and 12 explicitly strict
slope pairs (`glm.tol=1e-10`, `fixef.tol=1e-11`). Initial default slope discrepancies
remain NR in initial02. Robust PPML and IID OLS inference are included; these
rotations request no optional saved observation outputs. Separate licensed
independent-oracle fixtures cover the scientific boundary. These rotations do
not establish a new independent large-design oracle.

All fresh cases use predeclared seed 202609287. The nine-pair slope confirmation
reuses the exact originally losing seed 202609286 input. Candidate snapshot
`4aa44429…` / plugin `d1156e6d…` is unchanged throughout; baseline is `8e2d5af2…`.
No benchmark-dependent guards or thresholds were fitted.

Fresh low, wide and sparse PPML improve **20.9%, 24.3% and 16.1%** at paired
medians. Small fast and serial gains narrow to **5.7% and 8.8%**; the larger
N122880/K10 case is neutral at 1.010. Marginal baseline/candidate/stock medians
(ms) are 448/425/211 fast, 561/475/238 serial, 397/332/219 low, 925/700/616 wide,
1203/1210/1166 larger and 595/499/1175 sparse. Remaining stock losses are
substantial: **2.015 / 2.130 / 1.607** for small fast/serial/low. Wide1.136 and
larger 1.038 meet the 1.20 ceiling only at these medians; sparse0.410 beats the
0.90 aspiration. This is not a universal performance guarantee.

## Controls, losses and memory

Fresh OLS C/B is 1.021, range 1.000–1.044, with marginal clocks193/193/123ms and
stock ratio1.569. Fresh slopes favor the candidate at 0.970, range 0.865–1.002,
clocks565/549/1427ms. The original losing slope control now has nine-pair median
**1.018**, range **0.804–1.255**, with **five losses**. Its marginal medians
**494→541ms** imply a larger 9.5% loss; paired and marginal summaries answer
different questions and neither is hidden. Initial all-three losses at 1.080
were not reproduced uniformly, but these mixed controls do not establish
regression freedom. No slope algorithm or arithmetic is changed by this
candidate. Retain the small, portable proposal change for its repeatable wider,
low and sparse gains; do not start another control-specific tuning cycle.

Numbers of C/B losses across the nine table rows are 0, 2, 0, 0, 2, 1, 2, 1, 5. All
individual distributions and scientific checks remain in summary.json. No new
numeric storage is introduced. Median process-peak differences (MiB) are
+1.08,+0.45,-4.36,+6.75,-2.46,-1.22,-1.64,-2.85,-1.56. Sampled physical peaks
vary; they do not change native allocation bounds or prove zero physical cost.

Corresponding candidate T4/T1 repetitions on the same fresh small input pass
all five frozen science comparisons, native rank/DoF and actual team checks.
T4/T1 median is **0.848**, range 0.680–0.953; all five T4 calls are faster.
Requests were separately grouped, not directly alternating. This is bounded
scaling evidence, not a monotonic guarantee over all core counts.

Job **7763099**: clean accounting, 365s, 28 isolated slots, maxvmem1.984G; both
rotation and wrapper terminal sentinels. No compilation, profiling, archiving
or competing fit overlaps timings. The subsequent24-fit CPU diagnostic is
separate. Combined with initial02, there are 174 headline applications, 58 native
passes, 55 stock passes and 3 initial default-stock NR comparisons. No release or
qualification claim follows.
