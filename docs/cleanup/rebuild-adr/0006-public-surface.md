# ADR 0006: Public surface reduction with retained solver choice

**Status:** Deferred
**Date:** 2026-09-29

Current authority: [active ADR index](../../adr/README.md). Historical text below
is subordinate to that disposition and the approved incremental plan.

## Context

The rebuild reduces implementation controls while preserving estimation
capabilities, PPML mean bias correction and the requested numerical choices.

## Decision

- Keep model/factor/TS syntax, absorb interactions/slopes/saved effects, weights,
  vce(), dof(), keepsingletons, residuals(), noconstant, threads(), tolerance(),
  iterate(), verbose, timeit and documented PPML options including bias correction.
- Retain solver(auto|lsmr|cmg) on fereg and feppml with explicit support and
  requested/used/fallback diagnostics.
- Remove public solver(map|cg|it|squarem), preconditioner(), poolsize(), memory()
  and deterministic at cutover/cleanup as specified by the plan.
- Certify residuals, xbd and saved FEs at the requested tolerance; do not
  automatically re-solve at 1e-14.
- The temporary engine(legacy|next) switch is for migration and removed in M7.
- Do not add IV, new RHS interaction classes, HAC or other GLMs before M9.

## Consequences

- Help, parser tests, wires and compatibility documentation change together.
- Unsupported explicit solvers fail clearly and do not silently become auto.
- Ordinary and corrected PPML outputs retain distinct statistical meanings.

## Alternatives rejected

- Hiding all explicit solvers would remove approved LSMR/CMG capabilities.
- Keeping every legacy tuning option would retain the old implementation surface.
