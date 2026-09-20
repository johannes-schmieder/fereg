# One-FE OLS: repeated local Mac win over pinned fixest

The owner-approved endpoint is met for both primary cases: 983,040 observations,
ten or fifty numeric regressors, one ordinary intercept fixed effect, unit
weights, IID inference and 16 native threads on the M2 Ultra. Two preselected,
independent five-pair blocks pass the stricter median-paired-ratio test. Fereg
wins all twenty pairs.

| Block | Regressors | fereg median | fixest median | Median paired fereg/fixest | Paired reduction |
|---|---:|---:|---:|---:|---:|
| A | 10 | 131 ms | 139 ms | 0.935252 | 6.5% |
| B | 10 | 129 ms | 142 ms | 0.908451 | 9.2% |
| A | 50 | 393 ms | 465 ms | 0.849462 | 15.1% |
| B | 50 | 395 ms | 456 ms | 0.857456 | 14.3% |

Ratios are medians of same-repetition ratios, not ratios of the displayed
command medians. Each block must have ratio <=0.95 at each width; each width
must win at least nine of ten pairs. All eleven protected size/thread/group-layout
cells pass: maximum median paired candidate/predecessor ratio 1.015915,
geometric mean 0.988444. No control needs the preselected five-pair recheck.
The candidate and plan were fixed before these final blocks. See
[acceptance-plan.json](acceptance-plan.json) and [acceptance.json](acceptance.json).
Earlier unsuccessful final blocks remain unchanged in the input-completion and
residual-start reports; they are not replaced or included in this final gate.

## The final implementation change

The single-column unit-weight norm keeps the original scaled sum-of-squares
recurrence and observation order. It removes dynamic weight normalization and
the generic column loop, retaining the scale and square sum in scalar locals.
Unit normalization is exactly one. Shapes, finite-value errors, the final norm
check and the general weighted/multiple-column path remain intact. It introduces
no heap allocation and no new owner. Successful norm, scale and square-sum bits
match the explicit-unit-weight implementation, including extreme magnitudes.

The first five-pair screen compares the committed residual-start baseline,
parallel finite scans, finite scans plus the unit norm, and fixest. Narrow
medians are 134/136/131/143 ms. A fresh five-pair comparison isolates the norm:
baseline/combined/norm-only/fixest medians are 132/132/131/142 ms (K=10) and
399/392/389/459 ms (K=50). Narrow completion falls from 11.05 to 8.44 ms in
that confirmation. The parallel finite scans do not give a reliable incremental
benefit and are removed. Their source patch and measurements are preserved.

A separate scalar-kernel experiment computed independent divisions in groups
of eight while retaining accumulation order. It matched the scalar result but
did not improve its roughly 0.85 ms steady-state timing, so it was not adopted.
The small standalone source and five rotated timing pairs are retained here;
these are diagnostic kernel timings, not full-command benchmark calls.

## Scientific and source evidence

The archive contains 146 accepted development calls plus forty final calls.
All 126 native development calls have byte-identical coefficient, covariance,
status, DoF, certificate and fit CSVs against their same-repetition predecessor.
Fixest comparisons pass the frozen coefficient, covariance, sample and RSS
checks. Four focused native test executions pass: finite-validation error
boundaries, 49 unit-norm configurations, and two 45-configuration residual
rejection/complete-fit comparisons. The initial finite-scan fixture had a wrong
internal configuration argument; its corrected passing rerun and the earlier
compile failure are disclosed in validation.json.

All three candidate ABI checks and all nine licensed fixture runs pass,
including independent explicit-dummy and publication-transaction oracles.
The final norm-only artifact has its own three passing fixtures. Reported
native peak allocation is unchanged in every candidate/control cell, and the
source adds no heap owner. The previously documented opaque runtime coverage
gap remains; this is not a new memory qualification.

The final plugin SHA-256 is
`0ac6a0449cff224f02d6d7b7b7696b79fe6c6d182d89e3b406b4ea26a761e9fa`.
Its identity binds base commit `78672b85` plus the explicit borrowed_map.rs
overlay; all 509 source-manifest entries match the final working source.
The reference remains fixest 0.14.2 archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Input and sample hashes, artifact identities, return codes, terminal acceptance,
scientific comparisons and output hashes are retained in the JSON receipts.

This completes the bounded local Mac development target. It follows the
committed input accessor, owned loading/RHS pipeline, sample and topology reuse,
and independent preparation/inference/certification scheduling improvements.
The original local profile measured 223 ms and 593 ms for these fereg cases;
those historical measurements are separate from the final paired gate above.
Timing includes the complete estimation command and Stata-to-native loading;
dataset import and audit exports are outside the timer. No installed package
replacement, push, remote run or release qualification occurred.
