# Preliminary Veneto results audit

Fixed snapshot: September 12, 2026, approximately 10:23 UTC. This audit covers **nine complete comparison tasks, 99 command invocations**: all six cold CPU settings and the first warm repetition at 1, 2 and 4 cores. Incomplete tasks are excluded from the comparisons. There are no three-warm-repetition medians yet. The ongoing campaign is unchanged.

The main finding is that **fereg OLS is fast and numerically consistent with reghdfe, with higher memory use; feppml is not yet reliable on this Linux real-data case**. Several fast-looking comparator results require qualifications. Most elapsed time is spent on PPML failures and timeouts.

## Evidence and identity

- 3,159,720 observations, 1,133,933 workers, 125,707 firms; 1999–2001 dominant-job panel, common singleton pruning, worker and firm effects, four controls.
- Frozen candidate source `d44e36bfa5ad7e41a06752d1008f0b9683f8c8dd`, Linux plugin SHA-256 `74d3ce3bd0c01332aaf3a38aea7ee96b7b31012eb6ef0d00782e1d29a6895df9`, campaign fingerprint `bedd0cbb29c066ea31a2401363f62d21e6fc1f0c2d7f24acc00a6e19a055c423`. These measurements do **not** test the later uncommitted OLS changes currently in the working tree.
- Each of the nine tasks has scheduler `failed=0`, `exit_status=0`, 28 slots, an application completion marker, and all 11 distinct expected receipts. Runtime, model-file, core count, repetition, and same-node identities were checked. No estimator was rerun for this analysis.
- `audit.json`, `time-memory.csv`, `accounting.json`, and `snapshot-hashes.json` beside this report preserve the analysis, comparisons, accounting, and collected evidence identities. The full private snapshot, including logs and covariance/solver files, is in ignored `work/audit-20260912T1023/`.
- This is a VWH-derived computational benchmark, not a replication of KSS's leave-out selection or variance-component estimates. The full-model independent oracle has not been run. The earlier bounded oracle covers 3,732 real observations locally; Linux smoke covered 7,680 synthetic observations at 1/28 cores.

## Runtime: measured cold calls, seconds

Each table entry is one call, not a median. `E` is elapsed time before a native command error; `NC` is a returned nonconverged fit; `T` is a timeout. None of these are successful-fit times.

### OLS

| Command | 1 | 2 | 4 | 8 | 14 | 28 |
|---|---:|---:|---:|---:|---:|---:|
| fereg | 23.5 | 14.1 | 12.4 | 7.5 | 9.2 | 6.1 |
| reghdfe | 293.2 | 174.6 | 131.0 | 127.5 | 134.9 | 133.1 |
| greg | 69.8 | 67.8 | 70.8 | 63.7 | 69.3 | 64.0 |
| areg | 32.0 NC | 22.2 NC | 31.8 NC | 28.8 NC | 31.1 NC | 29.0 NC |
| fixest | 41.4 | 28.1 | 24.3 | 12.9 | 14.3 | 13.3 |
| pyfixest | 1225.7 | 733.1 | 523.6 | 323.2 | 348.0 | 330.4 |

### PPML

| Command | 1 | 2 | 4 | 8 | 14 | 28 |
|---|---:|---:|---:|---:|---:|---:|
| feppml | 20.8 E | 14.7 E | 15.4 E | 10.4 E | 152.0 | 9.7 E |
| ppmlhdfe | 1466.4 | 948.4 | 737.8 | 707.7 | 734.3 | 707.6 |
| gglm | ≥2,400 T | ≥2,400 T | ≥2,400 T | ≥2,400 T | ≥2,400 T | ≥2,400 T |
| fixest | 499.8 | 339.5 | 255.4 | 160.5 | 177.9 | 164.2 |
| pyfixest | ≥2,400 T | 2266.4 E | 1428.1 E | 987.4 E | 1009.2 E | 971.4 E |

Fereg/reghdfe time ratios imply 12.45x, 12.41x and 10.55x speed advantages at 1/2/4 cores in these cold calls. At those settings Stata and the Rust engine have the same allowed core count. At 28 affinity cores the observed ratio is 21.71x, but reghdfe can use only four licensed Stata processors. That is an installed-environment result, not an equal-28-thread algorithm comparison.

