# Native allocation inventory and coverage

Two separate checks protect the current native source:

- `scripts/fereg_allocation_source_contract.py audit` enforces the OLS route,
  allocation syntax, literal payloads, Cargo/native wiring and exact one-shot
  boundaries against `fereg_allocation_source_contract_v18.json`.
- `scripts/action_allocation_inventory.py audit` covers every Rust source under
  both core and plugin, including PPML, separation and retained sessions, against
  `action_allocation_inventory_v1.json`. It also binds `Cargo.lock`. It does not
  exclude a file because the OLS-only route cannot reach it.

Both are structural guards. Their allocation counts are syntax counts, not
numbers of live buffers, bytes, or a complete call graph. A `.clone()` may copy
an iterator rather than allocate; a helper can allocate without familiar syntax.
Full production token and literal hashes catch changes in either case. The
parser removes provably test-only items within a file. An out-of-line test
module remains conservatively inventoried; in particular, singleton oracle
allocations in `fe/singleton_index_tests.rs` are not production memory charges.
Changing its parent cfg or adding a helper still changes the guarded source.

A passing structural inventory cannot establish complete memory coverage.
Capacity models and lifetime proofs must additionally cover all applicable
rows below. Unknown coverage cannot produce a defined whole-action ratio.

## Byte and lifetime obligations

| Scope | Existing model / implementation | Required coverage proof |
| --- | --- | --- |
| Request, loading and preparation | OLS `estimate_phase2_memory`; PPML allocation-free classification followed by admitted materialization; owned compaction in `ols/prepared.rs` | Count candidate capacity, all original-row masks, nested request/compiler metadata, factorization and interaction scratch. No observation-sized PPML preparation state before admission. OLS's base heuristic is not an exhaustive capacity inventory. |
| Prepared owners | Prepared numeric/categorical/cluster topology and PPML `common` | Include nested column recipes, labels, encoded levels, membership indexes and retained raw-owner/current-generation overlap. Do not substitute final lengths for unreleased candidate capacities. |
| Owned FE construction | `memory/owned_fe.rs`, PPML `fe_solver_topology_bytes` | All retained effect/code/member/offset/slope payloads coexist with one effect's count/cursor pair. The common PPML preflight must cover MAP and Mu as well as LSMR/CMG. |
| Numerical frames | OLS fit buffers; PPML `iteration_peak_bytes`, strict metadata and residual/predictor overlap | Establish parents at cold initialization, warm refresh, strict candidate/rollback, rank changes and failed-attempt recovery. Count old and new state when both remain live. Existing phase models alone do not prove all these paths. |
| FE attempts | `SolverWorkspacePeak`, `FeSolverMemoryBudget`, route-specific admission | Sum retained parents with a nested attempt; take maxima over sequential released attempts. Include failed setup, lazy pools, pilots, coefficient banks and certificate/recovery storage. Propagate `UNMODELED`; do not turn it into zero. Generic/unsupported reporting scopes remain unresolved. |
| Finite solve and inference | Wide Cholesky/QR fallback; packed/strided/scratch scores; PPML retained inference result | Include failed finite attempts, cluster subset buffers, production and independent reducers, rank/status/bread/covariance metadata, and parents retained during inference. Route-specific envelopes do not establish the complete OLS action. |
| Saved outputs | Intercept recovery admission; PPML complete-channel recovery and requested-output inventory | Include parents, normalization, all recovered channels even for partial save masks, total/channel vectors and certificates. OLS slope recovery and all publication-parent overlaps need explicit coverage. |
| Publication | Core certified result and plugin shadows/validators | Count simultaneously live coefficients/covariance, row maps, result vectors, optional-cell inventories, temporary validators and request/output-name storage until the COMPLETE receipt. Preserve rollback on allocation failure and interruption. |
| Runtime/dependencies | Explicit worker counts and pinned `within`/Rayon dependencies | Model opaque retained/lazy containers or keep their coverage unknown. Distinguish native tracked allocation, private-file scratch and process RSS; Stata-owned storage is not interchangeable with native heap bytes. |

This table defines review obligations, not a grant of complete coverage. The
modelled German OLS base-plus-CG envelope and the measured native peak address
one particular route; they do not discharge every row or establish a cap breach
from the old base-only ratio. See [the reproduction](../benchmarks/reports/development/candidate-german-ols-memory-2026-09-15/README.md).

## Reviewed OLS source transitions

The immutable v17 and F0 records remain unchanged. F0 is still
`2dee5d8c4cd99b51bbaca600348f66f325b58ab4`. V18 adds the previously unwatched
`memory/**/*.rs` closure and pins four exact replacement bodies from
`a765c338f252350d0a9f5bf496330181dfbdb4b8`:

| Boundary | Reviewed change |
| --- | --- |
| Plugin `estimate_ols_test` | Move loaded owners into shared preparation, reuse the bounded worker team after SPI loading, and pass the owned result to the same one-shot inference/publication boundary. Six VCE/topology branches retain their estimator policies. |
| Ordinary robust adapter | Transfer the optional low-memory projected-column scratch owner from fit to inference. |
| Categorical robust adapter | The same scratch handoff with categorical DoF/result composition preserved. |
| `fit_prepared_ols_coefficients_view_map` | Explicitly omit the optional retained-score donor for the ordinary one-shot fit. |

The first transition is supported by the preparation receipts and
[ADR 0036](adr/0036-native-ols-input-preparation.md) /
[ADR 0042](adr/0042-candidate-run-numeric-loading.md). The three score-boundary
transitions follow [ADR 0065](adr/0065-low-memory-ols-score-retention.md).
F0 literal pins, dispatch bindings, prohibited session/PPML tokens and exact
live-boundary checks remain enforced. Rendering a new manifest cannot authorize
a different replacement body. Mutation tests verify that property. These are
later development changes, not a retroactive claim of pure F0 extraction or
formal qualification.

The allocation-bearing changes since the v17 source checkpoint `d44e36b` were
reviewed by ownership family:

- Native loading, stable weighted compaction and cluster preparation retain
  the authoritative classifier and bounded candidate capacities (ADRs 0036,
  0038, 0042). Additional `Vec` operations implement owned construction; their
  presence does not mean a second full design is retained.
- Borrowed absorption, LSMR transport/batches, bounded interaction/cluster
  tables and score caches follow ADRs 0037, 0043–0049, 0059–0060 and 0065.
  Dense-table guards retain checked ceilings and certified fallbacks.
- Generic FE admission, saved-output precision/recovery and low-memory score
  ownership follow ADRs 0062–0065. The memory carrier and fallback additions
  retain unknown scopes and do not authorize an aggregate action claim.
- Singleton membership indexing replaces per-deletion full scans; its tests
  preserve legacy and independent masks, supported multiplicities, allocation
  failure and interruption. The larger scale receipt is
  [preserved separately](../benchmarks/reports/development/candidate-singleton-scale-2026-09-15/README.md).
- Metadata/export wiring and timing/error diagnostics are inventoried even
  when allocation-site counts do not change. SPI stays on the calling thread.
  No OLS adapter constructs a retained PPML session.

The original failed v17 audits at baseline `9ecf47d` and checkpoint `a765c33`
remain in [their report](../benchmarks/reports/development/candidate-source-contract-audit-2026-09-15/README.md).
The newer inventories describe their own source, never those historical runs.
