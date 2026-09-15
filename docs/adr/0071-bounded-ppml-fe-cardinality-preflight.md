# ADR 0071: bounded PPML FE cardinality preflight

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-15

Refine the generation-zero PPML FE envelope from borrowed input columns after
successful authoritative row classification. Tuple runs upper-bound distinct
levels in any order. For loose bounds on samples of at least 4,096 observations,
exact bounded-integer marginal counts may tighten interaction product bounds.
A fixed 256 KiB stack bitmap covers disjoint intervals in at most 128 source
passes; noninteger, nonfinite, excessively wide or otherwise unsupported domains
retain the conservative bound. Counts include all loaded rows and cap at active
N, so later restrictions cannot exceed the admitted counts. Interrupted scans
publish nothing. Model-sized borrowed views remain fallibly allocated; no
observation-sized preparation allocation or new topology precedes admission.

This refines ADR 0020's preflight dimensions, preserving its fixed route and
capacity binding, raw weight/sample semantics, headroom and transactional output
rules. The fixed stack payload is separate from allocator heap telemetry. Private
itemized tracing is optional and outside the allocator observation window.

The [local evidence](../../benchmarks/reports/development/german-ppml-preflight-bounds-local-2026-09-15/README.md)
records exact-set oracles, weak weighted controls, full-input rejection probes,
paired command costs and the remaining full-fit admission/certification work.
