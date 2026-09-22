# ADR 0099: bind numeric columns for serial slope score-cache means

**Status:** Accepted for local development; milestone validation remains open
**Date:** 2026-09-22
**Decision owner:** Johannes Schmieder

## Decision

After successful initial cached-slope absorption with one actual worker, bind
continuous/intercept model columns once for packed inference-cache means.
Reuse the existing bounded `NumericOlsColumns` representation, with at most
64 columns and no heap owner. Its source alignment and recipe checks remain
authoritative; unsupported models use the generic evaluator. Other predictor,
RHS and residual admission guards are unchanged.

Preserve generated-column identities, absorbed-intercept exclusion, every
mean's scale and multiplication/row order, fixed reduction blocks, compensated
merge and within-cache values. The packed-cache byte limit precedes binding.
Fresh inference score/covariance certificates remain mandatory. The successful
cached-slope guard keeps explicit/shared, PPML, low and rejected-slope routes
outside this optimization; threaded and strided routes retain their old path.

## Evidence and limits

The [local report](../../benchmarks/reports/development/serial-slope-score-binding-local-2026-09-22/README.md)
binds 254 successful application calls, four focused release fixtures and two
actual plugin ABI checks. All 106 timed native pairs have exact numerical
exports, samples, requested saved arrays and non-timing solver fields, with
equal tracked allocation peaks. Bitwise fixtures cover all weight types,
extreme values, width/sample boundaries, intercept handling, reduction blocks
and allocation/nonfinite failure paths.

Two unchanged five-pair blocks show about 3–4% AW robust and 2–3% PW robust
whole-command gains, with smaller clustered gains and mixed unit-weight
results. Separate single diagnostic traces reduce the packed-cache row pass
by roughly 45%. This is a bounded local improvement; serial reference losses,
Linux confirmation and final integration remain open. Captured sources include
the separately pending graph/window drafts.
