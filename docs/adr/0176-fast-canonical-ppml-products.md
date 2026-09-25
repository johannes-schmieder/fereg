# ADR 0176: certified canonical products with dense fast PPML storage

Status: accepted for local development, 2026-09-24. Not qualification.

## Decision

Extend the current-call two-categorical-FE canonical product engine to admitted
fast PPML publication fits. Retain the current engine's original-column bindings,
current-weight component factors, work admission, independent residual/Gram
certificate and same-call moments. Reconstruct certified controls once into the
fast session's existing dense retained bank. The rank owner consumes the direct
Gram without scanning that bank to build the same finite products again.

This generalizes canonical construction and storage; it adds no ordinary IRLS
solver. Ordinary LSMR/joint directions, the profiled intercept curve and final
product acceptance keep their implementations. Slopes, other FE counts, low
execution and one-shot OLS retain their routes. Optional work, geometry and memory
refusal uses the existing certified fast projection.

## Ownership and science

The builder still owns one weighted call only. It reconstructs residuals from
immutable original inputs and freshly solved FE coefficients, then checks both
original partitions at the existing tolerance. All candidate controls participate,
including those later dropped by rank. No previous weight frame grants authority.
The one-use direct Gram is consumed by the unchanged canonical rank, inverse,
finite-solve and score boundaries. Objective, sample, weight semantics, rank,
DoF, separation, reconstruction, final Hessian and publication remain unchanged.

Dense reconstruction uses the same subtraction as the existing scratch writer.
Streamed group coefficients are prepared once per output tile with one shared
helper. It never recomputes a group system independently for every member row.
Finite output and exact dimension checks remain. Revoke the retained certificate
before fallible rewriting. Failed rank solving returns the projected response
bank to its owner before propagating the error.

After a direct write the fused response anchor advances to the raw working
response belonging to that physical projected bank. Strict rollback restores
accepted fit and basis ownership; it does not roll physical current-weight
projection buffers back to a different frame. Therefore the anchor stays with
the newly projected frame. Subsequent ordinary response-only or warm refresh
must not add the old working-response delta a second time.

## Memory

Before construction, optionally admit 16*N*min(poolsize, finite_columns) bytes
beside the complete existing fast solver/retained/checkpoint plan (with the
existing positive-dimension floors). Do not credit old projector space or unused
dense capacity. The existing builder still checks topology, complete phase peak,
actual capacities and work within that allowance before mutation. The allowance
is charged conservatively throughout the fit even though numeric factors and
certificate reductions die after each call. Refusal preserves the prior budget.
There is one retained N-by-K bank, never a dense copy beside an implicit owner.

## Evidence

The [source-bound report](../../benchmarks/reports/development/ppml-fast-canonical-products-local-2026-09-24/README.md)
records source identities, independent science, paired distributions, memory and
protected controls. Fresh fast K64 and disconnected K102 medians improve by
42.3% and 48.0%; both beat comparable pinned stock by more than 10%. Narrow
and small disconnected fits still lose to stock. This is a bounded milestone,
not completion of the broad performance objective.

The weighted WLS tests cover retained,
streamed and tiled coefficients, disconnected components, extreme scales,
changed weights, exact dense/scratch residual identity, one-byte refusal,
rank deficiency, interruption after writing, retry, rollback and handoff back
to response-only projection. Existing strict/final and excluded paths remain
covered separately. No physical memory or qualification claim is made.
