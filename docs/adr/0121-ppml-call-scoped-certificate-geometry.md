# ADR 0121: PPML certificate geometry scoped to one absorption call

Status: accepted for local development, 2026-09-23.

## Decision

For a publication PPML fit using the resolved MAP engine, multiple effects and
at least one slope, optionally retain certificate basis capacity in the generic
reusable linear owner. The first certificate of every absorption invocation
builds the existing normalized, pivoted, reorthogonalized weighted QR basis.
Subsequent certificates in that invocation read exactly those basis values and
ranks. Arithmetic order, rank threshold, residual norms, raw moments, span
projections, check schedule and every final certificate remain unchanged.

Validity is a local variable on the absorption stack, initially false. It is
not a property of the retained owner. Every new call therefore rebuilds the
weighted geometry, including response-only and low-memory block calls with
apparently identical weights. Early success, failure and unwind cannot give
the next call authority to reuse old numerical state. Sample/topology changes
still require a new session. This generalizes the geometry mechanism without
extending a one-shot OLS cache across PPML weight generations.

The optional payload bank is `8*N*sum(channels) + sizeof(usize)*sum(levels)`
bytes. Existing maximum-group QR scratch remains available. Reserve all bank
capacity before linear execution, after the existing parallel projection bank;
an unavailable budget or allocation leaves the uncached certificate path.
Check actual capacities against the reservation. No buffer grows during IRLS.
The fixed owner descriptors and existing runtime headroom remain subject to
the documented incomplete whole-action allocation proof.

A separate memory-plan field charges the bank throughout preparation, weight
refresh, ordinary/strict linear work and line search. Release it with the
linear session before inference/recovery/publication. Fresh telemetry must
re-admit it, and solver slack excludes it. Both basis and rank capacities enter
the session capacity fingerprint. Mu exploration, non-MAP engines, one-effect
and categorical specializations retain their execution; there is no fitted
sample-size, thread-count, case-name or seed guard.

## Validation and boundary

The [local report](../../benchmarks/reports/development/ppml-certificate-geometry-local-2026-09-23/README.md)
binds the implementation, independent science, alternating timings, memory
tradeoff and visible failures. ADR 0120's extrapolation probe remains closed;
this decision adds no recurrence or changes to PPML rank, Hessian, separation,
score, objective, inference or transaction policy.

Five successful slope cases gain 11.3–20.4% at the paired median, including
fresh wider T8, serial and low-memory controls. Added tracked peak is
1.0–2.5 MB. All 26 successful native pairs have exact scientific exports and
non-clock solver fields; all 20 mean pairs and 44 repeat checks are exact.
Two independent Poisson/Hessian/rank pilots, 64 distinct Rust tests and three
licensed fixtures pass. A small excluded three-FE OLS control loses 2.3%
at the paired median while both medians remain 43 ms.

The disconnected three-FE slope control fails on baseline and candidate with
MAP iteration exhaustion. Stock returns a certified public state after about
86 seconds. Failed native elapsed times are not ranked. The remaining timing
repetitions were stopped, with the interrupted attempt and missing arms
recorded. A separate error diagnostic confirms absence of partial results.
This pre-existing convergence gap becomes the next bounded solver investigation;
it is not repaired by caching. Close geometry tuning without fitted guards.
