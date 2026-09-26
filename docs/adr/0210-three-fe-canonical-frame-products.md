# ADR 0210: full-frame three-FE canonical products

Status: accepted for local development, 2026-09-25.

## Decision

Extend the existing canonical product handoff to connected three-categorical-FE
publication PPML frames. Prepare the current-weight centered Schur geometry once
for the response and all finite controls. Reuse the matrix construction and
independently stopping CG recurrence from ADR 0163 through one internal geometry
owner; the previous checkpoint caller uses that same implementation. Do not add
another ordinary IRLS solver or retain weighted numerical state across calls.

Eliminate the largest partition. The sum q of the other two level counts still
must satisfy q² <= 6N. A complete byte preflight must also pass. Disconnected
designs decline to the existing component-scaled checkpoint route. Numerical or
work refusal preserves the previous certified projector; allocation and other
fatal errors propagate. These are mathematical/work/storage rules, independent
of benchmark names, seeds and favorable timings.

The previous low-memory response/control tiles rebuilt equivalent geometry and
repeated reference and materialization scans. The new builder binds original
columns, computes compensated means and scaled original norms, solves all RHS,
and returns certified within norms and finite moments to the existing rank and
bread consumers. It materializes the retained dense or scratch controls once.
The finite rank authority, strict RAII transaction, physical response anchor,
ordinary joint LSMR and final q*mu frame ownership remain unchanged.

## Independent acceptance

CG diagnostics only propose coefficients. Reconstruct each checked residual as
`original - original_scale * FE_fit` using original inputs, not the recurrence.
Accumulate every current-weight group score and the finite Gram on those exact
residuals. The normalized group certificate is algebraically the existing
`abs(D' W r) / sqrt(group_mass) / max(original_norm, residual_norm)` check.
It keeps the existing tolerance, finite checks and all intercept channels.
Normalizing by a nonzero original norm changes units, not the bound; zero columns
retain the established nonzero-scale convention. Gram products avoid subtracting
two large moment matrices and are checked against independently constructed
weighted contrasts, including nested and deficient designs.

Sharing the reconstructed rows between the certificate and finite products
removes duplicate work; it does not turn the solver residual into an acceptance
certificate. Existing finite-solve, rank, original-input score, model-space,
objective, reconstruction, final-Hessian and inference acceptance remains.
Samples, public weights, separation, DoF and publication atomicity are unchanged.
No tolerance or intermediate acceptance condition is relaxed.

All numerical geometry belongs to this one call. A weight/design/frame change
requires fresh preparation. Only group coefficients live through the retained-row
write; no geometry receives cross-generation authority. Changed-weight retry,
rank refusal after physical writes, dropped strict candidates, commit and later
same-weight response-only solving have focused tests.

## Memory and scope

The full construction bound charges normalized weights, geometry and construction
scratch, topology roots, group-by-RHS coefficients, bindings, means/norms,
Gram, and fixed worker Krylov/certificate buffers. Reservations are fallible and
actual capacities consume a cumulative allowance; dropped geometry scratch does
not earn budget credit. The dead normalized-weight bank becomes the response.
No additional N-by-K bank or worker team is introduced.

Low execution fits the existing `16*N*pool` temporary allowance. It may consume
and drop an obsolete projection panel only after the complete preflight succeeds.
Eligible fast execution separately charges that allowance beside its complete
existing plan, with no credit for old projector or dense capacity. The same
shape/byte calculation is used before fast admission and in construction, so
ineligible geometry does not inflate the optional memory forecast. Refusal
retains the former affordable route.

The new product route serves three-FE PPML canonical construction. Two-FE direct
products retain their arithmetic; slopes, four-plus FE and one-shot OLS retain
their existing routes. The shared three-FE Schur kernel is refactored rather
than duplicated. No public option, SPI call, C-bridge computation or unsafe core
code is added.

## Evidence and remaining work

The [source-bound report](../../benchmarks/reports/development/ppml-three-fe-canonical-frame-local-2026-09-25/README.md)
records production artifact canonicalthree1, separate diagnostic builds, every
distribution, independent science, memory telemetry and failures. Hard factor
PPML improves 15–26% on development cases and 15–18% on fresh cases; matched low
numeric improves 31%. All fresh numeric three-FE ratios are 0.859–0.880 of stock.
Seven of ten fresh families meet 0.90 in every pair; sparse stock is unranked,
large two-FE numeric remains slower and four-slope PPML remains around parity.
This is not universal non-regression or qualification.

Forty-five distinct focused Rust tests and five licensed fixtures pass. An
unchanged report-flag assertion fails on both the candidate and a freshly rebuilt,
hash-verified accepted baseline; it remains visible and unresolved. All 93 public
B/C science comparisons pass, independent small Poisson/Hessian/sandwich checks
pass 12/12, and 12 OLS export pairs are exact. An interrupted OLS timing block is
preserved separately from the complete clean retry.

Do not tune this route on consumed seed 202609440. Move to the large-N two-FE
construction/ownership and full-command pass budget: current medians 1238/864 ms
and fresh 932/762 ms still require about 460/246 ms to reach 0.90 of stock.
Canonical construction alone cannot promise those savings. The existing dense
two-FE product organization declines at that shape; a coherent implicit
construction and cross-stage ownership redesign must earn its complete cost.
Standalone rejected Schur, incidence, representative and cache probes remain
closed.
