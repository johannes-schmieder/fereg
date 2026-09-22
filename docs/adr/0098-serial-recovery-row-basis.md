# ADR 0098: cache normalized slope values within recovery rows

**Status:** Accepted for local development; milestone validation remains open
**Date:** 2026-09-22
**Decision owner:** Johannes Schmieder

## Decision

Within ADR 0097's admitted serial geometry-cached direct recovery, compute each
row's normalized slope basis and normalized target once. A fixed four-double
stack array serves the existing bounded group Gram and cross-moment loops.
Every entry retains its multiplication order, observation order, compensated
accumulation and final correction. Scaling, rank policy, coefficient solve and
fresh residual certification are unchanged. There is no additional heap owner.

Uncached and threaded direct recovery, ordinary MAP and PPML retain the previous
moment loop. This does not widen recovery admission or change solver selection.
The bounded loop checks its channel and RHS dimensions before using the cache.

## Evidence and limits

The [local report](../../benchmarks/reports/development/serial-slope-recovery-row-cache-local-2026-09-22/README.md)
binds 163 successful applications, a plugin ABI check and five focused release
fixtures. All 71 native pairs preserve exact numerical exports, sample and
requested saved arrays, with equal tracked allocation peaks. Moment fixtures
compare exact bits across bounded widths, zero RHS, extreme scales and shuffled
groups. Existing rank-aware recovery, memory fallback and protected PPML checks
also pass.

Two frozen five-pair blocks show PW robust gains of about 1.8–2.2%, clustered
PW gains of about 0.4–1.2% and neutral to modest AW gains. Saved-effects-only
results are mixed. This is a small local refinement, not closure of the remaining
serial fixest gap or broader milestone. Captured sources include the separately
pending graph/window drafts; Linux confirmation remains open.
