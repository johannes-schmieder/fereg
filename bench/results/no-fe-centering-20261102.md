# No-FE structural scheduling comparison — November 2, 2026

The first candidate reduces no-FE native work on both measured platforms. All exported scientific results match beta.14 exactly at fixed worker counts, and all 30 full-command median comparisons remain inside the max(5%, 1 ms) regression guard. It does not reach native Stata regress speed in the wider parallel cases.

This is a descriptive target panel, not full protected routing qualification or release approval. The subsequent direct-centering and approved residual-shortcut candidate is a separate round and is absent from these measurements.

## Protocol

One million observations; 1, 16 or 64 RHS variables; 1, 2, 4, 8 or 16 fereg workers. The same double-precision Gaussian dataset remains resident with all 64 regressors in every arm. Independent SciPy Householder QR supplies coefficients, covariance and RSS. Six mirrored/rotated blocks use all six permutations of beta.14, candidate and regress. Each cell has one retained warmup and one measured fit per block, with science exported outside the clock. No observations were excluded or retried. Full-command time uses a monotonic clock; fereg native time is runtime row 20, excluding loading.

Linux ran on one physical host, scc-wf2, in a verified isolated 28-slot allocation (job 8396235, exit zero). The Stata license permits **four processors**, so requested 8/16-worker rows compare fereg 8/16 with regress 4. Mac used an M2 Ultra with ordinary desktop activity retained; its Stata license permits **eight processors**, so its 16-worker rows compare fereg 16 with regress 8. One Linux host does not establish a multi-host performance qualification.

## Results

All values are median milliseconds. Native columns exclude loading; regress exposes only full-command time.

### Linux

| RHS | fereg workers | regress processors | beta.14 command | candidate command | regress command | beta.14 native | candidate native |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 1 | 1 | 468.7 | 445.1 | 348.2 | 259.3 | 234.3 |
| 1 | 2 | 2 | 397.1 | 345.0 | 197.8 | 226.9 | 189.5 |
| 1 | 4 | 4 | 323.6 | 270.6 | 134.8 | 217.8 | 164.9 |
| 1 | 8 | 4 | 301.0 | 245.2 | 149.3 | 201.6 | 147.9 |
| 1 | 16 | 4 | 301.8 | 242.3 | 137.2 | 204.7 | 146.8 |
| 16 | 1 | 1 | 1452.3 | 1344.3 | 874.5 | 1069.9 | 971.9 |
| 16 | 2 | 2 | 1102.2 | 858.6 | 389.9 | 811.1 | 572.0 |
| 16 | 4 | 4 | 890.0 | 611.2 | 236.5 | 703.9 | 424.1 |
| 16 | 8 | 4 | 801.9 | 505.2 | 234.9 | 654.6 | 367.2 |
| 16 | 16 | 4 | 763.9 | 516.8 | 234.7 | 631.3 | 386.2 |
| 64 | 1 | 1 | 6031.6 | 5749.5 | 3709.8 | 4404.9 | 4168.9 |
| 64 | 2 | 2 | 4303.1 | 3418.5 | 1565.0 | 3483.0 | 2599.7 |
| 64 | 4 | 4 | 3697.5 | 2582.4 | 859.2 | 3177.8 | 2078.1 |
| 64 | 8 | 4 | 3175.1 | 2102.4 | 837.7 | 2846.9 | 1780.8 |
| 64 | 16 | 4 | 2992.8 | 1935.9 | 834.5 | 2743.0 | 1684.9 |

### Mac

| RHS | fereg workers | regress processors | beta.14 command | candidate command | regress command | beta.14 native | candidate native |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 1 | 1 | 199.6 | 197.3 | 313.8 | 108.5 | 105.8 |
| 1 | 2 | 2 | 173.4 | 163.7 | 151.5 | 99.1 | 89.9 |
| 1 | 4 | 4 | 152.3 | 138.3 | 86.1 | 96.4 | 82.6 |
| 1 | 8 | 8 | 141.9 | 126.2 | 52.0 | 94.3 | 78.2 |
| 1 | 16 | 8 | 141.9 | 123.9 | 51.8 | 98.3 | 80.0 |
| 16 | 1 | 1 | 532.2 | 502.3 | 620.4 | 383.1 | 354.0 |
| 16 | 2 | 2 | 409.9 | 319.7 | 268.9 | 311.6 | 221.6 |
| 16 | 4 | 4 | 363.1 | 230.0 | 149.1 | 293.0 | 159.5 |
| 16 | 8 | 8 | 333.1 | 198.5 | 86.0 | 276.6 | 141.5 |
| 16 | 16 | 8 | 314.3 | 207.2 | 85.6 | 263.6 | 154.2 |
| 64 | 1 | 1 | 1989.2 | 1929.7 | 1768.3 | 1624.6 | 1563.7 |
| 64 | 2 | 2 | 1575.6 | 1238.7 | 1079.7 | 1321.6 | 985.3 |
| 64 | 4 | 4 | 1346.9 | 967.0 | 566.7 | 1188.3 | 808.2 |
| 64 | 8 | 8 | 1093.3 | 689.5 | 297.2 | 982.1 | 577.8 |
| 64 | 16 | 8 | 1140.3 | 706.8 | 296.7 | 1051.7 | 617.5 |

## Science and limitations

Each platform retained 630 scientific observations, including preflight, warmups and measured fits. Candidate/incumbent science is exact, and fixed-worker repeats match. Maximum independent errors were 3.26e-14 for coefficients, 1.59e-19 for covariance entries and 4.98e-14 relative RSS. Source integration, independent numeric witnesses, both platforms’ 77-case actual-plugin replay, Mac transaction/parity checks and public OLS/PPML Break rollback pass. Native Intel Mac and Windows execution were not tested for this candidate.

The changes retain observation-order compensated means, fixed 512-row norm leaves and ascending merges. They reuse the initial structural means, schedule independent column bands and subtract over disjoint rows. Numerical and route-change scopes are empty. No solver, rank rule or tolerance changed.

The Linux build reservation was corrected from 8 to 16 slots before scientific preflight; the completed 16-slot build job 8396249 exited zero. This administrative repair preceded timing.

[Machine-readable evidence](no-fe-centering-20261102.json) retains every timing observation, medians, scientific bounds, allocation checks, artifact hashes and the frozen order. The source archive hash identifies the first candidate; the later direct path is not part of that source or those binaries.
