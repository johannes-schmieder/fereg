# ADR 0115: Remove an unused initial MAP certificate

Status: accepted for local development, 2026-09-22.

## Decision

Generic one-shot MAP need not calculate a full initial QR/span certificate
whose result is never used. The existing algorithm always performs at least
one sweep and replaces that result at the first scheduled check. A positive
iteration budget always checks its final iteration, including when the check
interval exceeds the entire budget. Initially orthogonal inputs retain that
same first-check behavior; no new early exit is introduced.

Keep initial weighted RHS norms and all configuration, shape and weight checks.
Before modifying the input, explicitly validate every weighted basis entry
that the former certificate would inspect. Keep the existing empty-RHS
exception. The no-FE path retains its original certificate and zero-iteration
return. Every successful or iteration-exhausted MAP report still contains a
fresh certificate of the actual returned residual at the same scheduled step.

The automatic router's initial certificate remains: it determines pilot
contraction and is used. Pilot checks, continuation checks, routing thresholds,
raw moments, QR/rank rules, tolerances and iteration budgets are unchanged.
The changing-weight reusable MAP session is a separate implementation and is
unchanged. Generic MAP callers read their current weights and input afresh;
there is no cached OLS or PPML geometry, residual, score or Hessian.

No buffer, worker team, public interface or admission rule is added. Existing
workspace admission remains conservative and exact-budget/one-byte-short
regressions remain binding. Allocation or scientific failure still prevents
public result publication.

## Evidence and follow-up

The [local report](../../benchmarks/reports/development/map-initial-certificate-local-2026-09-22/README.md)
records a 5.6–8.3% paired median gain across eight generic slope rows, including
wide, weak, low-memory, analytic-weight robust and probability-weight clustered
fits. All 33 native pairs have identical scientific exports and tracked heap
peaks; all 81 headline calls and 44 repeat checks pass. Stock still wins all
five directly comparable timing rows. Small control losses remain visible.

Tests compare the previous MAP implementation across check/iteration budgets,
weights, RHS widths and symmetric/asymmetric sweeps. They verify initially
orthogonal input scheduling and weighted-basis-overflow rejection before
mutation. Independent explicit-design and licensed fixtures protect science.

An isolated pinned fixest build shows only 1–2 ms of FE setup on the profiled
models, followed by 37/56 ms of multi-thread iteration work or 387 ms on a wide
serial case. Its b/V/sample/DoF/RSS exports match the stock artifact exactly.
That diagnostic is excluded from headline comparisons. A geometry cache is
deferred until this removal of unused work is measured; further reuse must
carry explicit current-weight ownership and memory admission. No checkpoint
frequency or certificate accuracy is weakened to approach reference timings.
