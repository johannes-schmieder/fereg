# Unit intercept score scan — local Mac development

Hoisting the unit intercept and weight out of the normalized-score row loop
reduces its median from 6.48 to 4.47 ms for K=10 and 6.13 to 4.19 ms for K=50.
The independent residual norm, response scaling, compensated accumulation,
row order and score tolerances remain unchanged. Other columns and weights
keep their existing path. No workspace or worker is added.

Three rotated primary pairs measure full-command medians of 165/163 ms
(baseline/candidate, K=10) and 441/438 ms (K=50). Pinned fixest medians are
143/448 ms. These small incremental gains do not meet the two-width win target.
Eleven protected controls with three repeats have maximum median ratio 1.01026
and geometric mean 0.99742; all pass. The 84 accepted calls have bit-identical
native scientific exports. Raw numerical receipts and source/artifact identities
are adjacent. The observed primary native allocation peaks are unchanged.

The focused test compares the specialized score with explicit unit-valued
frequency weights, which exercise the generic loop: four row counts, four
scales (including zero and extreme finite magnitudes), and nonfinite residual
errors. It passes, as do the actual-plugin ABI and three licensed Stata
fixtures including the independent dummy oracle and transaction check.
`validation.json` binds the native test; `science.json` holds fixture receipts.
This is bounded local arm64 development evidence, not qualification.
