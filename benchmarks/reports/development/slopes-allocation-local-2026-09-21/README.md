# Single-slope allocation review, 2026-09-21

Source: `d2ec66e3a69f92628e4b2a109ae6b2a2ad85984c`.
The [fit checkpoint](../slopes-cache-local-2026-09-21/README.md) binds measured
artifact, scientific checks, exact production overlays and observed memory.

OLS contract v38 and action inventory v23 preserve all previous manifests.
The pre-refresh drift is retained, along with an initial invocation using an
older Python lacking tomllib; subsequent checks use the project environment.
`source-delta.json` records all eleven changed/new production files. Allocation
syntax adds four sites in the geometry helper and one fallback warning; helper
calls and their buffers are covered by production-body hashes, not site counts.

The reviewed geometry owner retains square-root weights, per-group fixed-size
Gram/scales and a flat QR basis. Construction temporarily doubles basis storage.
Projection retains untouched input, a separate result matrix, group coefficient
bank, norm vectors and group certificate reductions. Admission sums overlapping
capacities; sequential discarded work contributes a maximum and is released
before legacy fallback. The fit drops the cache before saved-effect recovery.
No Stata calls move to workers and no changing-weight/PPML session uses the cache.
The loader uses its existing bounded buffers; preparation discards temporary
slope categories only after complete sample classification.

This is a structural and component-lifetime review, not a complete whole-action
capacity proof. The reported allocation increase, constrained-memory testing
still to perform, and existing dependency/parent proof gaps remain explicit.

Repository checks and the focused exact-source boundary-pin test pass. The
broader mutation suite was deliberately interrupted to keep this inventory-only
refresh focused; its partial output is retained and is not counted as a pass.
