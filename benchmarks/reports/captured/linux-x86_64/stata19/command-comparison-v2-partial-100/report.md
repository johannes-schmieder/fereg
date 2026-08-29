# fereg V2 completed-scope benchmark analysis

This report freezes the 100 scopes that passed the repository's strict shard validator before the remaining SCC work was canceled. Canceled, timed-out, partial, and queued scopes are excluded. Ratios below are warm-median fereg time divided by comparator time; values below 1 favor fereg.

- SCC run root: `/projectnb/welfgr/fereg/runs/20260828T031754Z-command-v2-d452dbe`
- Benchmark source: `d452dbeab958b44c8d106d2147acf25613bceb48`
- Parent array: `7342059`; held collector: `7342062`
- Remaining parent, collector, and isolated-retry jobs were canceled on 2026-08-28.
- This directory also contains the accepted-scope inventory and machine-readable summaries used to produce the tables.

## Evidence integrity

- Accepted scopes: 100 of 116
- Timing rows: 3600 (exactly 36 per scope)
- Scientific preflights: 100; failures: 0
- Commands: fereg, greg, reghdfe
- Stata: 19 IC; license processors=4; machine processors=16
- Missing scope indices: 92, 95, 96, 99, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116

| scale | design | accepted | planned |
|---|---|---|---|
| n10m | akm_lowmob | 4 | 8 |
| n10m | akm_lowmob_year | 0 | 8 |
| n10m | akm_standard | 14 | 14 |
| n10m | akm_standard_year | 10 | 14 |
| n10m | one_fe | 14 | 14 |
| n1m | akm_lowmob | 8 | 8 |
| n1m | akm_lowmob_year | 8 | 8 |
| n1m | akm_standard | 14 | 14 |
| n1m | akm_standard_year | 14 | 14 |
| n1m | one_fe | 14 | 14 |

## Overall paired comparisons

| mode | comparator | cells | geomean ratio | median | p10 | p90 | fereg wins |
|---|---|---|---|---|---|---|---|
| fit | greg | 100 | 3.290 | 2.900 | 1.054 | 10.630 | 7/100 |
| fit | reghdfe | 100 | 0.712 | 0.595 | 0.271 | 2.154 | 63/100 |
| save_fe | greg | 100 | 1.983 | 1.934 | 0.346 | 8.639 | 21/100 |
| save_fe | reghdfe | 100 | 0.599 | 0.520 | 0.226 | 2.129 | 77/100 |

## By design

| mode | comparator | design | cells | geomean ratio | median | p10 | p90 | fereg wins |
|---|---|---|---|---|---|---|---|---|
| fit | greg | akm_lowmob | 12 | 0.970 | 1.020 | 0.566 | 1.699 | 5/12 |
| fit | greg | akm_lowmob_year | 8 | 2.374 | 2.320 | 1.014 | 7.389 | 1/8 |
| fit | greg | akm_standard | 28 | 2.405 | 1.954 | 1.292 | 7.638 | 1/28 |
| fit | greg | akm_standard_year | 24 | 3.506 | 3.055 | 1.960 | 7.123 | 0/24 |
| fit | greg | one_fe | 28 | 7.893 | 4.850 | 3.070 | 40.852 | 0/28 |
| fit | reghdfe | akm_lowmob | 12 | 0.234 | 0.221 | 0.122 | 0.419 | 12/12 |
| fit | reghdfe | akm_lowmob_year | 8 | 0.402 | 0.320 | 0.241 | 0.890 | 7/8 |
| fit | reghdfe | akm_standard | 28 | 0.544 | 0.421 | 0.313 | 1.682 | 24/28 |
| fit | reghdfe | akm_standard_year | 24 | 0.573 | 0.531 | 0.333 | 1.181 | 20/24 |
| fit | reghdfe | one_fe | 28 | 2.127 | 1.616 | 1.270 | 5.987 | 0/28 |
| save_fe | greg | akm_lowmob | 12 | 0.306 | 0.175 | 0.144 | 1.398 | 10/12 |
| save_fe | greg | akm_lowmob_year | 8 | 0.574 | 0.415 | 0.208 | 2.382 | 6/8 |
| save_fe | greg | akm_standard | 28 | 1.790 | 1.310 | 0.869 | 6.853 | 5/28 |
| save_fe | greg | akm_standard_year | 24 | 2.581 | 2.402 | 1.472 | 5.610 | 0/24 |
| save_fe | greg | one_fe | 28 | 5.564 | 3.656 | 1.959 | 33.001 | 0/28 |
| save_fe | reghdfe | akm_lowmob | 12 | 0.175 | 0.149 | 0.090 | 0.387 | 12/12 |
| save_fe | reghdfe | akm_lowmob_year | 8 | 0.377 | 0.366 | 0.199 | 0.777 | 7/8 |
| save_fe | reghdfe | akm_standard | 28 | 0.478 | 0.360 | 0.242 | 1.612 | 24/28 |
| save_fe | reghdfe | akm_standard_year | 24 | 0.527 | 0.477 | 0.313 | 1.206 | 20/24 |
| save_fe | reghdfe | one_fe | 28 | 1.619 | 1.073 | 0.827 | 5.656 | 14/28 |

