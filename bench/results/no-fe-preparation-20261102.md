# No-FE preparation candidate — November 2, 2026

The fixed revised candidate fails the Linux 16-regressor, one-worker guard (+7.3%). Johannes explicitly accepted this measured exception and authorized publication on November 2, after reviewing this report. All 15 Mac guards and the other 14 Linux guards pass. The measurements remain a descriptive comparison on one Linux host and one Mac; the failed performance guard remains unchanged.

One million observations, 1/16/64 regressors, 1/2/4/8/16 fereg workers, six balanced blocks and one retained warmup plus one measured fit per cell/block. All 1,080 observations, including warmups, are retained in the [machine-readable report](no-fe-preparation-20261102.json). The earlier [scheduling round](no-fe-centering-20261102.md) and [centered numerical round](no-fe-centered-20261102.md) remain separate. No unfavorable observation was excluded or timing round retried.

Tables show median milliseconds. Native time starts after numeric loading and includes model preparation, summary construction and certificates, as well as the solver. The load counter includes numeric access, copying and optional packed-row production; it is not a transfer-only clock. Regress provides only complete-command time. Its licenses permit eight processors on Mac and four on Linux, so requested worker counts above those caps compare different actual processor counts.

## Linux

| RHS | Workers | Beta.14 command | Candidate command | Regress command | Candidate native | Candidate load | Guard |
|---:|---:|---:|---:|---:|---:|---:|:---|
| 1 | 1 | 423.5 | 397.0 | 425.9 | 234.9 | 71.7 | pass |
| 1 | 2 | 386.3 | 330.2 | 247.3 | 158.7 | 82.6 | pass |
| 1 | 4 | 349.7 | 250.9 | 142.7 | 123.7 | 41.4 | pass |
| 1 | 8 | 314.2 | 211.1 | 141.2 | 107.2 | 21.1 | pass |
| 1 | 16 | 311.9 | 208.2 | 140.0 | 110.9 | 13.0 | pass |
| 16 | 1 | 1367.0 | 1466.3 | 892.4 | 1132.4 | 218.0 | FAIL |
| 16 | 2 | 1022.7 | 885.8 | 427.6 | 631.0 | 147.8 | pass |
| 16 | 4 | 860.8 | 585.6 | 248.6 | 410.2 | 79.9 | pass |
| 16 | 8 | 778.4 | 371.2 | 242.3 | 233.4 | 44.9 | pass |
| 16 | 16 | 768.8 | 311.4 | 243.1 | 187.9 | 30.1 | pass |
| 64 | 1 | 5069.4 | 5309.0 | 3122.4 | 4326.5 | 843.4 | pass |
| 64 | 2 | 3882.1 | 2841.1 | 1405.1 | 2209.3 | 503.7 | pass |
| 64 | 4 | 3537.2 | 1778.3 | 751.7 | 1344.5 | 295.9 | pass |
| 64 | 8 | 3235.8 | 1048.8 | 738.3 | 759.1 | 157.3 | pass |
| 64 | 16 | 3100.5 | 708.9 | 748.8 | 485.8 | 90.6 | pass |

## Mac

| RHS | Workers | Beta.14 command | Candidate command | Regress command | Candidate native | Candidate load | Guard |
|---:|---:|---:|---:|---:|---:|---:|:---|
| 1 | 1 | 200.9 | 192.1 | 317.6 | 99.7 | 49.1 | pass |
| 1 | 2 | 172.7 | 145.1 | 152.2 | 70.8 | 34.2 | pass |
| 1 | 4 | 151.3 | 115.1 | 86.4 | 57.3 | 17.8 | pass |
| 1 | 8 | 142.7 | 97.9 | 52.6 | 49.9 | 9.3 | pass |
| 1 | 16 | 143.0 | 94.4 | 52.0 | 50.7 | 5.1 | pass |
| 16 | 1 | 544.2 | 566.1 | 626.0 | 415.2 | 103.4 | pass |
| 16 | 2 | 416.0 | 350.5 | 270.1 | 241.2 | 65.0 | pass |
| 16 | 4 | 363.3 | 221.2 | 149.8 | 146.7 | 32.2 | pass |
| 16 | 8 | 332.9 | 160.0 | 85.5 | 99.4 | 17.5 | pass |
| 16 | 16 | 313.0 | 136.5 | 85.9 | 83.8 | 10.0 | pass |
| 64 | 1 | 2022.6 | 2070.7 | 1792.3 | 1702.1 | 302.4 | pass |
| 64 | 2 | 1606.5 | 1318.4 | 1079.1 | 941.8 | 314.6 | pass |
| 64 | 4 | 1354.0 | 723.6 | 567.1 | 495.4 | 164.3 | pass |
| 64 | 8 | 1115.3 | 441.1 | 297.0 | 292.8 | 86.8 | pass |
| 64 | 16 | 1156.7 | 315.6 | 297.3 | 201.0 | 53.5 | pass |

