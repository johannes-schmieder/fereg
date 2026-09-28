# Fresh OLS slope confirmation

192 fits, 56 paired repetitions.

Command milliseconds are medians. Ratios show the median of paired ratios [minimum, maximum].
NR means at least one registered science comparison is ineligible; raw clocks do not override that exclusion.

| Case | Baseline ms | Candidate ms | C/B | C/default | C/strict | B/C peak MiB |
|---|---:|---:|---:|---:|---:|---:|
| two-dense-fast | 1349.5 | 786.5 | 0.575 [0.448, 0.768] | NR (0/8 eligible) | 1.127 [0.789, 1.430] | 149.5 / 104.7 |
| two-dense-low | 1794.5 | 729.5 | 0.417 [0.303, 0.511] | NR (0/8 eligible) | 1.069 [0.842, 1.348] | 146.8 / 107.0 |
| three-sparse-fast | 2664.0 | 2643.0 | 1.013 [0.922, 1.193] | not rerun | not rerun | 155.7 / 156.4 |
| two-dense-serial | 1134.5 | 364.5 | 0.357 [0.289, 0.485] | NR (0/8 eligible) | 0.406 [0.379, 0.556] | 110.1 / 87.8 |
| three-sparse-low | 7433.0 | 8265.5 | 1.078 [0.924, 1.213] | not rerun | not rerun | 156.0 / 154.3 |
| two-dense-wide | 1628.5 | 1322.5 | 0.804 [0.522, 0.981] | NR (0/8 eligible) | 1.167 [0.749, 1.414] | 216.2 / 122.5 |
| two-dense-larger | 4293.5 | 2298.5 | 0.541 [0.452, 0.640] | NR (0/8 eligible) | 1.237 [1.093, 1.478] | 540.1 / 255.0 |

Process RSS includes the full warm-plus-timed estimator process, including loaded data and requested outputs.
Setup/loading and extraction are outside the command clock; their process-time distributions remain in the JSON receipt.

- two-dense-fast / default: error, fitted.
- two-dense-low / default: error, fitted.
- two-dense-serial / default: error, fitted.
- two-dense-wide / default: error, fitted.
- two-dense-larger / default: error, fitted.
