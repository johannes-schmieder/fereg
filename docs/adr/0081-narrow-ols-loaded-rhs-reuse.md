# ADR 0081: narrow OLS loaded-RHS reuse

**Status:** Accepted for local development
**Date:** 2026-09-20
**Decision owner:** Johannes Schmieder

## Decision

At least eight workers may use existing numeric transport/RHS ownership for
3–64 loaded columns and at least three compiled RHS columns, retaining the
16,384-row, ordinary-intercept FE, full-candidate, numeric-recipe and memory
admission guards. Smaller teams retain their previous width thresholds.
The existing unit raw-moment overlap may likewise admit 3–64 RHS columns;
supplied weights retain their prepared weighted-moment scan.

The consumer uses a separate narrow binding entry point. At least eight actual
workers and 16,384 retained rows may bind two or more generated model columns
for loaded-RHS matching. Every response, finite-column and constant bit must
match the final prepared design before the original allocation is transferred.
Sample/recipe/value mismatches discard both speculative rows and moments.
Ordinary RHS construction, predictor and residual-update bindings keep their
previous width guards. Deferred errors, SPI thread ownership, fallback,
weight semantics and all scientific certificates remain unchanged.

This activates existing owners earlier in some narrow requests. Capacity is
charged during input and preparation and transferred into the fit allowance;
no admission bound is enlarged. Two K=1 saved/full-output checks increase the
native peak by approximately 4 MB (2.8–3.3%), within their unchanged early
estimates. A robust K=1 case adds 72 bytes. These measured lifetime costs are
accepted explicitly rather than described as memory-neutral.

## Evidence boundary

The [local report](../../benchmarks/reports/development/two-fe-narrow-input-local-2026-09-20/README.md)
retains an initial 107-call prototype whose consumer guard still rejected
narrow reuse, and the corrected 226-call screen. All 114 native pairs preserve
the checked scientific files exactly. Seven independent output audits and
three final-artifact Stata fixtures pass. Focused tests cover allocation
identity, threshold/worker guards, negative zero, extreme finite magnitudes,
stale and malformed rows, deferred errors, interrupted transport, and 108
AW/FW/PW sample/domain/loading cases. The initial malformed test recipe is
preserved with its correction; it failed before loading.

Three-pair shuffled unit K=5 improves 285 to 273 ms; K=6 improves 269 to 258 ms.
One-FE K=1/5 improves 78/114 to 67/100 ms. Weighted shuffled K=6 IID remains
338 ms versus default fixest's 300 ms. Several narrow graph cells still lose;
this change does not close the broader two-way milestone.
