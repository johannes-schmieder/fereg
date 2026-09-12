# ADR 0038: PPML numeric selection and adjustment ownership

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-12

Issue #27 requests broader PPML ownership after generation-zero memory admission.
The production owner retained the loaded original-axis registry for separation
rebuilds but first constructed, compared and discarded a compact numeric copy of
every outcome/model source. Its separately loaded adjustment also remained live
beside the canonical compact offset throughout fitting.

Production now runs the same authoritative row visitor before admission for scalar
classification and after admission to construct only the retained-row map, drop
ledger and canonical weight reports. Original/candidate/active counts must agree
exactly. The raw source owner remains the source of all generation rebuilds; raw
pweight subsets are canonicalized under each generation's physical N as before.
There is no rebasing of original-row identifiers or numerical-session change.

The admitted owner consumes offset/exposure storage through stable compaction.
It may reuse capacity only within candidate N; otherwise it reserves a smaller
compact vector fallibly. Exposure is logged once with the existing helper.
The materializing numeric API remains an independent test oracle for selection,
canonical reports and exact original-row error precedence. Production no longer
needs to compare copies of the source data with the source from which it just
constructed them.

ADR 0020's allocation-free preflight, frozen admission, generation identity,
current-weight and publication contracts remain intact. Existing conservative
memory estimates, all automatic solver regions, VCE and scientific tolerances are
unchanged. PPML preparation remains serial; no worker calls SPI. This refines the
PPML preparation description in ADR 0036 without changing its OLS decision.

The [local report](../../benchmarks/reports/development/ppml-owned-preparation-local-2026-09-12/README.md)
binds the isolated patch/artifacts, exact scientific comparisons, independent
oracles, command/phase times and allocation/RSS observations. Public complete-command
paired medians improve 3.3–6.4% on the selected primary cases. Overall native peak
is effectively unchanged without an adjustment and slightly lower with one;
removing a large transient copy does not imply the same reduction in the fit peak.
Ado removal, broader loading, parallel selection and generation-copy reduction
remain open. This is local development evidence, not cross-platform qualification.
