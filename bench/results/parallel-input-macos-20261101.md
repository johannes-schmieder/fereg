# Parallel numeric input: Mac comparison, 2026-11-01

Frozen paired comparison of numeric input loading in licensed Stata 19.0 MP
console (12 Aug 2026) on an Apple M2 Ultra (16 performance + 8 efficiency
cores, 192 GB, macOS 26.6.2; Stata licensed for 8 cores), run by
`bench/parallel_input.py` with `bench/parallel_input.do` on inputs from
`bench/parallel_input_data.do`. Raw rows: [CSV](parallel-input-macos-20261101.csv)
(384 timed calls).

- Incumbent: main `8cf25f74`, arm64 plugin sha256 `bba6731e…0d6b`.
- Candidate: `84704edb`, arm64 plugin sha256 `aa7eef3f…5677` (the qualified
  build). Both built from source with the same release flags; each arm used its
  own ado tree.
- Schedule (declared before timing): threads 4 and 8, four mirrored blocks
  (order from seed 20261101), one warmup and three timed calls per
  cell/threads/process. Every timed call is retained.
- Path check (`verify`): candidate 18 window reads across its 16 calls (8
  cells, warmup plus one; PPML adds its exposure read); incumbent none.
- Disclosure: a background ssh poll (one call per minute to the SCC queue) ran
  on this Mac during the timed run; it was not stopped and is retained as noise.

Medians (seconds; `load` is OLS `e(fereg_runtime)[19,2]`, complete input
loading; `native` is the plugin call; `wall` is Stata's timer around the whole
command). Allocation peaks are identical in every OLS cell.

| Cell | Threads | Wall inc → cand | Load inc → cand | Native inc → cand |
|---|---:|---:|---:|---:|
| c1 1M, y x2, one FE (pipeline) | 4 | 0.208 → 0.141 (0.68) | 0.092 → 0.025 (0.27) | 0.160 → 0.092 |
| | 8 | 0.199 → 0.120 (0.60) | 0.093 → 0.014 (0.15) | 0.151 → 0.072 |
| c2 1M, 64 regressors, no FE (batched) | 4 | 1.662 → 1.453 (0.87) | 0.312 → 0.098 (0.31) | 1.589 → 1.379 |
| | 8 | 1.428 → 1.173 (0.82) | 0.311 → 0.051 (0.16) | 1.355 → 1.099 |
| c3 1M, 32 regressors, one FE | 4 | 0.544 → 0.450 (0.83) | 0.166 → 0.069 (0.42) | 0.485 → 0.391 |
| | 8 | 0.453 → 0.313 (0.69) | 0.166 → 0.037 (0.22) | 0.395 → 0.255 |
| c4 1M, 16 regressors, 50% scattered sample | 4 | 0.394 → 0.220 (0.56) | 0.218 → 0.041 (0.19) | 0.304 → 0.129 |
| | 8 | 0.376 → 0.186 (0.49) | 0.215 → 0.022 (0.10) | 0.286 → 0.095 |
| c5 1M, 8 regressors, string FE | 4 | 0.435 → 0.352 (0.81) | 0.274 → 0.188 (0.69) | 0.383 → 0.300 |
| | 8 | 0.415 → 0.318 (0.77) | 0.272 → 0.177 (0.65) | 0.363 → 0.266 |
| c6 10M, 4 regressors, one FE | 4 | 1.075 → 0.920 (0.86) | 0.258 → 0.095 (0.37) | 0.997 → 0.842 |
| | 8 | 0.954 → 0.757 (0.79) | 0.258 → 0.056 (0.22) | 0.876 → 0.678 |
| c7 PPML 1M, 4 regressors, restricted, exposure | 4 | 1.000 → 0.866 (0.87) | — | 0.529 → 0.398 |
| | 8 | 0.956 → 0.805 (0.84) | — | 0.484 → 0.334 |
| c8 = c3 with memory(low) | 4 | 0.919 → 0.819 (0.89) | 0.166 → 0.054 (0.32) | 0.861 → 0.761 |
| | 8 | 0.768 → 0.634 (0.82) | 0.166 → 0.029 (0.17) | 0.709 → 0.575 |

No cell meets the regression rule (candidate median wall above incumbent by
more than max(5%, 1 ms)). The string-FE cell gains least because its string
column is still read on Stata's thread after the numeric window, by design.