## By scale

| mode | comparator | scale | cells | geomean ratio | median | p10 | p90 | fereg wins |
|---|---|---|---|---|---|---|---|---|
| fit | greg | n10m | 42 | 3.161 | 2.708 | 1.074 | 11.826 | 2/42 |
| fit | greg | n1m | 58 | 3.386 | 3.324 | 1.040 | 10.218 | 5/58 |
| fit | reghdfe | n10m | 42 | 0.681 | 0.553 | 0.275 | 1.991 | 28/42 |
| fit | reghdfe | n1m | 58 | 0.735 | 0.608 | 0.271 | 2.182 | 35/58 |
| save_fe | greg | n10m | 42 | 2.054 | 2.073 | 0.769 | 8.781 | 6/42 |
| save_fe | greg | n1m | 58 | 1.934 | 1.746 | 0.319 | 8.570 | 15/58 |
| save_fe | reghdfe | n10m | 42 | 0.564 | 0.485 | 0.231 | 1.732 | 35/42 |
| save_fe | reghdfe | n1m | 58 | 0.626 | 0.547 | 0.223 | 2.148 | 42/58 |

## By control width

| mode | comparator | controls | cells | geomean ratio | median | p10 | p90 | fereg wins |
|---|---|---|---|---|---|---|---|---|
| fit | greg | 0 | 24 | 2.787 | 2.824 | 1.059 | 5.520 | 0/24 |
| fit | greg | 10 | 15 | 3.462 | 3.296 | 0.874 | 11.431 | 3/15 |
| fit | greg | 2 | 30 | 1.849 | 1.968 | 0.650 | 3.653 | 4/30 |
| fit | greg | 30 | 31 | 6.374 | 6.173 | 1.418 | 36.244 | 0/31 |
| fit | reghdfe | 0 | 24 | 0.616 | 0.550 | 0.250 | 1.595 | 14/24 |
| fit | reghdfe | 10 | 15 | 0.734 | 0.598 | 0.305 | 1.847 | 9/15 |
| fit | reghdfe | 2 | 30 | 0.392 | 0.363 | 0.202 | 1.157 | 26/30 |
| fit | reghdfe | 30 | 31 | 1.395 | 1.201 | 0.437 | 5.684 | 14/31 |
| save_fe | greg | 0 | 24 | 1.041 | 1.625 | 0.164 | 2.355 | 6/24 |
| save_fe | greg | 10 | 15 | 2.432 | 2.183 | 0.579 | 8.484 | 3/15 |
| save_fe | greg | 2 | 30 | 1.039 | 1.266 | 0.180 | 2.729 | 10/30 |
| save_fe | greg | 30 | 31 | 5.534 | 4.792 | 1.374 | 28.512 | 2/31 |
| save_fe | reghdfe | 0 | 24 | 0.425 | 0.477 | 0.141 | 0.922 | 24/24 |
| save_fe | reghdfe | 10 | 15 | 0.676 | 0.559 | 0.283 | 1.656 | 9/15 |
| save_fe | reghdfe | 2 | 30 | 0.319 | 0.329 | 0.154 | 0.833 | 30/30 |
| save_fe | reghdfe | 30 | 31 | 1.359 | 1.172 | 0.439 | 5.353 | 14/31 |

## By requested thread count

