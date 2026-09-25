# ADR 0184: parallel current-weight slope preparation

Status: accepted for local development, 2026-09-25.

## Decision

Prepare the admitted PPML coefficient owner's weighted local QR geometry using
its existing worker observation buffers as scratch. Geometry preparation precedes
RHS solving, so those buffers have no live solution authority. Recursively divide
the ordered group descriptors and their disjoint basis/rank slices across the
available workers. Balance divisions by basis elements while retaining at least
one group per worker; empty declared groups may have zero-length basis slices.
Each leaf calls the existing rank-aware QR routine directly into the geometry
bank. This removes the serial per-group basis copy as well as distributing work.

A local group needs `members * declared_channels` scratch elements. Check every
group against the worker's admitted N-element buffer before writing geometry.
Fewer than two workers or an oversized local block retains the existing serial
preparation with its separately admitted maximum-group scratch. The test uses
division to avoid overflow. No benchmark, seed, timing or sample-size threshold
selects the route.

Prepare disjoint symbolic group-pair numeric banks concurrently on the same
installed team. Keep the original observation/channel/FMA order within each pair.
Local diagonal products and any explicit Schur construction retain their prior
implementation. A two-partition design has only one pair, so this part of the
change does not accelerate its numeric pair construction.

## Numerical authority

The local scaling, pivot selection, reorthogonalization, rank threshold and active
basis arithmetic are unchanged. Direct preparation may overwrite inactive basis
padding differently from the serial copy; no consumer may treat padding as an
active coordinate. Tests compare ranks and active entries exactly, including empty
levels, duplicate/zero slopes, changing weight scales and scratch refusal.

ADRs 0124 and 0126 still govern sample/topology/weight-generation authority. Clear
authority before fallible work and restore it only after fresh acceptance. All
parallel work joins before returning a preparation error. No residual, score,
solution, rank decision for the finite model or acceptance result is cached.
Original reconstruction, full current-weight FE and finite checks, model space,
objective, strict rank/bread, final Hessian, inference and transactional publication
remain unchanged. The recurrence, check schedule and tolerances do not change.

## Storage and scope

There is no new persistent payload, descriptor, factor, observation bank or thread
team. Existing serial scratch remains admitted and retained. Borrowed worker
buffers and disjoint mutable slices enforce ownership in the safe core; workers
touch only Rust-owned data. Recursion depth is bounded by the worker count.
This does not change the static allocation inventory or repair its source drift.

The production entry is the existing admitted PPML multipartition-slope
coefficient owner, in both fast and low memory. Canonical and ordinary work share
this preparation. Categorical PPML and public OLS do not gain a new route.
One-worker execution retains serial QR preparation. Real warmed allocation audits
cover fast and low execution at T1/T4; stable capacities alone are insufficient.
Unchanged modeled storage does not assert identical peak RSS or physical-cap
qualification.

## Evidence and remaining work

The [source-bound report](../../benchmarks/reports/development/ppml-parallel-slope-preparation-local-2026-09-25/README.md)
records current-command phase attribution, baseline/candidate/stock rotations,
fresh cases, independent weighted-projection and Poisson/Hessian oracles, memory
audits, excluded-path OLS/categorical controls and all losses. Threaded slope
cases improve 7--15% at their fresh medians without another solver bank. The
serial slope median loses 1 ms; categorical controls have no causal optimization
claim. The fresh two-slope case still takes 107 ms versus stock 84 ms.

This coherent preparation improvement is not completion of the multi-FE
0.90-times-stock goal. The compact single-RHS FE operator, repeated ordinary
Poisson work and canonical/final construction remain separate opportunities.
Rejected dense factors, paired RHS lanes and retained-product layouts stay closed.
Frozen benchmark definitions, numerical contracts, constrained-fit/CG failures
and deferred cross-platform qualification remain unchanged.
