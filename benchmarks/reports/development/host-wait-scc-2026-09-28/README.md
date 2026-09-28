# Host-wait diagnostic: no blanket production change

Job **7763005** completes **45 fits**, all **30 scientific comparisons** passing,
on the unchanged accepted plugin `8e2d5af2…` / source `a7b46c2f` (snapshot
`1f290d1c…`). The same native thread request and affinity apply to all arms.
Normal host waiting is compared with passive OpenMP waiting and scoped Stata
processors1. Setting and restoring processors are inside the command timer;
restoration is asserted after warm-up and estimation. No production setting,
arithmetic, tolerance, sample or frozen benchmark definition changed.

| Case | Pairs | Default / passive / scoped-host1 command medians (ms) | Passive/default paired median [min,max] | Scoped/default paired median [min,max] |
|---|---:|---:|---:|---:|
| OLS N61,440/K10/T8 | 6 | 182 / 162 / 167 | 0.873 [0.866,0.934] | 0.912 [0.882,0.934] |
| Categorical PPML N30,720/K6/T4 | 6 | 450 / 437.5 / 434.5 | 0.973 [0.922,1.084] | 0.979 [0.889,1.125] |
| Fresh two-partition slope PPML N30,720/K6/T4 | 3 | 573 / 600 / 612 | 1.077 [1.005,1.161] | 1.099 [1.007,1.125] |

All OLS pairs improve; each categorical PPML variant loses two of six pairs;
every slope pair loses for both variants. These diagnostic rotations contain no
stock arm and do not revise earlier stock ratios. Public-command comparisons
include IID OLS or robust PPML and no optional saved observation outputs.
Original OLS seed202609284 and PPML seed202609282 match captured hashes; the
slope control uses predeclared fresh seed202609286. Native coefficients and SEs
match exactly in all pairs; full covariance, objective, sample, rank/DoF and
actual requested/effective-team checks pass under existing envelopes.

The wait-loop CPU attribution predicts a useful OLS question but not a broad
solution. Neither intervention reliably closes the small PPML loss. Reject a
blanket host-setting change: it would trade demonstrated slope regressions for
one categorical OLS gain. An OLS-specific scoped intervention needs fresh
control-plane/transaction tests and broader case evidence before adoption;
process-start OpenMP settings are not a transparent public-command substitute.
Large sampled wait-loop shares must not be presented as wall-time savings.

The next small PPML target is the profiled native product work. Any change must
distinguish speculative current-weight operator applications from Gram/rank and
independent certificate authority. The earlier sparse-incidence experiment
remains rejected; a portable instruction-call optimization would require its
own Linux scientific and complete-command evidence.

Accounting is clean: failed=0, exit_status=0, **83 seconds**, 28 isolated slots
on scc-wo4, **1.804G maxvmem**, all application and wrapper terminal sentinels.
No compilation, profiling or competing fits overlapped these measurements.
Raw licensed logs stay on SCC; JSON/CSV hashes bind the collected evidence.
This closes the bounded host-wait diagnostic, not the broader performance plan.
