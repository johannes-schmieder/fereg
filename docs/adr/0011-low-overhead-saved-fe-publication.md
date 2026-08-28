# ADR 0011: low-overhead saved-FE publication

**Status:** Accepted
**Date:** 2026-08-27
**Decision owner:** Johannes Schmieder

## Context

Saved fixed effects are a primary fereg feature. A second iterative recovery
solve made publication much more expensive than an otherwise identical fit.
Retaining solver coefficient products removed that solve, but observation
construction and the generic slope-capable publication certificate still made
several full passes over large samples. One-FE designs were especially poorly
matched to a serial per-level certificate containing a tiny linear solve for
every group.

The optimization may not adopt reghdfe's weaker residual-change stopping rule.
Published coefficients must continue to reconstruct the absorbed target under
fereg's relative-infinity requirement and pass the independent weighted FE
moment certificate.

## Decision

1. Estimation routes retain or construct one compact FE coefficient product;
   publication never launches a second solve when that product is available.
2. Derived `xb`, absorbed targets, saved channels, and requested predictions
   execute inside the bounded action-local native thread team. The public
   `threads()` value therefore governs publication as well as estimation.
3. Saved-only calls construct the absorbed target without retaining `xb` and do
   not materialize a separate combined absorbed vector. Only requested FE
   channels cross the publication boundary.
4. Intercept-only retained products use an observation-major scatter and fixed
   deterministic row/level chunks. Reconstruction and FE moments keep the
   existing tolerances and fail closed on malformed membership, nonfinite
   arithmetic, or a failed certificate.
5. One-FE publication evaluates the reconstruction residual and certificate
   directly from the target and retained level coefficients. It avoids an
   observation-length residual allocation and the generic rank-revealing group
   solver.
6. Large one-FE single-RHS projection is eligible for the existing parallel
   group-mean kernel. Tests require bit-identical residuals, routing, iteration
   counts, and certificates at 1, 2, 4, 8, and 16 threads.
7. Slope-bearing recovery retains the generic certified path. Routes without a
   retained product retain the bounded CG/SD-LSMR/final-MAP fallback policy.
8. The expanded validation and timing contract is
   `benchmarks/manifests/ols_command_comparison_stata19_v2.json`, analyzed by
   `scripts/command_comparison_v2.py` and executed by the versioned Stata 19
   harness. It crosses control width, one/two/three FEs, and requested native
   threads through 16 on macOS and Linux/SCC.

## Consequences

- Saved-FE cost becomes memory traffic and coefficient publication rather than
  another numerical solve.
- The strict independent publication certificate remains the reason fereg may
  still spend more time than a comparator using a residual-change heuristic.
- Requested 16-thread fereg runs remain meaningful even under an eight- or
  four-core Stata license; comparator processor caps are reported separately.
- No CMG upstream API change is required for this round. The pinned CMG
  coefficient and memory interfaces already supply the retained product needed
  by fereg. Any future expansion of automatic CMG routing remains governed by
  ADR 0010's calibration and compatibility requirements.
