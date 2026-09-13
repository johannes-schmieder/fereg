# ADR 0049: bounded LSMR batches and shared categorical memory accounting

**Status:** Accepted for local development
**Date:** 2026-09-13
**Decision owner:** Johannes Schmieder

## Decision

Window independent categorical LSMR RHSs only above both 64 columns and four
times the installed Rayon team. Each window contains at most the team's width
and shares one immutable weighted solver. Narrow batches use the original
upstream entry point. The threshold is a conservative performance guard, not
a numerical rank policy or a universal crossover.

Retain the full final coefficient/residual output banks, append windows in
original RHS order, and release all partial output on failure before existing
whole-batch retry. New reservations use checked dimensions and fallible capacity
requests. Preserve allocation errors as allocation errors. No recurrence,
tolerance, preconditioner fallback condition, original scale, certificate,
sample or publication policy changes.

Use the same window guard in the categorical OLS/PPML checkpoint memory model.
Unwindowed independent solves may re-enter through nested Rayon, so active
recurrence memory must not be capped by worker count alone. Include primary
dense cross-tabs, weighted compensation, transformed RHSs and output overlap,
coefficient-space recurrence/scatter state, and Schwarz worker stacks/pools.
Share the categorical Schwarz setup estimator with PPML's existing joint
estimator while retaining the latter's numerical implementation.

## Evidence and limits

The [adoption report](../../benchmarks/reports/development/lsmr-batch-adoption-local-2026-09-13/README.md)
links the preserved 72-pair investigation and a prospective 12-pair ordinary-command
A/A and A/B follow-up. Three-seed wide weak OLS/PPML peaks fall 12.9%/7.0%, with
+1.5%/+0.5% median paired command cost. All six ordinary K17 PPML A/B pairs fit
the original review envelope; the only increase is 3 ms. Earlier traced increases
of 21/36/38 ms remain unexplained and are not discarded. Adoption rests on wide
memory savings and the ordinary-command review, with no narrow speed claim.

There are 134 distinct focused tests, three independent licensed fixtures in
both arms, exact full scientific/solver comparisons, and nine source-bound
allocation probes. A discriminating constrained-memory fixture rejects a
previously admitted over-budget LSMR construction and uses certified CG.
The reviewed universal plugin is byte-identical to the measured candidate;
static and native arm64 ABI checks pass.

Public OLS selected-memory reporting still omits solver increments. Opaque
factor/stack bounds, discarded-attempt reporting and remaining setup reuse need
further work. Pre-existing weak certificate failures remain failures. This
decision does not qualify universal memory bounds, a release, other platforms,
new solver regions or dependency updates.
