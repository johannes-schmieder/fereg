# Absorbed-slope baseline and allocation review, 2026-09-21

Development evidence for unchanged estimator source
`0efa78bd5edaf96e5dcce5704b1ab198e2d2f8df`, Apple M2 Ultra, native/R threads 1
and 16. No release or cross-platform claim.

At N=245,760, G=49,152, one intercept-and-slope partition, three alternating
fresh-process pairs reproduce the gap against the pinned default fixest:

| Regressors | Threads | fereg median ms | fixest median ms | Median paired ratio |
|---:|---:|---:|---:|---:|
| 10 | 1 | 359 | 101 | 3.520 |
| 10 | 16 | 336 | 54 | 6.148 |
| 50 | 1 | 1,035 | 667 | 1.544 |
| 50 | 16 | 854 | 193 | 4.425 |

All 24 application calls and twelve coefficient/covariance/RSS/sample comparisons
pass. A separate N=360/K=2 fixture passes a dense explicit-dummy oracle for both
arms. Four instrumented calls reproduce native scientific exports bitwise.
For the first K=10 diagnostic, six certificate calls total 223.785 ms, versus
16.206 ms for two projections and 4.157 ms for owned design construction.
Nested auto/MAP timers must not be added to their certificate/projection children.

`summary.json` contains paired timings, all diagnostic phases, and exact native
comparisons. `evidence.zip` preserves numerical exports, source identities,
workers, application sentinels, input hashes and oracle receipts. Workspace
prefixes in text are replaced with `<repository>`; `archive-hashes.json` binds
original and portable bytes. Raw licensed logs and generated data stay ignored
under `target/ols-next-20260921`. The baseline build embeds the older helper's
September 19 date; the instrumented build embeds September 21. Their identities
and diagnostic source changes remain separate. The initial runner import failed
before input generation or dispatch, and was corrected before these runs.

The source/allocation review compares the old inventory checkpoint `e23f15ca`
with this baseline. `allocation-site-delta.json` records canonical added/removed
allocation sites in all 32 changed production Rust files. New source contract
v37 and action inventory v22 preserve all earlier manifests. The 102 historical
checker findings remain in the linked
[cleanup receipt](../repository-cleanup-2026-09-21/README.md).

Reviewed changes include fallibly reserved ordered factor/membership buffers;
borrowed finalized descriptors and transferred topology owners; pre-reserved
prepared-source tables; bounded graph queues; and the native numeric loader's
two 4,096-row, at-most-64-column tiles. The latter charges planned and actual
capacity, calls SPI on the main thread, and closes its worker on unwinding.
Deferred numeric columns and optional RHS storage have explicit input-memory
admission. Several syntax additions are nonallocating iterator/error clones,
empty ownership-transfer vectors, or function-name changes. Batched C access
retains row/column failure order and main-thread checks; it adds no scientific
implementation to the bridge.

Exact reviewed boundary pins now resolve to this baseline; historical F0,
prepared-parent and input-memory excerpts remain independently bound. The
repository checker passes. This review does not close the previously documented
whole-action allocation proof gaps or alter historical receipt applicability.

The 39 inventory tests initially passed 37 and exposed two stale mutation
anchors (the renamed loaded-moments call and increased trace-literal count).
After updating those anchors, both rejection tests pass. The initial failures
and focused rerun are retained; no guard was relaxed.