| mode | comparator | threads | cells | geomean ratio | median | p10 | p90 | fereg wins |
|---|---|---|---|---|---|---|---|---|
| fit | greg | 1 | 32 | 4.329 | 3.758 | 1.140 | 11.911 | 2/32 |
| fit | greg | 16 | 33 | 2.123 | 2.439 | 0.860 | 5.409 | 5/33 |
| fit | greg | 2 | 12 | 5.346 | 3.508 | 1.674 | 33.614 | 0/12 |
| fit | greg | 4 | 12 | 3.485 | 2.721 | 1.449 | 14.723 | 0/12 |
| fit | greg | 8 | 11 | 3.045 | 2.016 | 1.418 | 9.234 | 0/11 |
| fit | reghdfe | 1 | 32 | 0.714 | 0.628 | 0.191 | 1.958 | 19/32 |
| fit | reghdfe | 16 | 33 | 0.516 | 0.413 | 0.242 | 1.598 | 25/33 |
| fit | reghdfe | 2 | 12 | 1.124 | 1.037 | 0.336 | 5.330 | 6/12 |
| fit | reghdfe | 4 | 12 | 0.959 | 0.833 | 0.364 | 3.472 | 7/12 |
| fit | reghdfe | 8 | 11 | 0.814 | 0.686 | 0.344 | 2.278 | 6/11 |
| save_fe | greg | 1 | 32 | 2.354 | 2.517 | 0.237 | 8.931 | 6/32 |
| save_fe | greg | 16 | 33 | 1.111 | 1.471 | 0.179 | 3.953 | 11/33 |
| save_fe | greg | 2 | 12 | 4.086 | 2.582 | 1.133 | 26.585 | 1/12 |
| save_fe | greg | 4 | 12 | 2.652 | 2.076 | 0.949 | 12.270 | 2/12 |
| save_fe | greg | 8 | 11 | 2.270 | 1.432 | 1.291 | 7.931 | 1/11 |
| save_fe | reghdfe | 1 | 32 | 0.600 | 0.547 | 0.117 | 1.715 | 23/32 |
| save_fe | reghdfe | 16 | 33 | 0.431 | 0.364 | 0.158 | 1.140 | 29/33 |
| save_fe | reghdfe | 2 | 12 | 0.964 | 0.792 | 0.278 | 5.030 | 8/12 |
| save_fe | reghdfe | 4 | 12 | 0.809 | 0.777 | 0.296 | 3.293 | 9/12 |
| save_fe | reghdfe | 8 | 11 | 0.690 | 0.668 | 0.308 | 2.215 | 8/11 |

## Saved-FE overhead

Median incremental seconds and geometric-mean save/fit ratio. Negative increments can arise from timing noise; the final column counts cells whose median increment is no more than 50 ms.

| arm | scale | design | cells | median incremental s | geomean save/fit | <=50ms |
|---|---|---|---|---|---|---|
| fereg | n10m | akm_lowmob | 4 | 4.266 | 1.311 | 0/4 |
| fereg | n10m | akm_standard | 14 | 5.503 | 1.157 | 0/14 |
| fereg | n10m | akm_standard_year | 10 | 5.598 | 1.135 | 0/10 |
| fereg | n10m | one_fe | 14 | 1.587 | 1.147 | 1/14 |
| fereg | n1m | akm_lowmob | 8 | 0.294 | 1.151 | 1/8 |
| fereg | n1m | akm_lowmob_year | 8 | 3.167 | 1.242 | 0/8 |
| fereg | n1m | akm_standard | 14 | 0.332 | 1.121 | 0/14 |
| fereg | n1m | akm_standard_year | 14 | 0.766 | 1.101 | 0/14 |
| fereg | n1m | one_fe | 14 | 0.329 | 1.219 | 0/14 |
| greg | n10m | akm_lowmob | 4 | 102.215 | 6.630 | 0/4 |
| greg | n10m | akm_standard | 14 | 8.748 | 1.545 | 0/14 |
| greg | n10m | akm_standard_year | 10 | 8.392 | 1.631 | 0/10 |
| greg | n10m | one_fe | 14 | 1.395 | 1.521 | 0/14 |
| greg | n1m | akm_lowmob | 8 | 6.515 | 2.887 | 0/8 |
| greg | n1m | akm_lowmob_year | 8 | 19.221 | 5.140 | 0/8 |
| greg | n1m | akm_standard | 14 | 0.707 | 1.515 | 0/14 |
| greg | n1m | akm_standard_year | 14 | 0.835 | 1.436 | 0/14 |
| greg | n1m | one_fe | 14 | 0.348 | 1.848 | 0/14 |
| reghdfe | n10m | akm_lowmob | 4 | 73.782 | 1.873 | 0/4 |
| reghdfe | n10m | akm_standard | 14 | 22.544 | 1.315 | 0/14 |
| reghdfe | n10m | akm_standard_year | 10 | 26.750 | 1.256 | 0/10 |
| reghdfe | n10m | one_fe | 14 | 3.577 | 1.490 | 0/14 |
| reghdfe | n1m | akm_lowmob | 8 | 4.834 | 1.493 | 0/8 |
| reghdfe | n1m | akm_lowmob_year | 8 | 8.325 | 1.323 | 0/8 |
| reghdfe | n1m | akm_standard | 14 | 1.636 | 1.275 | 0/14 |
| reghdfe | n1m | akm_standard_year | 14 | 2.141 | 1.179 | 0/14 |
| reghdfe | n1m | one_fe | 14 | 0.702 | 1.618 | 0/14 |

