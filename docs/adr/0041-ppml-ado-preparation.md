# ADR 0041: Remove redundant PPML ado preparation passes

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-12

PPML's three nested entry points no longer use `sortpreserve`. They perform no
observation sorting: Stata expands metadata, Rust loads/classifies/factors native
sources, and the transaction publishes through private shadows. Tests verify
original order and stored sort metadata on success and failure. Candidate
markers, public weight-expression evaluation and the protected publication
transaction remain unchanged.

The transaction sends the existing optional `input_complete=0` transport hint
without first copying `touse`, marking source missingness and counting that
temporary marker. Native numeric loading already records every missing value
on the same bulk path regardless of this hint. Original-row classification,
weight/offset/exposure domain precedence, allocation-free PPML preflight and
exact post-admission materialization remain authoritative. The true-hint
consistency check remains available to native callers. Final sample/receipt,
drop-count and scientific publication checks are unchanged.

The [local source-bound report](../../benchmarks/reports/development/ppml-ado-preparation-local-2026-09-12/README.md)
uses the identical native plugin in both arms. Across 25 paired comparisons,
scientific exports and solver work are exact. Primary median paired command
improvements are 0.4–2.4%, with outer Stata cost reduced by about 32–170 ms and
unchanged primary native peaks. A noisy low-memory control, all repeats and
the limits of attribution are retained. This closes the ado audit in issue #27
P1; broader SPI batching and later work packages are not implied complete.
