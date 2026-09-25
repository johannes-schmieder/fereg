# ADR 0191: reject the earlier-point canonical convergence witness

Status: prototype rejected, 2026-09-25. Local development, not qualification.

## Decision

Retain ADR 0177's canonical checkpoint after two accepted ordinary convergence
passes. Do not adopt the prototype that runs that checkpoint after the first
pass, counts the canonical Newton proposal as a second witness, and publishes
the preceding accepted point. Restore production to `b8f04a23`; preserve the
prototype and both test runs in the [source-bound report](../../benchmarks/reports/development/ppml-canonical-witness-closeout-local-2026-09-25/README.md).

The prototype computes the proposed point's objective and fresh full score. It
also requires the normal full-step objective rule, relative-deviance condition,
strict point/basis comparisons and final-Hessian authority. Those checks are
insufficient to justify publishing the earlier point this soon: four existing
independent accuracy tests pass on the baseline and fail on the prototype.
They cover slope truth, categorical three-FE truth, solver-role permutations
and a small dynamic-range model. The latter has relative predictor error
`3.13320702716168853e-10`, exceeding its unchanged bound.

The focused baseline run has 53 passes and the one previously documented
checkpoint-count assertion failure. The prototype has 49 passes and five
failures. No test assertion or numerical tolerance is changed. No public
performance measurement, licensed fixture or fresh holdout follows rejection.

## Consequence for further redesign

A fresh score at a proposed point does not make the preceding published point
equally accurate. Keeping that older point makes its existing Hessian products
convenient to reuse, but cannot replace the independent truth requirement.
Do not repair this experiment with a seed/size guard, a looser oracle bound or
an empirically fitted predictor-movement threshold.

This rejection does not rule out a different finalization organization that
actually accepts the more accurate point and efficiently forms and certifies
products under its exact `q*mu`. Any such change must account for the extra
current-weight work, preserve both convergence predicates and strict validation,
and demonstrate independent accuracy, rank, reconstruction, objective, score,
Hessian, refusal/resume, iteration-cap and rollback behavior. Prior-frame
solutions can be guesses; prior-frame products cannot authorize new weights.

No new algorithm or convergence policy is accepted by this closeout. The broader
multi-FE PPML performance goal remains unfinished. The bounded implementation
plan and current priorities belong only in STATUS.md.