Fereg's cold time falls from 23.55 to 6.13 seconds between 1 and 28 cores: 3.84x speedup, far below linear scaling. It is 1.56–2.17x faster than R fixest across these cold calls, but native fixest does not satisfy the same strict accuracy/SE comparison. The 8-to-14-core regression is real in this snapshot, but cannot yet be attributed to a solver or thread effect: core counts alternate across two physical hosts. `scc-wo4` supplied 1/4/14 and `scc-wj3` supplied 2/8/28 in the cold pass. Same CPU model does not eliminate host, NUMA, or clock differences.

The available warm repetition is close to cold at 1/2/4 cores. Fereg changes by +0.54%, −1.48%, +2.96%; R fixest by +3.28%, +0.87%, +0.24%; PyFixest OLS by −0.02%, −0.06%, −0.45%. This suggests one-time initialization is not explaining the large OLS gaps here, but one warm call is insufficient for stable dispersion estimates.

## Memory: absolute sampled process RSS, GiB

These numbers include the loaded dataset and runtime. They are not incremental estimator allocations. Error/time-out entries show only memory observed before termination and are not comparable to completed-fit memory requirements.

### OLS

| Command | 1 | 2 | 4 | 8 | 14 | 28 |
|---|---:|---:|---:|---:|---:|---:|
| fereg | 1.66 | 1.72 | 2.30 | 2.81 | 3.11 | 3.99 |
| reghdfe | 1.02 | 1.02 | 1.02 | 1.02 | 1.02 | 1.02 |
| greg | 0.83 | 0.83 | 0.83 | 0.84 | 0.84 | 0.83 |
| areg | 1.04 NC | 1.06 NC | 1.05 NC | 1.04 NC | 1.05 NC | 1.05 NC |
| fixest | 0.95 | 1.01 | 1.13 | 1.22 | 1.22 | 1.22 |
| pyfixest | 1.47 | 1.53 | 1.66 | 1.66 | 1.68 | 1.67 |

### PPML

| Command | 1 | 2 | 4 | 8 | 14 | 28 |
|---|---:|---:|---:|---:|---:|---:|
| feppml | 2.45 E | 2.60 E | 3.15 E | 3.81 E | 4.92 | 5.91 E |
| ppmlhdfe | 1.60 | 1.60 | 1.59 | 1.59 | 1.59 | 1.59 |
| gglm | 1.14 T | 1.14 T | 1.14 T | 1.14 T | 1.14 T | 1.14 T |
| fixest | 1.86 | 1.96 | 2.16 | 2.26 | 2.26 | 2.26 |
| pyfixest | 2.18 T | 2.24 E | 2.37 E | 2.39 E | 2.41 E | 2.38 E |

Fereg OLS rises from 1.66 GiB at one core to 3.99 GiB at 28, versus approximately 1.02 GiB for reghdfe and 0.95–1.22 GiB for R fixest. Relative to reghdfe, the measured memory ratio rises from 1.62x to 3.89x. Thus this case shows a speed–memory tradeoff, not a memory win. Fresh processes prevent these observations from establishing a leak. The largest scheduler `maxvmem` among the nine tasks is 10.203 GiB, well below the 84 GiB reservation; it is virtual memory and must not be substituted for estimator RSS.

The RSS controller sleeps for 10 ms between polls but also reads shared-filesystem markers and process state. Duration/sample-count ratios imply approximately **15–25 ms average sampling intervals** in this snapshot. The original 10 ms label is the requested sleep, not measured sampling frequency; brief peaks may be missed. The final report now makes this distinction.

## Numerical results and returned status

The coefficient and SE checks use the previously registered relative/absolute thresholds (coefficients 1e-7/1e-10; SEs 1e-6/1e-10) against the same task's pinned reghdfe or ppmlhdfe. Full 4×4 slope covariance matrices were also compared. Reference self-comparisons are included below; 0/0 means no returned coefficient comparison.

