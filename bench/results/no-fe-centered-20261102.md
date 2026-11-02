# No-FE centered numerical candidate — November 2, 2026

The candidate substantially improves parallel native work on Mac, but fails the serial regression guard. It is **not qualified for promotion**. Linux scientific preflight passes; its timing round is incomplete after an isolation-monitor startup failure. The completed scheduling-only round remains [separate](no-fe-centering-20261102.md).

## Frozen comparison

One million observations, 1/16/64 regressors and 1/2/4/8/16 fereg workers; six balanced arm permutations, one retained warmup and one measured fit per cell/block. Beta.14, candidate and Stata regress use the same resident double-precision Gaussian input and independent Householder QR references. All 540 Mac timing observations and all 30 observations from the incomplete Linux baseline arm are retained in the [machine-readable report](no-fe-centered-20261102.json). Nothing was excluded or retried.

Values below are Mac median milliseconds. Native excludes loading. Stata exposes only full-command time; these columns do not isolate its solver. The Mac Stata license permits eight processors, so the 16-worker row compares fereg 16 with regress 8. Ordinary desktop activity is retained.

| RHS | Workers | Beta.14 command | Candidate command | Regress command | Beta.14 native | Candidate native | Guard |
|---:|---:|---:|---:|---:|---:|---:|:---|
| 1 | 1 | 201.9 | 198.9 | 314.3 | 109.2 | 106.3 | pass |
| 1 | 2 | 173.8 | 151.7 | 152.3 | 99.1 | 77.2 | pass |
| 1 | 4 | 150.3 | 115.8 | 86.3 | 93.0 | 59.9 | pass |
| 1 | 8 | 142.5 | 100.1 | 52.2 | 94.7 | 52.3 | pass |
| 1 | 16 | 141.8 | 95.6 | 52.3 | 98.3 | 52.2 | pass |
| 16 | 1 | 539.3 | 623.9 | 621.7 | 390.1 | 474.2 | FAIL |
| 16 | 2 | 412.3 | 365.3 | 269.1 | 313.3 | 264.9 | pass |
| 16 | 4 | 364.3 | 226.6 | 148.7 | 293.5 | 155.7 | pass |
| 16 | 8 | 329.1 | 160.4 | 85.9 | 272.3 | 103.7 | pass |
| 16 | 16 | 314.0 | 147.1 | 85.9 | 263.4 | 96.1 | pass |
| 64 | 1 | 2006.2 | 2242.8 | 1785.0 | 1636.4 | 1874.3 | FAIL |
| 64 | 2 | 1593.1 | 1280.6 | 1076.7 | 1339.4 | 1025.9 | pass |
| 64 | 4 | 1350.3 | 738.0 | 567.2 | 1190.5 | 578.8 | pass |
| 64 | 8 | 1107.2 | 463.2 | 297.2 | 995.0 | 351.0 | pass |
| 64 | 16 | 1143.4 | 347.9 | 297.1 | 1054.2 | 259.0 | pass |

At 8–16 workers, the wider models reduce native time by roughly 2.6–4.1× relative to beta.14. The 16- and 64-regressor single-worker command medians regress by 15.7% and 11.8%, exceeding max(5%, 1 ms). Parallel full-command time generally remains slower than regress. The candidate therefore misses both the serial protection requirement and the overall speed goal.

## What the implementation changes

Three deterministic row scans build scaled, anchored means, raw/within norms and a fresh centering certificate. Immutable centered views feed bounded row panels and deterministic product reductions. The usual factor is guarded Cholesky; a Householder compression retry cannot bypass the canonical strong rank guard. Original-source residual/score and inverse/covariance checks retain their tolerances. No raw-total subtraction supplies centered cross-products or RSS. Ambiguous classification, rank boundaries and failed numerical checks return once to the canonical path.

The approved final residual shortcut removes a second projection when its independent certificate passes. The timing cells use centered projection, row products, Cholesky and the derived residual row. TSQR has production-kernel retry tests, but no actual-plugin timing evidence.

