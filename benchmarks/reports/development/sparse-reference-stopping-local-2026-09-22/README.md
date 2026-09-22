# Sparse two-FE reference stopping diagnosis

The six remaining small sparse-graph discrepancies come from the pinned
fixest projection's stopping rules. They reproduce with current fereg, which
passes both the existing grounded-SVD oracle and a separate full-model QR
oracle. This investigation changes no fereg code, benchmark tolerance,
reference pin or historical result. The six stock-reference comparisons remain
failures; the modified reference is a diagnostic witness only.

## Scope and identities

The original cases are indices 704/722/737 and 773/791/806: 1,920 observations,
K=1/6, unit/analytic/probability weights, seed 8675309, crossed two-way
clustering and all saved outputs. Their graph has 384 left levels, 50 right
levels and 19 connected components. The FE rank is 415. Both effects are
nonnested in the clusters; the original full, exact FE DoF correction is kept.
AW/PW receive identical positive numerical weights in this diagnostic. There
is no frequency-weight comparison.

Current native artifact dtk1 is
`77b2b35764ae216267503b34a9965834bd3a6c8ef60e0f307bdf62c379f5b151`.
All 523 computational source hashes match its captured source, including the
six existing graph/window drafts. Its full identity and source overlays are
in [sources.zip](sources.zip); this is not acceptance of those drafts.

The fixest 0.14.2 archive remains
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
The original installed reference binary remains
`bece258f6c6fc3fc47accea45c7b004b0c495d46170d78fa2d6302e66de5fbf9`.
The fresh, isolated diagnostic clone has binary
`840ef97e8b39f814e72840b51a7d6225589a9e6ff38ab6689856e9063b13d0be`.
Its original source hashes, exact two-file patch, generated compiler settings,
build record and selected original/modified source files are preserved in
[diagnostics.zip](diagnostics.zip). No user library or installed plugin changed.

There are twelve ordinary worker calls (six native and six stock reference),
plus thirty reference diagnostic fits. All complete successfully. The original
worker includes its usual untimed warm-up. All times in this report's archive
are diagnostic and excluded from performance rankings.

## What fails and why

At the supported reference tolerance 4e-12, all six stock fits disagree on
saved effects and some residual/fitted values. Case 773 additionally fails two
symmetric covariance entries. Its native/reference covariance values are
3.5433499400518e-5 and 3.543365046237e-5. Coefficients remain within tolerance.

The clone records the exit reason for each projected column. With normal
stopping enabled, all 27 projections across the six fits exit at the SSR-change
check, which runs every forty iterations. Instrumentation preserves outputs,
demeaned arrays, combined effects, coefficients and covariance **byte for
byte** against the stock diagnostic export in every case. Those stock saved
outputs also exactly reproduce the ordinary reference worker.

Disabling only the SSR early exit removes the covariance failure and reduces
the output errors substantially. All columns then stop on the coefficient-step
criterion. A few near-zero outputs still fail the unchanged output envelope.
The final diagnostic lowers the internal projection tolerance to 1e-14 while
keeping SSR stopping disabled. All six fits then pass the original coefficient,
covariance and output envelopes against the independent full-model oracle.

| Weight | K | Stock maximum output error | SSR disabled, 4e-12 | SSR disabled, internal 1e-14 |
|---|---:|---:|---:|---:|
| Unit | 1 | 6.016e-5 | 6.416e-9 | 1.726e-11 |
| AW / PW | 1 | 2.550e-7 | 8.383e-9 | 2.209e-11 |
| Unit | 6 | 7.654e-5 | 6.599e-9 | 1.723e-11 |
| AW / PW | 6 | 2.282e-7 | 8.144e-9 | 2.193e-11 |

A separate control lowers the internal tolerance while retaining SSR stopping.
It passes the weighted cases, but unit-weight maximum output errors remain
2.939e-6 and 3.416e-6. Tightening the scalar tolerance alone does not remove the
unit-weight discrepancy.

The internal 1e-14 setting deliberately bypasses the public tolerance floor
inside the diagnostic process. It is **not a supported stock fixest invocation**
and must not replace the frozen comparator or its timings. This shows the
cause of the errors in these cases, not a general fixest correction.

Saved-effect recovery is consistent with the fitted contribution: reconstruction
errors are below 7e-16. In the pinned implementation, two categorical effects
use direct `cpp_get_fe_gnl` recovery; the `fixef.tol` recovery argument does not
trigger another iterative solve on this path. The errors therefore originate
in the fit's projection, rather than the normalization used by the comparison
harness. The combined-design condition numbers are about 1,409–1,863, so a
small coefficient step or objective change need not bound every saved effect
to the comparison's absolute tolerance.

## Independent checks and preservation

The original grounded dummy SVD oracle passes all six current native fits.
A separately formed joint design `[X, D]` is solved by rank-revealing QR,
without first absorbing the regressors. It independently forms the cluster
sandwich from the joint bread and scores, including the same exact DoF,
minimum-cluster correction and PSD handling. Native coefficients, covariance
and all outputs pass. The largest native/QR output discrepancy is 2.621e-13.
The stage audit also compares exported reference projections to a grounded
SVD projection. No sample, weight, rank or tolerance change is used.

[adjudication.json](adjudication.json) contains per-case errors, mismatch
counts, exit reasons, projection errors, fresh weighted group moments, input
bindings and native comparisons. [evidence.zip](evidence.zip) preserves all
ordinary worker records, checks and output arrays. The diagnostic archive
includes numerical inputs, all diagnostic arrays, scripts and failed attempts.
An initial preparation mistake expected `Makevars` before configuration; no
fit ran. The first verifier used the wrong working directory for Git
pathspecs. Both failures are retained; no prior record was rewritten.

`python3 verify.py` verifies 560 archive entries and reconstruction of all 523
native source files, together with artifact bindings and all six outcomes.
The accompanying privacy scan passes. This bounded diagnosis explains the six
reference discrepancies. It does not reclassify the original 766 reference
failures, qualify a release, establish a speed result, or finish the wider
optimization milestone.
