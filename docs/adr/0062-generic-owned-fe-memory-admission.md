# ADR 0062: joint admission for generic owned fixed-effect absorption

**Status:** Accepted for local development
**Date:** 2026-09-13
**Decision owner:** Johannes Schmieder

## Context

The generic intercept/slope dispatcher received a joint execution/solver budget
but did not consult it. Its prepared adapter also copied codes, slopes and
memberships and materialized unit weights without charging that overlap.
Four closed-form accepted-source regressions reproduced successful projections
at a one-byte workspace limit. A separate saved-output precision candidate made
this gap material by selecting generic LSMR for a final slope projection.

## Decision

Admit owned-design construction before its observation-scale copies. Add retained
design and materialized unit-weight storage to the caller's execution reserve.
Zero-control prepared fits additionally charge the normalized weight frame,
retained within outcome and final residual: their base has no resident-X or
parallel-RHS allowance for those overlapping observation vectors.

The unified generic dispatcher checks the actual MAP, CG, Irons–Tuck, SQUAREM or
LSMR workspace before execution. Auto first admits only its MAP pilot. When a
required LSMR attempt cannot fit, the existing CG continuation remains available
if separately admitted. Its warning records the rejected attempt. Explicit LSMR
charges its retained original matrix and checks MAP again before an uncertified
result retries that route. An absent budget skips planning.

Reuse the existing LSMR recurrence, batch-window, scatter and Schwarz model.
Generic planning counts coefficient channels, slope whitening state and signed
Schur cover storage, and includes certificate QR and transport overlap. The
intercept-only model retains its exact previous values. Generic projection
workspace and explicit fallback-copy reservations are fallible. Numerical
arithmetic, scales, stopping, rank, weights, covariance, samples and publication
remain unchanged. Workers do not access SPI.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/generic-owned-fe-admission-local-2026-09-13/README.md)
records the red/green regressions, exact/one-byte-short workspace tests, admitted
MAP and CG continuations, 134 focused core checks, 95 serial plugin tests, 48
saved dummy fits, four weighted omission cases, and universal/native ABI checks.
A public weighted slope fixture agrees with independent dummy residuals and
preserves previous estimates/sample/order/output absence on allocation failure.
Its exact cap admits 806,328 usable bytes with 730,668 observed native bytes.
The earlier 708,024-byte model failed that peak check; the allocator diagnosis
and superseded candidate remain archived.

Ten paired commands preserve exact scientific exports and OLS non-time solver
fields. None crosses the prospective 5% and 10 ms cost threshold. Original
slope auto/LSMR/CG differences are +2/+3/−1 ms. Native peaks are exact in nine
pairs; weak OLS rises 530,456 bytes for an untraced reason. No speedup or general
memory reduction is claimed. Existing inaccurate saved-output baseline cases
remain pending the separate precision repair.

This closes the unified generic owned-admission bypass, not specialized slope
MAP, saved-FE recovery or public whole-action reporting. Existing opaque upstream
factor-storage allowances are not a universal memory proof. The saved-output
precision and Schur/recovery prototypes remain separate, unadopted changes.
No release, cross-platform or cluster qualification is implied.
