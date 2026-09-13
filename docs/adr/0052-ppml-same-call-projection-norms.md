# ADR 0052: Same-call certified PPML projection norms

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-13

Fast one-FE PPML fused projection must refresh retained within-column norms
under the current weight frame. The mandatory repair at `6b7d0fe` provides a
stable matrix scan; its stale predecessor is not an admissible baseline.

Reuse the norms already computed by the same successful one-FE categorical
projection. Copy trailing control norms into caller-owned retained storage
without moving the projector's workspace vector or publishing a last-result
accessor. Every successful one-FE return, including an initial warm certificate,
has norms for its exact returned values. Multi-FE solver/retry handoffs do not
establish that workspace identity and retain the scan.

Reuse also requires the frame's solver divisor to equal MAP's maximum raw
weight. Mismatched explicit divisors retain the existing stable frame-specific
calculation. Failure or unsupported reuse leaves the norm output unchanged.
The enclosing session preserves its invalidation and strict transaction rules.
No norm is reused merely because a sample or topology generation is unchanged.

This removes one N-by-K norm scan per eligible fused refresh at the cost of an
N-weight maximum check and K copied norms. It adds no observation-sized state.
Raw reference scales/means, useful warm certificates, rank/QR policy, current-
weight final inference and independent checks remain authoritative. Fresh-input
certificate removal is a separate evaluation; ordinary OLS, low/generic/slope
projection and multi-FE joint solvers retain their implementations.

The [source-bound report](../../benchmarks/reports/development/ppml-norm-reuse-local-2026-09-13/README.md)
records 15 paired comparisons, 4.5–8.5% primary median paired command gains,
unchanged admitted native peaks, exact full b/V and protected costs. Twenty-three
focused tests and both independent Poisson/sandwich oracle arms pass. The private
audit retains zero allocation events and zero live/peak growth. These are local
development findings, not a new automatic solver region or qualification.