| Model / command | Audited outcomes | Slope checks | SE checks |
|---|---|---:|---:|
| OLS / fereg | 9 pass | 9/9 | 9/9 |
| OLS / reghdfe | 9 pass | 9/9 | 9/9 |
| OLS / greg | 9 pass | 9/9 | 0/9 |
| OLS / areg | 9 nonconvergence | 0/9 | 0/9 |
| OLS / fixest | 9 pass | 0/9 | 0/9 |
| OLS / pyfixest | 9 pass | 9/9 | 0/9 |
| PPML / feppml | 1 pass, 8 worker_failure | 1/1 | 1/1 |
| PPML / ppmlhdfe | 9 pass | 9/9 | 9/9 |
| PPML / gglm | 9 command_timeout | 0/0 | 0/0 |
| PPML / fixest | 9 pass | 0/9 | 9/9 |
| PPML / pyfixest | 2 command_timeout, 7 worker_failure | 0/0 | 0/0 |

### fereg OLS

All nine fits pass slope, SE, full covariance and exact observed-sample comparisons. Maximum slope difference is **7.44e-12**; maximum relative SE difference is **7.18e-11**; RSS differs by around 2.4e-8 on a level of 83,825 (roughly 3e-13 relative). Residual degrees of freedom agree at 1,941,977. Across these nine completed comparisons, fereg's timing advantage against reghdfe is well supported as a development diagnostic for this fixed design, not as release qualification or a general performance result.

### Disconnected graph and OLS covariance conventions

A separate SciPy graph calculation using only the delivered worker/firm IDs finds **41,901 connected components**. Of those, 39,931 contain one firm; the largest component contains 2,905,231 observations (91.95% of the panel). The exact FE-indicator rank is 1,133,933 + 125,707 − 41,901 = 1,217,739. Subtracting this and four identified controls gives 1,941,977 residual degrees of freedom.

Fereg and reghdfe use this value. Default areg, R fixest, and PyFixest use 1,900,077 instead (directly reported or implied by N minus the reported parameter count). The difference is precisely **41,900 = components minus one**. The corresponding SE multiplier is sqrt(1,941,977 / 1,900,077) = **1.010965745**, explaining the approximately **1.0966%** SE excess. Greg's near-uniform SE ratio is consistent with that convention; its receipt does not expose the residual DoF, so the convention is inferred rather than directly observed.

As a diagnostic only, rescaling PyFixest's SEs to the reference DoF leaves a maximum relative gap near **6.3e-11**; its reconstructed inverse within cross-product matrix also agrees closely. R fixest retains around **2.05e-6** relative SE discrepancy after the DoF rescaling, reflecting a smaller numerical difference in addition to the convention. The estimated within cross-product condition number from the fereg covariance is about **49,653**, and control-coefficient correlations are high, making separate attention to solver accuracy useful. No covariance output or benchmark command was changed by the diagnostic rescaling.

### areg: nonconvergence hidden by the collector

