# Fresh OLS slope confirmation

72 fits, 36 paired repetitions.

Command milliseconds are medians. Ratios show the median of paired ratios [minimum, maximum].
NR means at least one registered science comparison is ineligible; raw clocks do not override that exclusion.

| Case | Baseline ms | Candidate ms | C/B | C/default | C/strict | B/C peak MiB |
|---|---:|---:|---:|---:|---:|---:|
| three-sparse-fast | 2677.0 | 2707.5 | 1.016 [0.848, 1.257] | not rerun | not rerun | 156.8 / 157.4 |
| three-sparse-low | 7064.0 | 7990.5 | 1.136 [0.671, 1.635] | not rerun | not rerun | 154.8 / 158.8 |

Process RSS includes the full warm-plus-timed estimator process, including loaded data and requested outputs.
Setup/loading and extraction are outside the command clock; their process-time distributions remain in the JSON receipt.

## Individual-input baseline controls

A combined median must not hide a repeatable loss on one input. Loss counts use paired ratios greater than one.

| Case | Seed | C/B median [min, max] | Slower pairs / eligible |
|---|---:|---:|---:|
| three-sparse-fast | 202609279 | 1.016 [0.848, 1.257] | 6 / 9 |
| three-sparse-fast | 202609280 | 1.017 [0.926, 1.085] | 7 / 9 |
| three-sparse-low | 202609279 | 1.029 [0.925, 1.635] | 6 / 9 |
| three-sparse-low | 202609280 | 1.200 [0.671, 1.348] | 7 / 9 |

