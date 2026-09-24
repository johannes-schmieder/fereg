# ADR 0151: one streamed joint PPML product kernel

Status: accepted for local development, 2026-09-24.

## Decision

Use the established resolved-column `finite_product` kernel for every streamed
joint PPML Hessian entry, independent of the admitted team width and complete-row
binding availability. Remove the single-worker row-product specialization and
its temporary borrowing of the factor bank. Each worker forms one upper-triangle
Hessian row from the existing projected observation buffer and immutable source
columns, then the triangle is copied symmetrically as before.

This supersedes the remaining direct-row product exception in ADRs 0148–0150.
Direct row binding avoided recipe lookup, but still gathered the full model row
for each Hessian row. The fixed eight-chain column reduction avoids that gather
and its dependent entry reduction. The choice concerns product formation only:
ADR 0149's storage policy for stable norms, reconstruction and residual moments
remains. The earlier rejection of unconditional column scans for those operations
did not test this product replacement.

Dense retained products and rare observation-space curvature fallback retain
their prior algorithms. No new threshold, solver, public option or alternative
recurrence is introduced. The displaced implementation is removed rather than
retained behind a benchmark-dependent width guard.

## Scientific and memory boundaries

The current-weight normalized upper-triangle inner product is still the one
defined by ADR 0148. Newly affected direct-row serial cases use the same eight
fixed accumulator chains and ordered final reduction already used elsewhere.
Reduction order changes; independent weighted-design and Poisson/Hessian/sandwich
oracles and the registered envelopes, rather than baseline bit identity, govern
acceptance. A product's reduction tree no longer depends on admitted worker count.
This does not promise cross-thread bit equality for the complete estimator.

Every frame retains fresh weighted geometry, stable norms, FE/control products,
factorization, residual reconstruction and independent FE/finite scores. Outer
objective, strict agreement, canonical rank/bread/final Hessian and publication
remain authoritative with unchanged check scheduling. No sample, tolerance,
weight semantics, DoF, separation, rank policy or failure-atomicity change.
Ordinary Newton products still have no covariance or rank authority.

There is no new heap workspace, N-by-K matrix, cache, worker team or admission
formula. The reused helper has eight f64 stack accumulators; existing factor and
observation bank ownership is simpler. Safe-core/FFI, main-thread SPI and the
transactional publication boundary remain unchanged.

## Evidence and remaining work

The [source-bound report](../../benchmarks/reports/development/ppml-unified-products-local-2026-09-24/README.md)
records stock rotations, predeclared fresh cases, independent oracles, allocation
and transaction boundaries, excluded-path losses and isolated phase attribution.
The artifact passes 58 distinct Rust tests and seven licensed fixtures. All 210
main applications and 25 fixed-input repeat applications pass scientific checks.
Fresh wide direct-storage and disconnected cases improve 51% and 36%; the narrow
repeat ties, and excluded timing/memory losses remain visible in the report.

The broad per-case 0.90-times-stock objective remains unfinished. Small gains on
narrow models are a reason to move to their measured dominant phase, not to add
width thresholds or tune the accumulator count. Physical caps, cross-platform
qualification and separately documented failures remain open.