## Interpretation

On the primary Linux host, the 16-regressor, one-worker median rises from 1367.0 to 1466.3 ms (+7.3%), failing the max(5%, 1 ms) protection guard. The 64-regressor, one-worker cell rises 4.7%, just inside the guard. Every parallel cell improves relative to beta.14. At 64 regressors and 16 Rust workers, command time falls 77.1% to 708.9 ms, versus 748.8 ms for regress capped at four processors; this is not an equal-core win. At four workers each, the candidate takes 1778.3 ms versus 751.7 ms. Johannes accepted this candidate for publication; more optimization is required to consistently outperform regress.

The Mac candidate beats regress at one worker with 1 or 16 regressors and at two workers with one regressor. It is still slower in the other Mac target cells. At equal eight-worker limits, 64 regressors take 441.1 ms versus 297.0 ms for regress; at 16 Rust workers the candidate reaches 315.6 ms, while regress remains capped at eight. The goal of consistently beating regress is therefore not achieved.

For the Mac 64-regressor, 16-worker cell, command time falls 72.7% from beta.14 and the post-load native counter falls 81.2%. Loading rises from 29.4 to 53.5 ms because it now produces packed rows. Native work falls from 1066.5 to 201.0 ms. The remaining command time outside load/native counters is about 61 ms; subtraction of medians is only an approximate decomposition. Eliminating all measured loading would save at most about 17% of this candidate command time and would also eliminate useful packing, so this is not a realistic transfer-only speedup estimate.

Stable summaries run after the numeric window closes, alongside model preparation. Exact packed-owner validation, three stable summary scans and the independent original-data score certificate remain. The bulk packing arithmetic and fused residual traversal reduce native work without a new BLAS dependency. These checks and full-data traversals remain plausible next targets, but the old profile does not establish their revised costs. A new arithmetic design would need a declared scope and independent mathematical witnesses; no extra timing round is implied by these results.

The same-machine preceding centered round had 15.7% and 11.8% command regressions at 16/64 regressors with one worker. The revised candidate is 4.0% and 2.4% slower than its contemporaneous beta.14 controls, within the 5% guard. Different rounds are descriptive comparisons, not paired estimates of individual optimization effects.

Regress exposes no solver-only clock here. Comparing its complete command with fereg native time cannot identify the relative cost of the small factorization or establish which internal algorithm/library it uses. The evidence identifies the fereg command's own full-data preparation as an optimization opportunity.

## Implementation and checks

Bulk panel preparation retains the ordered high/low centering operations and divisions. Immutable extrema guard finite subtraction, and a refused tile replays the scalar calculation. The existing Gram products already used SIMD; the revised arm64 packing loop also uses NEON across eight values without per-element centering calls. Narrow panels and tails remain inline scalar arithmetic.

Eligible multithreaded no-FE fits now use the existing packed loader and compute the unchanged stable three-scan summary alongside model preparation. Exact sample, source, response and column validation governs reuse; failed bindings retain canonical construction. Continuous/intercept designs compute separate centered and raw FMA residuals during one traversal. Raw validation stays at its original reported-intercept boundary; the independent compensated original-source score pass remains. Factor layouts retain their original residual producer.

The first actual revised-binary preflight exposed the 64-column stack-binding boundary: 64 regressors plus a constant refused packed reuse. Both Stata fixtures passed and all 30 target exports were exactly the prior centered candidate, but the declared handoff requirement failed. Held timing job 8399981 was deleted before execution; superseded build 8399980 was cancelled. The repair uses the existing admitted wide descriptor only for centered eligibility, preserving allocation identity and exact values. No timing qualification ran for that preflight artifact.

The final source gate, ABI audits, 77-case unaffected replay and both actual Stata fixtures pass. Fixed-worker scientific exports repeat exactly; cross-worker science and discrete decisions agree with the unchanged independent QR references. Actual traces prove the packed rows and summaries are adopted in every multithreaded target cell. Optional outputs, other inference/storage modes and explicit solver requests retain their existing paths.

A single bounded sample of the previous frozen Mac plugin found about 568 scalar-fill/centering descendant samples among 1,612 moment-stage samples, and about 1,910 samples in the three summary scans. These overlapping, correlated stack counts are diagnostic, not an independent timing estimate. No external BLAS dependency was added.

Frozen source SHA-256: `ad2188f8bfa03bbafe3967bfc1ed90807dcc9f32cff2479012ca84b16c0f528b`. Platform plugin hashes and host evidence appear in the machine-readable report. At measurement completion no installed package, release, or Git publication had changed. Subsequent beta.15 publication is documented in [validation](../../docs/VALIDATION.md#beta15-publication-decision).
