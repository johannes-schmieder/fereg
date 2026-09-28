# Initial portable-product experiment

24 fits, 9 paired repetitions.

Command milliseconds are medians. Ratios show the median of paired ratios [minimum, maximum].
NR means at least one registered science comparison is ineligible; raw clocks do not override that exclusion.

| Case | Baseline ms | Candidate ms | C/B | C/default | C/strict | B/C peak MiB |
|---|---:|---:|---:|---:|---:|---:|
| two-dense-fast | 562.0 | 506.0 | 0.950 [0.896, 1.013] | not rerun | 0.789 [0.602, 0.801] | 105.5 / 104.2 |
| three-sparse-low | 7019.0 | 7155.0 | 1.111 [0.708, 1.330] | not rerun | not rerun | 159.9 / 158.3 |
| two-dense-larger | 3975.0 | 1735.0 | 0.404 [0.393, 0.811] | not rerun | 0.948 [0.873, 1.044] | 239.0 / 313.2 |

Process RSS includes the full warm-plus-timed estimator process, including loaded data and requested outputs.
Setup/loading and extraction are outside the command clock; their process-time distributions remain in the JSON receipt.

## Individual-input baseline controls

A combined median must not hide a repeatable loss on one input. Loss counts use paired ratios greater than one.

| Case | Seed | C/B median [min, max] | Slower pairs / eligible |
|---|---:|---:|---:|
| two-dense-fast | 202609280 | 0.950 [0.896, 1.013] | 1 / 3 |
| three-sparse-low | 202609280 | 1.111 [0.708, 1.330] | 2 / 3 |
| two-dense-larger | 202609280 | 0.404 [0.393, 0.811] | 0 / 3 |


Job **7762063**, source snapshot `e07c7ec3…`, candidate plugin `e9d9f706…`, baseline boxed plugin `36f068f6…`. All native and strict-stock scientific comparisons pass. These are previously consumed development inputs, three rotations each, not fresh validation.

The larger case is promising, but its same-artifact baseline varies substantially from the preceding block; do not equate the 0.404 paired median with a stable 60% improvement. Candidate/stock is 0.948 with range 0.873–1.044. Small-case C/B is 0.950. The excluded low control loses 11.1% at its paired median, two of three pairs, with range 0.708–1.330; it remains a concern. No kernel acceptance follows before predeclared fresh validation and relevant control confirmation.

The next block was declared before generation: seven cases, seed 202609281, five rotations, identical T1/T4/T8 fast input bytes. It tests width, low memory, larger rows and the excluded three-FE control without modifying the candidate. PPML has passed independent scientific fixtures but has no new performance claim in this OLS block.
