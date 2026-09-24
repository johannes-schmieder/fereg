# ADR 0158: group-centered preparation with dense fast PPML products

Status: accepted for local development, 2026-09-24.

## Decision

Extend the admitted one-intercept-FE PPML group-center construction to fast
execution. Compute current-weight centers, raw/within norms and raw means with
the existing ADR 0139 implementation, independently certify its projected
response and controls, then materialize the certified differences once into
the fast session's contiguous dense bank. Strict refresh rebuilds current
geometry from immutable original columns and rewrites that same bank. Release
the temporary geometry after each construction. Low execution retains its
existing implicit representation.

This changes preparation, not the fast finite-product representation. Canonical
Gram products, QR fallback, rank/status decisions, inverse checks, concentrated
Poisson representatives and final inference retain dense access. It avoids the
repeated row-reconstruction cost that rejected the earlier implicit fast route.
The public MAP family, ordinary concentrated steps and final-product reuse rules
remain unchanged. There is no width, thread, observation-count or seed guard.

Eligibility retains the one-FE profile's publication purpose, actual categorical
intercept partition, MAP ownership and optional memory admission, with an
immutable shared prepared owner. Interaction partitions and supported generated
terms are included. Other solver families, slopes, multiple effects, synthetic
no-FE fits and ordinary OLS retain their routes. A declined geometry budget
preserves the existing certified construction.

## Acceptance and ownership

Nothing numerical survives into a new weighted construction as authority.
The builder recomputes weighted centers and reference statistics, then separately
replays original values minus proposed centers to accumulate fresh weighted
group moments. Its unchanged certificate covers response and every candidate
finite column, including subsequently dropped coordinates. It revokes validity
before fallible refresh. An unvalidated view cannot be materialized.

Materialization reads the same immutable originals and certified centers and
performs the same subtraction as the certificate. The difference from the raw
column is in the declared intercept-FE space by construction. No recurrence
residual or construction sum replaces acceptance. Finite/rank/inverse, original
Poisson score, objective, reconstruction, strict final-Hessian and exact weight-
generation conditions remain mandatory. Tolerances, sample, separation, weight
semantics, DoF and publication atomicity do not change.

Fast final products own their dense values and need not retain the prepared
owner solely for row access. Low implicit products still retain it through
inference. Main-thread SPI, safe core and the guarded transaction are unchanged.

## Memory

Admit the existing ADR 0139 group/binding/worker bound in addition to the existing
fast plan, without crediting its old projector or excess dense capacity. Fast
construction uses one retained N-by-K bank; it does not add an implicit row bank
beside another dense representation. Group state is temporary and may be rebuilt
at a strict frame. Ordinary concentrated-step allocation audits still pass;
canonical construction is not claimed allocation-free.

Fresh tracked peaks fall on four changed cases, but rise by 471,968 bytes on the
wide case. Modeled selected memory rises conservatively. No physical-cap,
static-allocation-inventory or cross-platform qualification claim follows.

## Evidence and closed alternatives

The [source-bound report](../../benchmarks/reports/development/ppml-centered-dense-one-fe-local-2026-09-24/README.md)
preserves implementation snapshots, independent projection and Poisson/Hessian
checks, current-frame/rank/ownership boundaries, fresh cases, excluded controls,
source hashes, distributions and failures. Final-source confirmation reduces
the wide median from 87 to 74 ms; comparable stock takes 65 ms. Serial gains are
not established and a small fresh serial median increases by 1 ms. The broad
0.90-times-stock objective remains unfinished.

Reject the single-pass Mata observation validator, the fused Stata expression
validator, and the isolated raw-norm handoff/direct-projection probe. Their
whole-command gains do not survive the protected cases reliably. Retain no
part of those prototypes and do not tune their guards. The new construction
reuses one existing mathematical implementation rather than adding another
one-FE solver.
