# ADR 0002: One FE interface with retained solver backends

**Status:** Deferred; proposed Schur default withdrawn
**Date:** 2026-09-29

Current authority: [active ADR index](../../adr/README.md). Historical text below
is subordinate to that disposition and the approved incremental plan.

## Context

Solver choice should not duplicate data preparation, estimator policy,
inference or output publication. LSMR and CMG remain useful explicit options.

## Decision

- Share one topology/design and coefficient/residual/diagnostic interface across
  direct projection, Schur-PCG, pinned within LSMR and pinned CMG.
- Keep public solver(auto|lsmr|cmg) for both estimators. within names the
  modified-LSMR library, not another public solver family.
- The initial auto candidate uses direct one-FE projection and multi-FE
  Schur-PCG with monitored LSMR fallback. Adoption requires the specified gates.
- Automatic routing uses model structure and observed convergence, supported by
  comparative evidence. No size, thread-count, weight-kind or memory fences.
- CMG requires a valid Laplacian/SDDM formulation; SPEC must preserve and
  describe supported OLS/PPML structures, including nuisance effects.
- Honor explicit requests, fail unsupported structures clearly, and document
  bounded numerical fallback with requested/used solver and reason diagnostics.
- All backends reconstruct residuals from coefficients and share independent
  certification and estimator checks.

## Consequences

- Explicit backends remain supported even when another is faster on a cell.
- PPML must refresh or invalidate numerical state with weights/sample changes.
- Tests force supported backends and failure/fallback paths independently.

## Alternatives rejected

- Removing CMG or hiding LSMR contradicts the approved amendment.
- Separate estimator pipelines per solver recreate the legacy complexity.
