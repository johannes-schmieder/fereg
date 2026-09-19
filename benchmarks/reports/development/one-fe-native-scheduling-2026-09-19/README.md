# Native preparation and independent statistics scheduling

Preparation now installs the existing bounded team from eight numeric source
columns, with the same 16,384-row threshold. Completeness, classification and
compaction use their existing parallel kernels; observation-order error
selection and serial fallbacks remain intact. IID composition uses the resolved
team and overlaps the independent residual sum-of-squares scan with the outcome
mean/TSS scans. It changes neither reduction arithmetic nor their error order.

Isolated arm64 builds start at `9daa6534` with explicit overlays in
[identities.json](identities.json). `preparation` changes only preparation;
`statistics` adds the independent-scan overlap. Separate Cargo targets and the
same release settings avoid cross-variant build contamination.

Three rotated full-command primary blocks, with native phase tracing, give:

| K | Pipeline baseline | Preparation | Preparation + statistics | Pinned fixest |
|---:|---:|---:|---:|---:|
| 10 | 216 ms | 214 ms | 204 ms | 140 ms |
| 50 | 496 ms | 500 ms | 492 ms | 445 ms |

Both cases use 983,040 observations and 16 native threads. The narrow preparation
phase drops from 24.04 to 17.34 ms in the combined candidate, and IID statistics
from 17.88 to 13.93 ms. Wide statistics drop from 17.37 to 13.47 ms. The combined
change improves the narrow command by 5.6%; the wide command remains close to
the baseline. Neither meets the owner's win over fixest target.

Eleven controls have three rotated blocks without phase tracing. Preparation
alone has maximum median ratio 1.0116 and geometric mean 0.9948. The combined
candidate has geometric mean 0.9971. One 245,760-row, K=10, one-thread cell
initially exceeds 5% (110/116 ms baseline/candidate); five new pairs give
115/110 ms, so that slowdown is not confirmed. Original and recheck results
remain separate in [measurements.json](measurements.json) and
[summary.json](summary.json).

All 133 accepted calls preserve input/sample identity and bit-identical native
scientific matrices; fixest checks retain existing tolerances. Both candidates
pass real-plugin ABI/panic checks and the three licensed scientific/transaction
fixtures. Focused native tests check preparation at 7/8/11/16 sources against
serial preparation, earliest-row classification errors, and exact statistics
bits across the parallel threshold for unit/frequency/analytic weights, very
large/small magnitudes and both intercept policies. Injected IID certificate
errors remain rejected. [Science](science.json) and [validation](validation.json)
record the evidence. This is local development evidence, not qualification.