## Thread scaling

Geometric-mean and median speedup relative to the same case at one requested thread, where a one-thread cell exists. Stata IC caps Stata processors at four, while fereg reports the full requested native thread count.

| arm | mode | threads | cells | geomean speedup | median speedup |
|---|---|---|---|---|---|
| fereg | fit | 1 | 32 | 1.000 | 1.000 |
| fereg | fit | 2 | 11 | 1.373 | 1.577 |
| fereg | fit | 4 | 11 | 2.424 | 2.087 |
| fereg | fit | 8 | 11 | 2.650 | 1.377 |
| fereg | fit | 16 | 32 | 2.029 | 1.665 |
| fereg | save_fe | 1 | 32 | 1.000 | 1.000 |
| fereg | save_fe | 2 | 11 | 1.381 | 1.571 |
| fereg | save_fe | 4 | 11 | 2.398 | 2.171 |
| fereg | save_fe | 8 | 11 | 2.631 | 1.366 |
| fereg | save_fe | 16 | 32 | 2.008 | 1.660 |
| greg | fit | 1 | 32 | 1.000 | 1.000 |
| greg | fit | 2 | 11 | 1.015 | 1.084 |
| greg | fit | 4 | 11 | 1.153 | 1.195 |
| greg | fit | 8 | 11 | 1.073 | 1.182 |
| greg | fit | 16 | 32 | 0.990 | 1.020 |
| greg | save_fe | 1 | 32 | 1.000 | 1.000 |
| greg | save_fe | 2 | 11 | 0.977 | 1.022 |
| greg | save_fe | 4 | 11 | 1.084 | 1.061 |
| greg | save_fe | 8 | 11 | 1.014 | 0.990 |
| greg | save_fe | 16 | 32 | 0.928 | 0.991 |
| reghdfe | fit | 1 | 32 | 1.000 | 1.000 |
| reghdfe | fit | 2 | 11 | 1.226 | 1.352 |
| reghdfe | fit | 4 | 11 | 1.865 | 1.688 |
| reghdfe | fit | 8 | 11 | 1.657 | 1.611 |
| reghdfe | fit | 16 | 32 | 1.474 | 1.525 |
| reghdfe | save_fe | 1 | 32 | 1.000 | 1.000 |
| reghdfe | save_fe | 2 | 11 | 1.212 | 1.358 |
| reghdfe | save_fe | 4 | 11 | 1.775 | 1.669 |
| reghdfe | save_fe | 8 | 11 | 1.616 | 1.482 |
| reghdfe | save_fe | 16 | 32 | 1.442 | 1.488 |

## fereg stage decomposition

The plugin's native time excludes preflight and input loading. Precommit is plugin elapsed time through native work; postcommit is command timer minus plugin elapsed time and captures result publication plus ado/Stata overhead.

