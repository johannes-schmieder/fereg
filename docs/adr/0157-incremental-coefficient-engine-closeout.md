# ADR 0157: close the categorical incremental coefficient-engine experiment

Status: experimental replacement rejected; development decision, 2026-09-24.

## Decision

Retain the production categorical incremental LSMR route. Do not adopt the
generic coefficient wrapper, the borrowed coefficient owner, its row/column
access variants, or its reconstruction normalization hoist from this experiment.
Do not tune width thresholds to retain favorable cells. No production numerical
or routing change follows from this ADR.

The [source-bound report](../../benchmarks/reports/development/ppml-incremental-coefficient-closeout-local-2026-09-24/README.md)
preserves the four prototypes, independent checks, profiles and public timings.
The final version reduces a narrow two-FE median from 54 to 44 ms, but stock
fixest takes 35 ms. Dense and unbalanced wider cases increase from 77 to 79 and
70 to 72 ms; every unbalanced pair loses. Smaller pilots also lose. This is not
a broadly useful replacement for the accepted route.

## What was tested

The candidate solves Newton increments in the joint finite/FE coefficient
system. It leaves canonical initial rank, response history, strict checkpoints
and final inference with their existing independent owner. The borrowed
successor removes a copied FE topology and active N-by-K control bank, prepares
numeric geometry from current weights, and borrows original controls. Current
weights remain mandatory for every direction and its original-input acceptance.

On a categorical intercept-only partition, one normalized indicator spans each
group's local model space. A fresh independently accumulated group score and
basis norm therefore supply its complete rank-one orthogonality check. The
successor uses that fact to avoid a second generic FE certificate around the
candidate, while retaining fresh original finite/FE scores and canonical final
checks. This is a tested candidate-specific justification, not permission to
remove slope span checks or change production checking policy. Tests separately
compare its residual with the full FE-space certificate.

Borrowing improves ownership but can worsen access cost: a diagnostic wide case
increases reconstruction from 3.190 to 9.079 ms. Column access and a guarded
normalization hoist do not establish a reliable public improvement across the
selected cases. Modeled memory falls substantially; tracked peak falls less,
and increases on weak/small cases. Neither alone justifies automatic adoption.

Additional integration work would still be necessary for numerical fallback,
budget refusal that preserves an affordable LSMR route, and zero-active-control
policy. These remain prototype limitations, not new production behavior.

## Separate shortcut rejected before implementation

A proposed strict zero-Newton-correction shortcut was checked on the accepted
baseline states using original controls and FE groups. All four families fail
the unchanged strict 1e-12 bound: maximum normalized moments range from
3.202e-12 to 2.081e-11. Do not add a predictably failing certificate or relax its
tolerance. This diagnostic is not a certificate for any published result.

## Consequences

The frozen inner coefficient-kernel advantages in ADR 0155 do not transfer
automatically to the public incremental boundary, even after ownership and
checking changes. Keep the accepted slope coefficient engine and categorical
LSMR wins. The broad per-case 0.90-times-stock objective remains open.

Current-source one-FE evidence now supports a materially different next target:
whole-fit lifecycle organization. Concentrated Newton arithmetic is already
small; canonical preparation/checking, objective evaluation, loading and output
work are substantial. Preserve canonical rank, fresh current-frame certificates,
final Hessian and publication independence while redesigning their data flow.
Two larger one-FE stock-mean discrepancies remain unranked pending adjudication.
