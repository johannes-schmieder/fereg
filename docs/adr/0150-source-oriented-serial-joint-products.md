# ADR 0150: source-oriented serial joint PPML products

Status: accepted for local development, 2026-09-24.

## Decision

Apply ADR 0149's storage-oriented scan policy to the single-worker finite-Hessian
kernel. When complete direct numeric row bindings are available, retain the fused
row reduction over independent upper-triangle entries. When rows require recipe
evaluation, use ADR 0148's existing `finite_product` column kernel, including for
one admitted worker. Resolve each source once per product; numeric sources borrow
their contiguous slices and other sources retain fallible column visitation.

This supersedes the single-worker exception in ADRs 0148 and 0149. The number of
workers alone does not justify complete-row access when that access repeatedly
resolves the full model. No new width/row threshold or solver route is introduced.
Multiworker products, directly bound row products, retained dense products and
rare observation-space curvature fallback retain their existing organization.

## Scientific and memory boundaries

Each entry remains the normalized current-weight inner product defined by ADR
0148. Eligible serial entries now use that helper's eight fixed accumulator
chains and final ordered reduction. This changes their reduction order; bitwise
identity with the old single chain is not the acceptance requirement. Fixed-team
determinism, independent weighted-contrast and Poisson/Hessian/sandwich oracles,
and the registered numerical envelopes remain authoritative.

Every solve freshly prepares its current weights, stable norms, FE/control
products and factor. Current-weight FE and finite scores, reconstruction,
objective, strict agreement, canonical rank, bread/final Hessian and publication
retain their existing checks and scheduling. No tolerance, sample, weight, DoF,
separation or failure-atomicity change. Ordinary joint products do not acquire
rank or covariance authority, and no numerical state outlives its existing frame.

There is no new heap bank, N-by-K copy, persistent cache, worker team or memory
admission formula. The reused product helper needs eight f64 stack accumulators.
Existing factor, RHS and observation banks retain their lifetimes and ownership.
Safe Rust, main-thread SPI and transactional Stata publication remain unchanged.

## Evidence and scope

The [source-bound report](../../benchmarks/reports/development/ppml-serial-products-local-2026-09-24/README.md)
records public baseline/candidate/stock rotations, predeclared fresh confirmation,
independent numerical checks, excluded-path controls and isolated phase attribution.

Four of six fresh cases clear the 0.90-times-stock target, including the wide
three-FE multiple-slope case. The source-bound artifact passes 58 distinct Rust
tests, seven licensed fixtures and all 201 headline applications; native changes
remain within the registered envelopes. No current candidate was rejected.

This completes the measured serial recipe-access omission. The direct-row K63
control still spends 68% of its diagnostic native root in product formation.
Evaluate efficient bounded serial products across that storage next, then
accepted-frame canonical products, fast categorical organization and one-FE/
public costs. Do not tune the accumulator count or fit
small threshold guards to the remaining control losses. Physical memory caps,
cross-platform qualification and separately documented failures remain open.
