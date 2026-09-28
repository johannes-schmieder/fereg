# ADR 0244: Initialize macOS worker sleep state during pool construction

Date: 2026-09-28

## Decision

Keep the pinned Rayon 1.13.0 runtime with one local production change: on macOS,
construct each worker's pthread-backed mutex and condition-variable storage when
constructing the pool's sleep state. Lock/release the mutex and notify the
condition variable before any worker can use that state. Other platforms retain
the upstream construction behavior. Sleep, wake, stealing, pool width, and
numerical execution policies do not change.

`vendor/rayon-core/FEREG_ORIGIN.json` binds the crates.io archive, upstream commit,
and every original file. Original licenses and tests are retained. The local
allocation test is separate from the production change. Cargo's path patch is
explicit and locked; no cached registry source is modified. Development source
identity collectors include vendored inputs. This creates a small dependency
maintenance obligation; remove the patch when a pinned upstream version offers
the same cold-construction guarantee.

## Reason

The source-bound [report](../../benchmarks/reports/development/ppml-private-low-scc-2026-09-28/README.md)
preserves an intermittent four-worker low-memory PPML allocation-audit failure.
An isolated instrumented build records exactly one 48-byte allocation inside the
actual sixteen-iteration audit window. Its stack is Rust `OnceBox::initialize`,
Rayon `Sleep::sleep`, a waiting join, and PPML mean construction. It is not a
finite-moment scratch allocation. The existing startup broadcast ensures workers
run, but cannot ensure every worker has slept. The changed moment scheduling
exposed an existing lazily initialized runtime object.

Rayon's public pool API does not expose initialization of private sleep state.
A timed delay or additional warm-up work cannot guarantee that every worker
sleeps. Excluding runtime events from the audit would weaken its purpose.
Eager construction fixes the lifecycle boundary directly, without either measure.
The runtime objects already belong to the pool. Their backing storage is now
materialized even for workers that would never otherwise sleep. The public PPML
embedding snapshot observes this live storage before core admission. There is
no observation-sized buffer, thread, or stack reservation added. The stronger construction boundary also applies
when OLS constructs the shared team.

## Validation boundary

A cold-state test exercises all mutexes and condition variables after constructing
pools of 1, 4, and 16 workers, with an independent thread-local allocation counter.
The same test fails on the exact upstream constructor (two first-use allocations
at one worker) and passes on the patch. It does not depend on scheduling luck.
Public PPML allocation assertions, sixteen complete iterations, capacities,
warm/cold equivalence, all mathematical certificates and failure atomicity remain
unchanged. Source-bound local/Linux validation and repeated public audit results
belong in the linked report; none of this is qualification or a timing claim for
the diagnostic build. Static allocation-inventory drift remains open.
