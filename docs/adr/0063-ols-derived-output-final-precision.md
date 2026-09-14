# ADR 0063: final precision for OLS residual-derived outputs

**Status:** Accepted for local development
**Date:** 2026-09-13
**Decision owner:** Johannes Schmieder

## Context

The ordinary final FE certificate can pass while individual requested residuals
or saved effects miss the registered output tolerance. Independent grounded
weighted-dummy, SVD, LSQR/LSMR and high-precision oracles establish this on the
preserved PW, weak, three-FE, low-FW and tiny-weight-cut requests. Coefficient
retention cannot stand in for output intent: low memory deliberately omits
retained FE products while still supporting requested outputs.

## Decision

Give prepared OLS an optional tighter final-residual tolerance, independent of
coefficient retention. Public requests for residual, absorbed, combined-prediction
or any saved-FE channel set it to 1e-14. Apply the minimum of this value and the
existing configured MAP/LSMR tolerance at the existing final projection in fast,
low and projection-only fits. Validate the optional tolerance with the existing
unit-interval rule. Leave initial absorption, reference scales, coefficient rank,
weight/sample/DoF policies and public tolerances unchanged.

A terminal CMG residual call consumes its session. It first certifies at the
requested final precision; if correction is necessary, it releases the hierarchy
before admitting and executing the existing certified CG correction. Report that
continuation and preserve the independent FE certificate. Earlier blocks retain
their ordinary CMG session. Generic owned projection uses ADR 0062 admission,
including an admitted CG continuation when LSMR cannot fit.

## Evidence and costs

The [source-bound adoption report](../../benchmarks/reports/development/ols-output-precision-adoption-local-2026-09-13/README.md)
binds the three-file repair against accepted `58bcf3a`. All 109 focused core
checks, 95 serial plugin tests, 48 saved dummy fits, four weighted omission cases,
universal/static/native arm64 ABI and public CG/generic cap checks pass. The
original saved slope succeeds at 33268760 usable bytes with 28906404 observed;
one byte below rejects transactionally. Its admitted CG continuation agrees with
the previous unrestricted precision candidate within 1e-14 on saved outputs.
The original tiny weighted cut has maximum residual error 3.05e-13 against the
preserved high-precision oracle, with full b/V and saved FE passing.

Nineteen paired commands include all thirteen original cases and six prescribed
follow-ups. Candidate science passes throughout. Median added costs on repeated
slope/weighted-cut/automatic-CMG requests are 28/53/18 ms. The unrestricted slope
native peak rises 28.87 to 38.54 MB; other changes and baseline inaccuracies are
retained in the report. This is a correctness adoption, not a speed improvement.
No passing accuracy comparison is inferred from an inaccurate baseline.

The previous failed precision prototypes and concurrent plugin-allocation test
remain historical evidence. This rebase passes the full serial plugin suite; it
does not claim to repair the intermittent concurrent failure. Saved-Schur and
final-target recovery optimization remain separate. Whole-action memory reporting
and other route-accounting gaps are still open. No release or cross-platform
qualification is implied.
