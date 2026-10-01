# ADR 0010: Engine module sizes are planning estimates

**Status:** Accepted
**Date:** 2026-09-30

## Context

The rebuild plan capped the engine at 25,000 production lines and assigned
module budgets totaling 24,500. Johannes instructed us to finish the rebuild,
ignore the 25,000-line ceiling and avoid a sprawling codebase.

## Decision

- The engine has no aggregate line ceiling. Its module figures are initial
  planning estimates; their sum and individual values are not admission gates.
- Keep cohesive responsibilities, shared kernels and one implementation per
  concept, including one prepared-data representation and one FE interface.
- Retain the 1,500-line file and 150-line function limits, applied to the
  engine/oracle now and every crate after M7. The separate 35,000-line total
  production and 8,000-line plugin ceilings remain in force.

## Consequences

- Required scientific behavior and the complete Stata transaction can be
  implemented beyond the initial engine estimate without a further waiver.
- Continue recording the M7 line-count report and enforcing existing size
  checks. Review growth for cohesion and duplication within the shared design.

## Alternatives rejected

- Enforcing the original engine cap contradicts Johannes's explicit waiver.
- Removing all size and architecture constraints exceeds the waiver's scope.