All nine areg calls returned `rc=0` but explicitly reported **`e(converged)=0`**, with the default 50-iteration setting. The [official Stata areg manual](https://www.stata.com/manuals/rareg.pdf), stored-results section, defines zero as nonconvergence. The collector checked `converged is False`; Stata values were parsed as `0.0`, so it mislabeled them `pass`.

The original receipts are preserved. A tested postprocessing correction now labels these calls **nonconvergence** in this audit and the final report. Their relatively small slope differences (maximum 1.19e-7, or 2.06e-5 of a reference SE) do not turn them into converged fits. Increasing iterations would define a separate comparison; no such change or rerun was made.

### R fixest and PyFixest OLS

R fixest returns convergence but misses the strict slope comparison in all nine calls: maximum absolute difference **1.87e-6**, only **0.000326 reference SEs**. Its RSS is about 0.00113 above reghdfe, a 1.35e-8 relative difference. These are economically tiny differences for the reported slopes, yet they fail this benchmark's strict equivalence criterion. Its larger SE disagreement is mainly the DoF convention just described.

PyFixest OLS and greg pass the slope threshold in every completed call. PyFixest's maximum slope difference is 1.29e-10; greg's is 4.76e-10. Their approximately 1.1% SE discrepancies are systematic. Greg exposes only a sample count, whereas observation-ID hashes match for fereg, reghdfe, areg, R fixest and PyFixest where a fit returns. All returned sample counts equal the full input N; the greg observation-level sample proof remains weaker.

### feppml: the principal reliability problem

Eight of nine completed calls return **native `r(430)`**, at cold 1/2/4/8/28 cores and warm 1/2/4 cores. In fereg's documented contract, this means nonconvergence or a failed numerical certificate. They fail after roughly 10–21 seconds and never publish coefficients. The sole success in this snapshot is **14 cores**, at 152.01 seconds. It matches ppmlhdfe's slopes (maximum 2.19e-12 difference), SEs (2.36e-8 relative), full covariance, and exact observation sample. It reports 15 outer iterations, 594 inner iterations, ten step halvings, and one strict-profile rejection. FE projection and strict final solving dominate its recorded time. No singleton or separation deletions occur in the successful fit.

That isolated success is 4.83x faster than ppmlhdfe and 1.17x faster than R fixest at the same affinity setting, but **it is not a usable PPML scaling result or evidence of reliable dominance**. The pattern is consistent with sensitivity of the numerical route to thread-dependent arithmetic, but does not locate or prove the cause. The same frozen Linux binary and data can succeed, and the earlier Mac pilot at four threads also succeeded, so blanket data invalidity is not a sufficient explanation.

The benchmark used `capture quietly`, which retained the return code but suppressed the detailed native failure message. Its generic `result_extraction_or_status` label merely means the command returned and wrote DONE; it does **not** prove result extraction was the source of the error. Existing evidence cannot distinguish the exact FE solve, IRLS stage, or final certificate that failed. A focused logged diagnostic using the same frozen Linux binary and unchanged input/options would be the next investigation, kept separate from these measurements.

For the successful feppml fit, reported deviance is 0.002401 lower and log likelihood 0.001201 higher than ppmlhdfe, tiny changes relative to their levels (~2.89e7 and −2.48e7). Slopes/covariance agreement and those reported objectives do not replace a full-data predicted-mean/model-space/KKT audit. Feppml's absorbed DoF differs by one from the reference because its reported constant shifts the bookkeeping; the total model rank and residual DoF agree.

### Other PPML commands

Pinned ppmlhdfe completes all nine calls, retaining the same N and sample. R fixest also completes all nine and passes the SE threshold. Its largest slope discrepancy is **6.76e-8**, about **7.31e-7 reference SEs**, so it fails the strict slope threshold despite negligible inferential size. Its reported deviance is slightly higher than ppmlhdfe (around 0.0104 at the 14-core comparison). These are native-default results, without tightened tolerances.

PyFixest PPML never completes successfully: the two one-core calls time out at 2,400 seconds; the remaining seven calls return `ValueError: Demeaning failed after 10000 iterations.` Gglm reaches the 2,400-second command cap in all nine tasks. A gglm timeout does not establish that it could never converge; its completion time and final memory remain unknown.

## Why the campaign is slow, and what can be concluded now

The nine tasks consume **16.19 node-hours**, including loading, audits and result export. Their measured command durations plus timeout bounds total **15.85 hours**. Gglm PPML alone consumes **6.00 hours (37.9%)**, and PyFixest PPML consumes **4.24 hours (26.7%)**. Together these unsuccessful PPML attempts use **64.6%** of command time. Fereg OLS uses 123 seconds across all nine tasks. Queue delay and only two concurrent nodes add elapsed calendar time; they do not explain the slow individual commands.

The most defensible current conclusions are:

1. Fereg OLS is substantially faster than pinned reghdfe on this frozen real-data design and agrees numerically, with materially higher RSS.
2. This is not a symmetric 28-thread Stata comparison: the actual SCC license cap is **four processors**. Rust and R/Python receive the requested limits; reghdfe and ppmlhdfe flatten beyond four Stata processors. Host changes also confound fine interpretation of the scaling curve.
3. Feppml's Linux real-data failures must be explained before making a reliability or scaling claim. They are a more important finding than its single fast successful result.
4. Native comparator defaults produce both convergence failures and DoF convention differences. A future matched-convergence/matched-DoF experiment would answer a different question and should have its own identity.
5. The report correction is safe to apply to all future collected receipts because it reads an existing native convergence flag. It does not alter measurements, source, samples, estimation options or running jobs.

The pending final PDF must retain the failures and these qualifications. The audit provides no reason to change or silently discard the existing campaign results.
