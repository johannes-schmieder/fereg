# ADR 0073: German OLS review-candidate adjudication

**Status:** Proposed; requires Johannes's decision
**Date:** 2026-09-15
**Decision owner:** Johannes Schmieder

## Evidence

The [source-bound report](../../benchmarks/reports/development/candidate-ols-adjudication-2026-09-15/README.md)
compares the unchanged German 5% sample and saved native CG output with the
pinned reference at both default tolerance and one bounded 1e-14 tolerance.
The tighter reference still misses the existing pointwise residual envelope.
Independent cut identities and a calibrated grounded, degree-scaled projection
match native residuals on every retained row (maximum error 1.177e-10) and
identify reference error (maximum 3.819e-7). Coefficients, covariance, sample,
ordering and DoF parity pass. Original failed receipts remain failed.

## Proposed decision

For this fixed German 5% case only, allow the Mac/Linux **development review
candidate** to pass its residual-accuracy gate through the independent
conditional projection, while recording reference residual parity as failed.
Require the exact frozen candidate to preserve sample/order, coefficients,
covariance, rank/status, DoF, fresh scientific certificates and residual accuracy
on the identical input. Retain the original and tighter reference diagnostics.

This creates no general CG exception, changes no solver or publication
tolerance, and does not modify the existing LSMR-specific reference exception.
It does not supply an interval enclosure of the joint finite optimum, qualify
performance, waive any memory/platform/install gate, authorize publication, or
change the formal release contract. Any broader use requires a separate decision.

Until approved, the review-candidate residual-parity gate remains blocked.