| mode | scale | design | cells | median total | median native | median input+preflight | median postcommit | native share | postcommit share |
|---|---|---|---|---|---|---|---|---|---|
| fit | n10m | akm_lowmob | 4 | 14.779 | 9.984 | 1.527 | 3.225 | 66.8% | 23.3% |
| fit | n10m | akm_standard | 14 | 37.621 | 32.170 | 2.963 | 2.178 | 82.2% | 9.8% |
| fit | n10m | akm_standard_year | 10 | 46.706 | 41.679 | 1.568 | 2.144 | 91.5% | 4.9% |
| fit | n10m | one_fe | 14 | 12.993 | 8.601 | 2.192 | 1.723 | 64.0% | 18.0% |
| fit | n1m | akm_lowmob | 8 | 3.475 | 2.900 | 0.271 | 0.258 | 77.1% | 11.9% |
| fit | n1m | akm_lowmob_year | 8 | 10.035 | 9.375 | 0.330 | 0.352 | 93.5% | 3.5% |
| fit | n1m | akm_standard | 14 | 1.817 | 1.316 | 0.265 | 0.258 | 72.4% | 12.9% |
| fit | n1m | akm_standard_year | 14 | 7.737 | 7.066 | 0.326 | 0.221 | 92.1% | 3.6% |
| fit | n1m | one_fe | 14 | 1.378 | 0.923 | 0.193 | 0.316 | 63.8% | 19.6% |
| save_fe | n10m | akm_lowmob | 4 | 19.951 | 12.941 | 1.497 | 4.983 | 64.6% | 28.0% |
| save_fe | n10m | akm_standard | 14 | 41.267 | 34.728 | 2.912 | 3.533 | 80.4% | 12.1% |
| save_fe | n10m | akm_standard_year | 10 | 52.880 | 45.478 | 1.637 | 3.502 | 89.0% | 7.6% |
| save_fe | n10m | one_fe | 14 | 15.055 | 10.407 | 2.196 | 2.024 | 67.3% | 18.5% |
| save_fe | n1m | akm_lowmob | 8 | 4.002 | 3.277 | 0.269 | 0.423 | 77.8% | 13.6% |
| save_fe | n1m | akm_lowmob_year | 8 | 12.494 | 11.570 | 0.327 | 0.628 | 92.5% | 4.9% |
| save_fe | n1m | akm_standard | 14 | 2.142 | 1.550 | 0.261 | 0.366 | 72.5% | 13.7% |
| save_fe | n1m | akm_standard_year | 14 | 8.874 | 7.950 | 0.305 | 0.355 | 91.6% | 5.0% |
| save_fe | n1m | one_fe | 14 | 1.755 | 1.226 | 0.183 | 0.467 | 69.1% | 17.4% |

## fereg solver routes

| design | used solver | fit/save cells |
|---|---|---|
| akm_lowmob | cmg | 24 |
| akm_lowmob_year | cg | 4 |
| akm_lowmob_year | lsmr | 12 |
| akm_standard | cmg | 56 |
| akm_standard_year | cg | 28 |
| akm_standard_year | lsmr | 20 |
| one_fe | map | 56 |

## Largest relative losses

