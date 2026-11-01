# Parallel numeric input: Linux comparison, 2026-11-01

Frozen paired comparison of numeric input loading in licensed Stata 19 MP
console on one whole SCC node (job 8386679, `-pe omp28 28 -l num_proc=28`,
host `scc-wo4`, Xeon E5-2680 v4, 28 cores; both arms in this allocation).
Same harness, cells and rule as the [Mac comparison](parallel-input-macos-20261101.md).
Raw rows: [CSV](parallel-input-linux-20261101.csv) (576 timed calls).

- Incumbent: main `8cf25f74`; candidate: `84704edb`. Both built in the job with
  Rust 1.85.1, gcc 12.2.0 and `scripts/build_linux.sh`; plugin sha256
  incumbent `b2cad921…7b95`, candidate `2daacd0f…6867`.
- Schedule: threads 4, 16 and 28; four mirrored blocks (seed 20261101); one
  warmup and three timed calls per cell/threads/process; all calls retained.
- Before timing, in the same allocation: the exactness oracle with 28 parallel
  workers passed (64 cases, 65 parallel runs, identical digests); `verify`
  found 18 window reads for the candidate and none for the incumbent.

Medians in seconds, incumbent → candidate (ratio).

| Cell | Threads | Wall | Load | Native |
|---|---:|---:|---:|---:|
| c1 1M, y x2, one FE | 4 | 0.833 → 0.612 (0.73) | 0.237 → 0.077 (0.32) | 0.634 → 0.412 |
| | 16 | 0.741 → 0.550 (0.74) | 0.244 → 0.029 (0.12) | 0.545 → 0.340 |
| | 28 | 0.759 → 0.547 (0.72) | 0.245 → 0.025 (0.10) | 0.551 → 0.334 |
| c2 1M, 64 regressors, no FE | 4 | 7.893 → 5.633 (0.71) | 2.321 → 0.648 (0.28) | 7.531 → 5.256 |
| | 16 | 6.929 → 4.682 (0.68) | 2.282 → 0.180 (0.08) | 6.567 → 4.310 |
| | 28 | 6.824 → 4.604 (0.67) | 2.252 → 0.115 (0.05) | 6.470 → 4.226 |
| c3 1M, 32 regressors, one FE | 4 | 2.671 → 1.697 (0.64) | 0.992 → 0.332 (0.34) | 2.404 → 1.406 |
| | 16 | 2.101 → 1.187 (0.57) | 1.035 → 0.120 (0.12) | 1.821 → 0.896 |
| | 28 | 1.985 → 1.115 (0.56) | 1.011 → 0.087 (0.09) | 1.731 → 0.823 |
| c4 1M, 16 regressors, 50% scattered | 4 | 1.815 → 0.984 (0.54) | 1.006 → 0.192 (0.19) | 1.523 → 0.632 |
| | 16 | 1.524 → 0.663 (0.43) | 0.937 → 0.052 (0.06) | 1.235 → 0.351 |
| | 28 | 1.435 → 0.663 (0.46) | 0.888 → 0.036 (0.04) | 1.169 → 0.341 |
| c5 1M, 8 regressors, string FE | 4 | 1.611 → 1.528 (0.95) | 0.740 → 0.663 (0.90) | 1.380 → 1.285 |
| | 16 | 1.372 → 1.228 (0.90) | 0.714 → 0.565 (0.79) | 1.165 → 0.983 |
| | 28 | 1.322 → 1.126 (0.85) | 0.718 → 0.532 (0.74) | 1.127 → 0.920 |
| c6 10M, 4 regressors, one FE | 4 | 5.454 → 3.986 (0.73) | 1.478 → 0.610 (0.41) | 4.761 → 3.306 |
| | 16 | 4.751 → 3.459 (0.73) | 1.499 → 0.236 (0.16) | 4.074 → 2.814 |
| | 28 | 4.776 → 3.294 (0.69) | 1.536 → 0.184 (0.12) | 4.085 → 2.660 |
| c7 PPML 1M, restricted, exposure | 4 | 3.438 → 3.035 (0.88) | — | 2.515 → 2.156 |
| | 16 | 2.995 → 2.639 (0.88) | — | 2.129 → 1.770 |
| | 28 | 2.997 → 2.645 (0.88) | — | 2.109 → 1.718 |
| c8 = c3 with memory(low) | 4 | 4.873 → 4.334 (0.89) | 0.785 → 0.376 (0.48) | 4.601 → 4.035 |
| | 16 | 3.971 → 3.003 (0.76) | 0.819 → 0.105 (0.13) | 3.701 → 2.719 |
| | 28 | 3.739 → 2.931 (0.78) | 0.783 → 0.078 (0.10) | 3.474 → 2.627 |

No cell meets the regression rule. OLS allocation peaks agree within 0.02%
(ratios 0.9998–1.0002). Loading time keeps falling from 16 to 28 workers, but
whole-command gains flatten because the remaining time is estimation.
