# ADR 0102: tiled LSMR matrix transport

**Status:** Accepted for local development
**Date:** 2026-09-22

## Decision

Use cache-sized tiles when copying between the row-major native design and
the pinned solver's independent column RHS representation. Input workers own
disjoint groups of eight destination columns and visit 32-row tiles. Output
workers own disjoint 32-row destination tiles. Both borrowed and owned LSMR
call the same private transport helpers. Every scalar is copied bit for bit;
there is no floating-point reduction or change to solver arithmetic.

Retain the existing RHS owners, candidate allocation and lifetimes. Reserve
the outer column bank and each column fallibly before use. The borrowed path
still drops its RHS transport before allocating the output candidate, charges
actual capacities, leaves authoritative caller inputs unchanged on failure,
and freshly certifies the returned projection. This does not add a matrix
bank, change batching/windowing, remove an admission guard, cache a weighted
operator, or alter OLS/PPML routing, tolerances or final-Hessian checks.

## Evidence and limits

The [local report](../../benchmarks/reports/development/lsmr-transport-local-2026-09-22/README.md)
binds the artifact, 150 completed public-command attempts, small explicit-design
oracles, three licensed fixtures and 76 focused Rust tests. The wide dense
three-FE screen improves from 854 to 785 ms; new threaded dense, disconnected
and unbalanced controls show median paired reductions of 6.3%, 4.1% and 5.7%.
Narrow, serial, four-FE and PPML controls show little change. Sparse/disconnected
default-reference scientific discrepancies remain unranked.

No additional matrix-sized memory is requested. Measured upstream concurrent
RHS allocation peaks fluctuate, including positive paired differences; this
is not an exact peak-neutrality or memory-reduction claim. The frozen static
allocation inventory still fails on source drift and is not regenerated.
This closes the bounded transport experiment; it does not eliminate the
remaining dense three-FE or PPML losses or qualify a release.