The target cells report zero loaded/reused rows. Inspection identifies the immediate blocker: the plugin’s `numeric_pipeline_effects_supported` explicitly requires a nonempty absorbed-term list. Loader-summary handoff exists but was not reached here, so none of these measured gains establish loader fusion or copy savings. The previous path already moved its packed matrix and centered it in place.

## Remaining native costs

Preflight phase counters are diagnostic, not controlled timing comparisons. For 64 regressors, one worker spends approximately 724 ms in preparation/projection, 711 ms in finite products/solve and 318 ms in bread/original residual checks. At 16 workers these are about 84, 62 and 71 ms, followed by roughly 20 ms in the derived-residual check. Three summary scans and repeated original-data checks still cost substantially; the small matrix factorization is not the only bottleneck.

The next proposed candidate should reduce full-data passes, reach the actual no-FE loader handoff and combine compatible independent residual/RSS/score reductions. Preserve stable arithmetic, canonical rank authority and the serial guard. Adding a BLAS dependency alone would not remove those scans. Any next candidate requires a new declaration and bounded round; this report is not substituted.

## Science, scope repair and Linux interruption

Both actual plugins pass their 77-case unaffected replay exactly and the direct/centered Stata fixtures. Mac completes 630 scientific exports; Linux preflight completes 90. Independent maximum errors are 3.26e-14 for coefficients, 1.59e-19 for covariance entries and 4.98e-14 relative RSS. Fixed-worker repeats are exact; cross-worker science and discrete decisions agree. Extreme-scale witnesses use independent orthogonal mathematics where regress drops a tiny regressor.

Final review found that `xb()` alone bypassed the intended no-output restriction. A separate no-FE eligibility flag now excludes all optional observation outputs, including `dropreason()`, while leaving other FE output policies unchanged. This repair is separate from the frozen binaries; all timing cells request no outputs. The repaired universal Mac plugin passes its ABI audit, 77 unaffected replay cases, and both actual Stata fixtures. The internal xb adapter stays on the canonical route with exact b/V/RSS/predictions. Public OLS obtains xb through postestimation; the adapter is the relevant output-request surface. All 30 no-output scientific exports (15 cells, two repetitions) match the frozen candidate byte for byte, and their traces retain the same centered/Cholesky/derived routes. No new timing qualification was run.

Linux preflight job 8398093 passed on scc-tb4. Measurement job 8398094 used a verified whole 28-core allocation on scc-wd2 and exited 1 after the first baseline arm. The monitor sampled processes, then spent 2.620 seconds querying scheduler census, then began a five-second wait. Its first interval consequently reached 7.708 seconds, exceeding the frozen 7.5-second limit. No competing computation was observed, but the required observation coverage failed, so Linux timings provide no comparison. The monitor now anchors its next deadline to the existing sample; all 21 focused monitor tests pass without changing the observation limits. This attempt is retained without resampling.

The earlier Linux launcher first stopped before preflight because it attempted to recreate the existing timing-clock plugin; its dependent measurement stopped immediately at the prerequisite. The bounded repair verified and reused the completed native build, immutable clock and passed replay. It changed neither the candidate nor the inputs.

Frozen source SHA-256: `984b8d4e8784d581a2f1f5482aeb20357c0f8782179fac84be5f0cd8fd6d7aa6`. Frozen Mac plugin: `5d53dde2d1ba9cb0a5b0ac8f9ebdd207f6b2e1e149d27f2b822d0414b55291ac`. Frozen Linux plugin: `1af8991823cdbedd8a7eca3936159db5dee32c6315512846bc8c4385a744bafc`.

No installed package, release or Git publication has changed. Native Intel Mac and Windows execution are not qualified by this work.

Repaired Mac plugin SHA-256: `6bdb73ac58f5f3138415d9db129274e5175d88bc4de80ff8ca101a6d329407d0`. Exact no-output scientific export SHA-256: `5c1bca57104b56692541bb60f52e57cf3e20b25c43179e00eb273a7eda978741`. Native source checks pass; the additional 436-test benchmark suite has three missing-PDF-dependency import errors and one unchanged input-report test failure. Focused no-FE and monitor tests pass.