| mode | comparator | scale | design | controls | threads | fereg s | other s | ratio |
|---|---|---|---|---|---|---|---|---|
| fit | greg | n1m | one_fe | 30 | 1 | 212.063 | 1.327 | 159.806 |
| save_fe | greg | n1m | one_fe | 30 | 1 | 219.258 | 1.771 | 123.805 |
| fit | greg | n1m | one_fe | 30 | 2 | 74.512 | 1.189 | 62.668 |
| fit | greg | n10m | one_fe | 30 | 1 | 410.197 | 7.949 | 51.604 |
| save_fe | greg | n1m | one_fe | 30 | 2 | 78.749 | 1.675 | 47.014 |
| save_fe | greg | n10m | one_fe | 30 | 1 | 417.539 | 9.604 | 43.476 |
| fit | greg | n10m | one_fe | 30 | 2 | 254.468 | 7.021 | 36.244 |
| save_fe | greg | n10m | one_fe | 30 | 2 | 258.807 | 9.077 | 28.512 |
| fit | greg | n10m | one_fe | 30 | 8 | 306.037 | 11.637 | 26.299 |
| fit | greg | n1m | akm_standard | 30 | 1 | 238.719 | 9.195 | 25.962 |
| save_fe | greg | n1m | akm_standard | 30 | 1 | 246.178 | 10.704 | 22.999 |
| save_fe | greg | n10m | one_fe | 30 | 8 | 327.444 | 16.543 | 19.794 |
| fit | greg | n10m | one_fe | 30 | 4 | 137.824 | 7.024 | 19.622 |
| save_fe | greg | n10m | one_fe | 30 | 4 | 142.798 | 8.919 | 16.011 |
| fit | reghdfe | n1m | one_fe | 30 | 1 | 212.063 | 13.454 | 15.762 |
| fit | greg | n1m | one_fe | 30 | 4 | 10.997 | 0.704 | 15.621 |
| save_fe | reghdfe | n1m | one_fe | 30 | 1 | 219.258 | 14.420 | 15.205 |
| save_fe | greg | n1m | one_fe | 30 | 4 | 11.308 | 0.873 | 12.953 |
| fit | greg | n1m | one_fe | 10 | 16 | 4.849 | 0.393 | 12.338 |
| fit | greg | n10m | one_fe | 10 | 1 | 32.871 | 2.723 | 12.072 |
| fit | greg | n1m | akm_standard | 10 | 1 | 29.149 | 2.784 | 10.470 |
| fit | greg | n1m | akm_standard_year | 10 | 1 | 38.709 | 3.829 | 10.109 |
| fit | greg | n1m | akm_standard | 30 | 2 | 78.577 | 7.897 | 9.950 |
| fit | greg | n10m | one_fe | 30 | 16 | 109.728 | 11.406 | 9.620 |
| save_fe | greg | n1m | akm_standard | 30 | 2 | 82.759 | 8.954 | 9.243 |
| fit | greg | n1m | one_fe | 30 | 8 | 6.879 | 0.745 | 9.234 |
| fit | greg | n1m | akm_standard_year | 30 | 2 | 97.027 | 10.636 | 9.123 |
| save_fe | greg | n10m | one_fe | 10 | 1 | 35.469 | 3.953 | 8.973 |
| save_fe | greg | n1m | akm_standard_year | 30 | 2 | 102.006 | 11.859 | 8.602 |
| save_fe | greg | n1m | akm_standard | 10 | 1 | 31.777 | 3.714 | 8.556 |

## Largest relative wins

| mode | comparator | scale | design | controls | threads | fereg s | other s | ratio |
|---|---|---|---|---|---|---|---|---|
| save_fe | reghdfe | n10m | akm_lowmob | 2 | 1 | 21.759 | 247.339 | 0.088 |
| save_fe | reghdfe | n1m | akm_lowmob | 2 | 1 | 0.947 | 10.688 | 0.089 |
| save_fe | reghdfe | n10m | akm_lowmob | 0 | 1 | 18.144 | 178.480 | 0.102 |
| save_fe | reghdfe | n1m | akm_lowmob | 0 | 1 | 1.260 | 11.810 | 0.107 |
| fit | reghdfe | n10m | akm_lowmob | 2 | 1 | 17.530 | 159.902 | 0.110 |
| fit | reghdfe | n1m | akm_lowmob | 2 | 1 | 0.836 | 7.003 | 0.119 |
| save_fe | reghdfe | n10m | akm_lowmob | 0 | 16 | 14.813 | 108.539 | 0.136 |
| save_fe | greg | n10m | akm_lowmob | 0 | 16 | 14.813 | 108.408 | 0.137 |
| save_fe | greg | n1m | akm_lowmob | 0 | 16 | 1.080 | 7.573 | 0.143 |
| save_fe | reghdfe | n1m | akm_lowmob | 2 | 16 | 1.400 | 9.493 | 0.147 |
| fit | reghdfe | n10m | akm_lowmob | 0 | 1 | 12.027 | 80.094 | 0.150 |
| save_fe | reghdfe | n1m | akm_lowmob | 0 | 16 | 1.080 | 7.180 | 0.150 |
| save_fe | reghdfe | n10m | akm_lowmob | 2 | 16 | 23.069 | 149.103 | 0.155 |
| save_fe | greg | n10m | akm_lowmob | 2 | 1 | 21.759 | 139.294 | 0.156 |
| save_fe | greg | n1m | akm_lowmob | 0 | 1 | 1.260 | 7.752 | 0.163 |
| save_fe | greg | n10m | akm_lowmob | 0 | 1 | 18.144 | 109.448 | 0.166 |
| save_fe | reghdfe | n1m | akm_lowmob_year | 2 | 16 | 4.060 | 23.871 | 0.170 |
| save_fe | greg | n10m | akm_lowmob | 2 | 16 | 23.069 | 135.407 | 0.170 |
| save_fe | greg | n1m | akm_lowmob_year | 2 | 16 | 4.060 | 22.634 | 0.179 |
| save_fe | greg | n1m | akm_lowmob | 2 | 16 | 1.400 | 7.785 | 0.180 |

