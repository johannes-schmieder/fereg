# PPML working-response reconstruction

The retained weighted-linear session certifies every fresh, strict and frozen
PPML subproblem. Its working response `z`, finite predictor `f`, absorbed
predictor `a` and within residual `r` satisfy `z = f + a + r` in exact arithmetic.
This check is separate from FE projection, regression score, rank and bread
certification, and from PPML's outer score, objective and final-Hessian checks.

## Construction

Canonical PPML candidates use `eta = offset + (z - r)` directly from the
session's authoritative response and certified residual. The borrowed output
exposes the existing response buffer; no additional observation-sized array
is allocated. Fresh/warm comparisons use the same construction. The separate
absorbed predictor is formed as `(z - r) - f` for callers that need the
decomposition. Ordinary frozen PPML already uses the combined representation.
Public one-shot OLS does not enter this retained-session path.

## Rounding bound

A check scaled only by `max(abs(z), 1)` can reject ordinary floating-point
rounding when `a` and `r` largely cancel. The Veneto diagnostic reproduces this
with a cancellation ratio of about 392. The initial weighted regression's
other six certificates pass, but a response-relative reconstruction error
slightly above `64 * epsilon` rejects the fit before its first PPML update.

Let `u = epsilon / 2` be the binary64 unit roundoff and
`gamma(k) = k*u / (1 - k*u)`. Define

```text
S = max(1, abs(z) + abs(f) + abs(a) + abs(r))
reconstructed = (f + a) + r
error = abs(reconstructed - z)
```

The internal reconstruction accepts `error <= gamma(8) * S`, with finite
inputs, intermediates and discrepancy required. Two subtractions form the
stored `a`; recombination and measurement take three more arithmetic
operations. Standard rounding bounds give at most
`(gamma(2) + gamma(3)) * S < gamma(5) * S` for that round trip.
`gamma(8)` conservatively covers this bound and its floating-point evaluation;
it is an operation-count allowance, not a calibrated PPML tolerance.

The implementation divides each absolute magnitude and the measured error by
`max(1, abs(z), abs(f), abs(a), abs(r))` before evaluating the inequality. The
nonnegative magnitude sum therefore cannot overflow. The unit floor covers
absolute/subnormal rounding. Nonfinite components, overflowing reconstruction
or an unrepresentable discrepancy fail closed. A failed row remains failed
when subsequent rows certify.

The original **response-relative** maximum remains the diagnostic carried to
final result validation. Publication still requires that value to be within
the unchanged PPML convergence tolerance, together with every other existing
final certificate. A componentwise allowance cannot by itself authorize a
material response-relative error in published results. Model, sample, weight,
rank, VCE, DoF, solver scheduling, line-search and score tolerances are unchanged.

## Independent checks

`tests/fixtures/ppml/generate_reconstruction.py` uses Python exact rational
arithmetic on the binary64 input values to form the correctly rounded absorbed
predictor and its exact decomposition discrepancy. The checked-in bit-pattern
oracle covers ordinary and Veneto-scale cancellation, negative responses,
combined predictors, very large/small magnitudes, subnormals and near-maximum
finite values. Deliberate component corruption must be rejected. Rust tests
also check both subtraction orders, nonfinite/overflow rejection, and failure
aggregation. Existing retained-session and PPML-driver tests cover the
surrounding numerical and transaction boundaries.
