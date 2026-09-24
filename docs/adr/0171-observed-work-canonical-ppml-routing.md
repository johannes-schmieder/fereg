# ADR 0171: observed-work admission for canonical PPML products

Status: accepted for local development, 2026-09-24. Not qualification.

## Decision

Extend the existing low two-categorical-FE canonical product engine using
measured work from its own immutable prepared model. Retain the cold admission
of ADR 0170. On later canonical projections, allow pair assembly plus component
factorization work up to the sum of:

- the existing finite-Gram allowance N*W*(W+1)/2, including the response;
- two observation passes per RHS lane of Schur operator work actually evaluated
  during the previous completed canonical projection/rank stage.

The counter includes evaluated converged lanes. It measures operations, not
public solver iterations, wall time, or benchmark identities. Count a product
only after its operator returns successfully. A saturating cumulative counter
and checked differences/products prevent overflow from authorizing a route.
Only a PPML owner exposes the count. The shared operator incurs one fixed-size
integer update per application; it adds no observation-loop branch, data-sized
allocation, or change to OLS arithmetic.

The session updates its hint only after the projection/rank stage succeeds.
A direct call with no iterative work retains the last observation. Changing
weights or responses may retain that performance hint, since prepared sample,
topology and column identities remain fixed. This never retains weighted
numerical authority. New prepared sessions start at zero; explicit basis or
prepared-alignment invalidation clears the hint. Strict rollback carries the
hint from its completed current-weight staging boundary along with the existing
rollback state. Failed speculative publication never turns it into scientific
authority.

## Memory ownership

Preserve the existing 16*N*pool projection reservation. Initially subtract the
capacity of the previous temporary projection block and admit the bounded
topology phase. After discovering components, the complete pair/factor work
and peak-memory plan must fit before reclaiming any old block.

If that complete plan needs its reservation, consume and drop the obsolete
projection block before allocating weighted geometry. Work/memory refusal
leaves the block untouched. Numerical refusal after reclamation may rebuild it
through the existing fallible projector path. Retained scratch is not modified
until the direct result has passed its independent certificate. Allocation or
I/O failure still prevents public publication.

There is no new persistent matrix, topology cache, or N-sized bank. The
counter, session hint and strict rollback hint are fixed-size u128 metadata;
the existing coexisting-owner descriptor charge uses the actual Rust type size.
All phase banks and actual capacities retain ADRs 0169–0170 admission.

## Scientific authority

Routing is a performance prediction, not a certificate. Every admitted call
still rebuilds normalized current weights, masses, maximum-mass grounding,
component matrices and guarded factors. Response and all controls share only
that call's factors. Original-input residual moments and direct residual Gram
are independently recomputed. Original samples, weights, separation, rank,
finite solve, reconstruction, score, objective, strict final Hessian, inference,
DoF and failure atomicity retain their existing requirements and tolerances.

The mathematical engine and acceptance checks of ADRs 0168–0170 are unchanged.
This changes when an established organization is used and which obsolete
temporary allocation it may replace. It does not revive the rejected prepared
matvec or residual-space extrapolation experiments.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-canonical-work-local-2026-09-24/README.md)
records the frozen candidate, independent oracles, public rotations, rejected
test assumptions, memory and excluded-path controls.
The identical-frame diagnostic establishes that direct products are materially
cheaper on the two studied work-refused designs, including setup and the full
independent certificate. The public development screen improves approximately
6–7%, primarily in strict-final solving. Fresh disconnected targets improve
5% and 9%; a narrow connected case improves about 2%. These smaller gains are
retained because they reuse the existing mathematical engine with fixed-size
metadata and bounded ownership changes. The broad speed target is unfinished.
Initial canonical work and ordinary joint iteration remain separate bottlenecks; the diagnostic speedup must not be
transferred to whole-public timings.

The pre-existing initial-full-width LSMR reporting assertion fails on both the
b782-based diagnostic snapshot (with unchanged reporting code) and the candidate.
Its failure is preserved separately; this decision does not repair or mask it.
