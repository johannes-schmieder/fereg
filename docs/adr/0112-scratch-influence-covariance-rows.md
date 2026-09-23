# ADR 0112: Contiguous compensated influence covariance rows

Status: accepted for local development, 2026-09-22.

## Decision

Reuse ADR 0110's compensated scaled-row update for the upper triangle of
scratch-backed production robust covariance. Move the existing helper into
the shared sandwich module and retain the internal OLS re-export used by the
score-Gram and independent covariance certificates. Do not duplicate the
arithmetic kernel or change those existing callers.

Preserve the serial reducer's order: read a reported score row, compute each
bread-times-score coordinate with scalar compensated accumulation, apply the
estimator's complete multiplier, then accumulate the influence outer product.
In particular, do not replace it with a raw score meat and a later sandwich
product. HC1 placement, weight roots, multiplication operands, per-coordinate
observation order and triangular mirroring stay unchanged.

The helper validates slice lengths before mutation and remembers every
nonfinite product, sum or correction in a private row before returning. As in
ADR 0110, more private coordinates may be touched before an invalid-data return;
the incomplete covariance cannot leave the reducer or reach publication.
The independent score-Gram and covariance certificates remain separate fresh
computations. Rank, DoF, weights, samples, solver selection and failure atomicity
are unchanged.

The reducer retains its four existing fallible workspaces, with no added
allocation, row cache, worker team or new reduction tree. The change reaches
OLS robust covariance when score storage uses the scratch reducer. Dense,
IID, clustered and PPML inference do not gain a new execution route or cache.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/low-covariance-local-2026-09-22/README.md)
records a wide outer-product phase reduction from 165 to 71 ms; compensated
bread multiplication remains about 187 ms. Uninstrumented wide command medians
improve 1046→945 ms, with fresh K40 confirmation at 1305→1155 ms and unbalanced
factor confirmation at 523→480 ms. Narrow and three-FE effects are small or flat.

All 58 native science pairs and three saved-output binary comparisons are
identical. Thirty-one focused Rust tests, four optimized serial tests, five
independent dummy-design pilots, actual-plugin ABI containment and three
licensed OLS oracle/transaction fixtures pass. Widths 0–129, extreme scales,
nonfinite bread/product/sum, malformed row slices and allocation failures are
covered. There are no new buffers or memory-admission changes.

The original two-FE +31 ms outlier and fast-control losses remain reported.
A predeclared five-pair confirmation on identical two-FE data does not reproduce
the large loss; a fresh fast-control confirmation has unchanged median. No
case-specific guard or further tuning was added. Stock still wins eleven of
twelve comparable rows. The disconnected three-FE covariance comparison has
different DoF and remains unranked. Static allocation drift, constrained-fit
failures and platform deferrals remain open; this is not qualification.