## Coverage limitation

This is a rigorously paired analysis of completed scopes, not a completed 116-scope matrix. The missing cells are disproportionately the most expensive 10-million-row, wide-control, year-effect/low-mobility scopes, so overall aggregates must not be presented as full-matrix population averages.

## Optimization decision from this evidence

The dominant failure pattern was control width, not coefficient recovery. At
30 controls, fereg's geometric-mean ratio was 6.374 against Greg and 1.395
against reghdfe for fit, versus 1.849 and 0.392 at two controls. The worst
completed cells spent more than 200 seconds in fereg's native finite solve.
That made the repeated column-pair scans and full-height pivoted QR the first
optimization target.

ADR 0012 implements a fused, row-major RHS/Gram kernel followed by a strongly
guarded full-rank diagonal-pivoted Cholesky solve. Weak or rank-deficient
designs and any failed independent bread certificate retain the original
column-pivoted QR. No publication tolerance was changed.

Targeted one-million-row macOS/Stata 19 paired medians after the change are
shown below. They are prototype evidence on a different machine, not an exact
before/after comparison with the SCC rows above.

| Design | Controls | Mode | fereg | Greg | reghdfe | fereg/Greg | fereg/reghdfe |
|---|---:|---|---:|---:|---:|---:|---:|
| one FE | 0 | fit | 0.376 | 0.184 | 0.371 | 2.043 | 1.013 |
| one FE | 0 | save FE | 0.472 | 0.271 | 0.737 | 1.742 | 0.640 |
| one FE | 30 | fit | 3.154 | 0.929 | 4.380 | 3.395 | 0.720 |
| one FE | 30 | save FE | 3.514 | 1.038 | 4.803 | 3.385 | 0.732 |
| worker-firm | 30 | fit | 7.127 | 6.421 | 19.791 | 1.110 | 0.360 |
| worker-firm | 30 | save FE | 7.439 | 6.863 | 20.853 | 1.084 | 0.357 |

Saved-FE increments in the two wide prototypes were 0.360 seconds (11.4
percent) for one FE and 0.312 seconds (4.4 percent) for worker-firm. The next
priority is therefore the remaining finite-kernel and Stata/plugin boundary
cost, especially in one-FE designs, rather than another recovery solver.

### Second focused implementation follow-up

The next local production round found that the raw score/intercept cache still
walked the row-major matrix twice per column. A deterministic two-pass
row-major cache, a direct contiguous-candidate moment kernel, and a rank-zero
shortcut were added without changing any publication tolerance. The original
columnwise/indexed paths remain the fallback for wider or noncontiguous cases.

On the same targeted one-million-row macOS lane, one-thread one-FE/K=30 fell
from 3.154 to 2.809 seconds overall and from 1.903 to 1.462 seconds native.
Against the earlier same-host comparator medians, that changes fereg/Greg from
3.395 to 3.024 and fereg/reghdfe from 0.720 to 0.641. Worker-firm/K=30 fell
from 7.127 to 6.816 seconds overall and from 5.825 to 5.444 seconds native;
the corresponding ratios move from 1.110 to 1.062 against Greg and from 0.360
to 0.344 against reghdfe. K=0 and 16-thread total time did not measurably
improve, so their remaining cost is not this cache.

These are focused local prototype results, not additional SCC scopes and not a
revision of the frozen aggregates above. All targeted Stata 19 preflights had
`science_fail=0`; maximum coefficient discrepancies were below `6.2e-15`, and
saved-FE reconstruction errors were below `4.5e-15`.

The final audited universal plugin
`638c6246b36b3b5754305ef754abecacb6ef39aa23bc95eb6101f7799038904a`
subsequently confirmed one-FE/K=30 at a three-run warm median of 2.608 seconds
total and 1.338 seconds native with the same green science gate. The seven-run
2.809/1.462 medians remain the conservative comparison used above.
