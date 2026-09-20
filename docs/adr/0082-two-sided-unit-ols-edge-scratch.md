# ADR 0082: two-sided counted OLS edge scratch

**Status:** Accepted for local development
**Date:** 2026-09-20
**Decision owner:** Johannes Schmieder

## Decision

When group-statistics scratch fits one counted Schur gather, eligible capped
unit-weight OLS may borrow the idle left-RHS bank for the other gather if it
already fits N. Retain the existing eight-worker, u32 observation-count and
bounded repeated-edge guards. Restore original left means from immutable
observations before reconstruction. Existing two-bank group scratch remains
unchanged; the shared reusable and PPML call sites explicitly opt out of the
new left-RHS use. Weighted caching keeps its accepted behavior.

No heap owner, allowance, tolerance, stopping rule, certificate, weight
semantics or fallback policy changes. Failed capped attempts leave original
observations available to the existing charged continuation. The counted
left gather changes floating-point summation, so numerical rather than
bitwise parity is required for affected projections.

## Evidence boundary

The [local report](../../benchmarks/reports/development/two-fe-unit-both-cache-local-2026-09-20/README.md)
binds 101 application calls, 34 native pairs without an allocation-peak
increase, four independent output audits, thirteen focused Schur tests and
three licensed Stata fixtures. Connected/disconnected MAP and observation
operator checks, scratch rejection, explicit authority, strict projection,
8/16-thread determinism and forced continuation pass. Default-reference
saved-output disagreements remain explicit.

Three-pair shuffled K=3/5/6 medians improve 233/268/259 to 225/258/252 ms.
Ordered fitting is broadly neutral; shuffled K=5 still trails fixest's 248 ms.
No native cell has a median paired regression above 5%. Restoration costs
part of the operator saving; this bounded checkpoint does not complete the
broader performance milestone.
