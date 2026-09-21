# ADR 0094: reuse freshly computed initial automatic norms

**Status:** Accepted for local development
**Date:** 2026-09-21
**Decision owner:** Johannes Schmieder

## Decision

Borrowed automatic absorption already computes reference norms on its initial
input. Immediately use the existing `certify_initial` helper to copy those
norms into the initial residual-norm slots and freshly check all FE moments.
The input and weights have not changed between these operations. The former
call recomputed exactly the same norms before checking the moments.

This adds no owner, allocation or admission allowance. It changes no solver
choice, rank rule, iteration budget or tolerance. Each new input still gets
freshly computed reference norms; no earlier residual certificate is reused.
The shared borrowed automatic entry point benefits from the same identity.
Explicit solver entry points and the separate generic/slope projector retain
their previous code.

## Evidence boundary

The [source-bound report](../../benchmarks/reports/development/initial-ols-norm-local-2026-09-21/README.md)
contains a focused exact certificate/workspace fixture covering two thread
counts, two row counts, three RHS widths, zeros, extreme input values and
unit/supplied weights across extreme scales. The actual plugin ABI test passes.

Sixteen cells complete 96 successful application calls with clean isolation.
All 32 independent native inference audits and 48 candidate output audits
pass. All 48 native pairs have exact finite exports, samples, saved vectors,
status, FE metadata, certificates and solver-report fields other than times.
Paired command-time ratios range from 0.9653 to 1.0189; no broad speedup is
claimed. Forty tracked allocation peaks match and eight vary by approximately
-2.5 MB to +51.1 MB in threaded fits. The source's allocation model is unchanged,
but these observations do not establish memory neutrality. Final-block and
Linux evidence for the larger optimization milestone remain open.
