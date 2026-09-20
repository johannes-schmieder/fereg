# ADR 0085: OLS Schur restoration during reconstruction

**Status:** Accepted for local development
**Date:** 2026-09-20
**Decision owner:** Johannes Schmieder

## Decision

When the accepted OLS edge cache has occupied left-RHS means, restore those
means during the existing left-group reconstruction traversal. Original
outcome/regressor values, memberships and normalized weights remain immutable
until observation-space reconstruction. The group weight totals established
before PCG remain valid. Each original-value sum and right-solution sum retains
its separate row-order multiply/add operations, followed by the same division
and subtraction. Both existing banks are overwritten before private packed
codes can enter floating-point arithmetic.

The uncached reconstruction path, edge-cache guards, workspace ownership,
iteration accounting, caller authority and independent final certificate are
unchanged. No new heap bank, weighted approximation or routing policy is added.
Shared/PPML calls retain their prior restoration behavior.

## Evidence boundary

The [local report](../../benchmarks/reports/development/two-fe-fused-reconstruction-local-2026-09-20/README.md)
retains 125 successful application calls, 49 bitwise-exact native scientific
pairs including two diagnostic pairs, thirteen exact and independently audited
output pairs, fourteen focused Schur tests and three final-artifact Stata
fixtures. No native allocation peak increases. Tests compare the separate and
combined traversals bitwise for unit/supplied weights, scales 1e-100 through
1e100, RHS counts 3/5/8/52, padded strides and 1/8/16 workers; independent MAP,
weighted cuts and charged fallback checks also pass.

Diagnostic shuffled reconstruction falls 7.897 to 5.684 ms for unit K=5 and
5.679 to 4.724 ms for AW K=6. Three-pair shuffled AW IID improves 295 to 285 ms
versus default fixest's 303 ms, but total-time changes also include run noise;
shuffled AW full outputs are 1.1% slower. No cell median paired native
regression exceeds 5%. Nine default-reference output discrepancies remain
explicit. Small graph/narrow losses and the broad repeated scoreboard remain
open; this is local development evidence, not release qualification.
