# ADR 0003: Scientific contract and independent accuracy

**Status:** Accepted
**Date:** 2026-09-29

## Context

Refactoring must preserve statistical meaning, sample semantics and accuracy.
Normal-equation convergence alone is insufficient on ill-conditioned designs.

## Decision

- Consolidate behavior in docs/SPEC.md using existing contracts, accepted
  scientific ADRs and public help. Map requirements to tests before cleanup.
- Preserve samples, weights, singleton order, rank/status, DoF, covariance,
  prediction, saved-effect normalization, PPML separation and transaction rules.
- Retain PPML mean bias correction, both second-order contributions, separate
  corrected outputs and independent-unit support requirements.
- Reconstruct r = v − Dα and independently check weighted FE moments, finite
  scores and estimator/inference requirements without relaxing tolerances.
- Require pinned reference parity plus independent dense and high-precision
  weak-graph oracles. Compare identified outputs on nonunique FE designs.
- Reproduce/localize reported CG and CMG defects and retain direct regressions.

## Consequences

- Historical failures stay failures at their original source revision.
- SPEC must explicitly identify planned public changes rather than silently
  carrying a different estimator through a convenient implementation.
- A rewritten solver or a passing internal certificate alone is not acceptance.

## Alternatives rejected

- Reference-only validation can inherit reference inaccuracies.
- Weakening tolerances, omitting difficult graphs or rerouting away from a
  broken explicit backend does not establish its correctness.
