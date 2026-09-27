# ADR 0228: Bound Schwarz scratch by unique yielding work

Date: 2026-09-27. Status: accepted for local development; paired performance
confirmation pending. No release or allocation-inventory qualification.

## Problem

The shared categorical Schwarz memory model charged `T * (1 + R)` worker
scratch banks per apply, where T is the installed team width and R bounds the
number of components capable of yielding inside their local solve. This can
veto LSMR at a larger team width even when the smaller team admits it. The
instrumented SCC baseline reproduces that route change in OLS and PPML.

## Decision and bound

Charge `T + R` banks per apply. Retain the existing conservative R bound,
`pair_level_incidence / 10002`, numeric bank sizes, descriptor allowance, four
retained top-level pooled objects, concurrent RHS applies, construction peaks,
signed-cover allowance, checked arithmetic and execution-plus-solver admission.

The bound follows from the pinned within/schwarz-precond revision
`1ac3522a251c2622dbeab451415924f9ebe117b9`:

1. `schwarz/executor.rs` traverses each subdomain once per apply. Each call to
   `WorkerBufferStack::with_buffer` belongs to one subdomain on one worker.
2. A worker needs another bank before returning its current bank only when a
   local solve yields into nested Rayon work. Restriction and prolongation are
   serial. The categorical `block_elim/solver.rs` and `csr_block.rs` parallel
   thresholds require more than 10,000 entries in at least one FE block;
   a yielding pair component therefore consumes at least 10,002 pair-level
   incidences. Different components partition these incidences within a pair.
3. For worker t with maximum nesting depth h(t), at least h(t)-1 distinct
   yielding subdomains execute on that worker. A subdomain cannot contribute
   to another worker's depth. Thus `sum(max(h(t)-1, 0)) <= R`, even when workers
   reach their maxima at different times, and `sum(h(t)) <= T + R`.
4. `schwarz/buffers.rs` first consumes a shared idle bank before allocating.
   At apply completion it gathers every worker's stack back into one pool.
   Previous idle capacity cannot accumulate on top of this bound: new banks
   are allocated only after shared idle banks are exhausted. Induction across
   repeated applies retains the same T+R bound for each top-level object.
5. Concurrent outer RHS calls may own separate top-level objects. Their apply
   peaks and up to four cached objects remain separately charged by the existing
   memory model. This decision does not bound all nested RHS work by T.

This is a tighter conservative model of the unchanged implementation, not a
reduction in actual allocated bytes. More demanding solver routes can now be
admitted and may use more actual memory than the previous fallback. Neither
the available-memory safety margin nor any certificate is relaxed. The model
is tied to the pinned executor ownership; a dependency change must review it.

## Validation and scope

[Source-bound evidence](../../benchmarks/reports/development/performance-repair-diagnostics-2026-09-27/README.md)
includes 111 existing memory tests, two new sparse-scaling/threshold/overflow
tests and 29 joint-LSMR tests (overlapping counts), plus three licensed Mac
fixtures. Independent explicit OLS and Poisson/Hessian/sandwich oracles cover
weights, robust/clustered inference, categorical effects and slopes. Binary
audits and actual-plugin ABI tests pass on Mac and Linux.

All eight SCC candidate diagnostics complete. At 28 threads, instrumented
command clocks fall from 155.619 to 32.768 seconds for sparse three-FE PPML K10,
643.931 to 60.551 seconds for K50, 12.713 to 3.924 seconds for two-FE graph OLS
and 71.817 to 15.402 seconds for sparse three-FE OLS K50. These are attribution
clocks; the separately registered alternating baseline/candidate/stock block
must establish timing distributions and scientific comparison eligibility.

The change reaches shared borrowed/owned LSMR and PPML joint/checkpoint
admission. It does not change team selection, implement thread ceilings, repair
the separate sparse-zero MAP failure, or claim that the full performance targets
are met. Existing constrained-fit issues and static allocation-inventory drift
remain open. Historical receipts remain bound to their original artifacts.
