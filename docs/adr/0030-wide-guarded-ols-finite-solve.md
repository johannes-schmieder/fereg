# ADR 0030: guarded finite OLS solve above 64 candidates

**Status:** Accepted for local development
**Date:** 2026-09-09
**Decision owner:** Johannes Schmieder

## Context

The local K64/K65 receipt records a large runtime discontinuity when the
finite solve changes from guarded Cholesky to QR. Johannes approved extending
the guarded fast solve beyond 64, preserving its rank, memory and accuracy
checks. This decision supersedes only the wide finite-solve restriction in
ADR 0012, clause 6. It does not change the automatic FE solver policy.

## Decision

Fast execution attempts the existing diagonal-pivoted Cholesky solve at every
nonzero candidate width. Every pivot must exceed the largest initial pivot
times the square root of the unchanged relative QR threshold. Weak or
rank-deficient designs and failed inverse certificates retain the existing
scaled column-pivoted QR fallback. Allocation and internal errors remain fatal.
Only QR may decide numerical omissions. Fresh residual, finite-score, FE,
reported-bread and reconstruction checks remain mandatory and unchanged.

Above 64 candidates, standardized moments partition disjoint Gram rows across
the bounded native team. Each cell accumulates observations in canonical order
with ordinary multiplication and addition, independent of thread count.
Contiguous candidate columns use row slices; omitted/noncontiguous columns use
indexed access. This replaces the strided compensated column-pair scans. It
requires no moment partial bank or additional observation-sized design.

Wide fast preflight adds `32*K^2 + 128*K` bytes for the four simultaneously live
square Cholesky work/inverse/bread arrays and linear metadata, in addition to
the existing cross-product allowance. Checked arithmetic and fallible
allocation apply at every width. There is no thread-count multiplier on this
increment. Low-memory QR, at-most-64 moment arithmetic, and the independent
at-most-64 raw-moment cache remain unchanged.

## Development comparisons

This intentional numerical change may alter rounding and the descriptive
factor-diagonal condition proxy. The separate wide development controller
uses the existing registered OLS coefficient, covariance and fit-statistic
envelopes, exact sample/status/DoF and solver decisions, and unchanged absolute
certificate bounds. It records numerical differences and complete raw
certificates; each arm must still repeat its own scientific output exactly.
The covariance determinant is a numerical function of the published V, and
pilot contraction is a measured ratio for the reconstructed residual. They
are recorded as numerical diagnostics, not exact policy identifiers. Solver,
preconditioner, stop reason, iteration counts, warnings and fallback decisions
remain exact; independent residual certificate bounds remain mandatory.
Independent group-elimination/SVD oracles and weighted fast/low and deficient
rank tests accompany pinned reghdfe comparisons.

This is not activation of `registered_tolerance` in F0. Its frozen manifest,
`extraction_exact` policy, closed inventory and qualification state are
unchanged. A local development receipt cannot satisfy those release gates.
Evidence and current limits belong in the immutable development receipt and
`STATUS.md` after the checks complete.
