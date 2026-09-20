# ADR 0079: narrow one-sided Schur edge cache

**Status:** Accepted for local development
**Date:** 2026-09-20
**Decision owner:** Johannes Schmieder

## Decision

Allow the capped unit-weight OLS Schur solver to use one observation-sized
idle group-statistics bank when two banks do not fit. Cache the right-effect
operator gather, which reads the larger left-level state and has more members
per level. The other half retains its original observation loop. If both
banks fit, retain the existing two-sided cache; if neither fits, retain the
original operator. The new one-bank route first samples at most 64 evenly
spaced left levels and at most 32 members per level, requiring at least one
quarter repeated neighboring codes. This bounded allocation-free check avoids
sorting a distinct-edge graph that cannot repay setup. It does not alter
scientific admission, the two-bank route or the fallback operator.

The existing eight-worker, capped-OLS and u32 observation-count guards remain.
Codes and multiplicities keep the original bit-packed representation and
checked membership ranges. No new heap owner or workspace allowance is added.
The cache lifetime ends before the full weighted observation-space certificate
overwrites its scratch. Supplied weights, low-memory routing, forced solvers,
PPML, stopping criteria and charged fallback remain unchanged.

## Evidence boundary

The focused operator check compares zero-, one- and two-bank admission against
the independently computed observation operator, at several RHS widths and
8/16 workers. Projection checks cover both one- and two-bank widths against a
separately converged MAP solve and across-thread determinism. The [local report](../../benchmarks/reports/development/two-fe-narrow-edge-cache-local-2026-09-20/README.md)
binds these checks, three final-artifact Stata fixtures, source identity and
164 application calls. It preserves the first prototype's 5.9% degree-six
regression; the guarded final candidate's five-pair control is within 3.0%.
Three-pair dense gains are roughly 5–9%, with no native peak increase in any
of sixty paired comparisons. Shuffled narrow fitting still trails fixest;
this change alone does not close the broad milestone.
