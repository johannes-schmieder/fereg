# CMG memory forecast attribution

Local diagnostic, 2026-09-18. The dominant error is the use of conservative
preconstruction reservations as expected heap consumption. This does not
establish excessive CMG allocation or a numerical defect.

The investigation reads the preserved
[calibration and component comparisons](../pragmatic-memory-components-2026-09-18/README.md).
No estimation was rerun. Source inspected: fereg `e7ad6501`, CMG
`1d39c113f0c397f29653d92672090e5a42f80921`. The actual calibration artifacts and
source manifests retain their original identities in `attribution.json`;
inspecting the present checkout does not relabel those receipts. The CMG checkout
was clean and was not modified.

## Reconstructed costs

All three OLS cases contain 65,536 retained observations and request eight native
threads. Values are MiB. Observations are whole-ABI tracked native heap, not RSS.

| Quantity | Connected | Tiny components | Mixed components |
|---|---:|---:|---:|
| Reported native forecast | 162.05 | 396.36 | 290.40 |
| Observed native heap peak | 14.17 | 13.98 | 19.95 |
| Runtime heap allowance | 48 | 48 | 48 |
| Requested worker stacks | 64 | 64 | 64 |
| Recursive workspace reservation | 9 | 192 | 105 |
| Other vertex-level reservations | 2.06 | 44 | 24.06 |
| Reduction from exact edge count alone, holding all other terms fixed | 21 | 22.5 | 20.25 |

The terms above are an attribution, not disjoint observed allocations or a new
accepted forecast. The edge-count row is a counterfactual reduction, not an
additional cost. The independent arithmetic reconstructs the entire logged
solver workspace estimate to within 336–368 bytes, left explicitly unattributed
as small layout/bookkeeping costs. Detailed byte terms and receipt hashes are
in [attribution.json](attribution.json).

Three source decisions account for the large terms:

1. CMG `memory.rs::CmgMemoryEstimate::conservative` uses
   `level_vertices = vertices * max_levels`, with the default maximum of 128.
   Its recursive workspace term is `48 * level_vertices * concurrency`. It
   reserves every possible level at the original vertex count, rather than
   predicting shrinkage. For 4,096 vertices and eight workspaces this is
   exactly 192 MiB. Other hierarchy/plan/scratch terms reuse the same bound.
2. fereg adds the runtime policy's complete allowance: 32 MiB per pool,
   2 MiB heap per worker, and 8 MiB requested stack per worker. At eight workers
   this is 112 MiB plus handles. The 48-MiB heap portion is engineering headroom;
   requested stacks are outside the tracked Rust heap and are not necessarily
   resident. Comparing that sum directly to tracked heap mixes different metrics.
3. OLS preflight uses observation count as an upper bound on canonical edges.
   Repeated worker–firm observations are therefore charged as distinct edges
   throughout the sparse hierarchy/plan estimate. It also sums construction
   scratch with retained solver/workspace banks. These broad prebuild
   reservations do not encode actual stage lifetimes or selected execution mode.

The connection into reporting is explicit:
`fe/cmg.rs::preflight_cmg_memory` initializes `CmgWorkspacePlan` from the
conservative estimate plus runtime. Subsequent admission records use `max`, so
smaller actual ownership cannot reduce it. The plugin's
`estimate.rs::memory_forecast` then uses that solver workspace peak as expected
memory. The upstream component estimate delegates to the ordinary conservative
formula; it does not account for component-specific savings. PPML has the same
class of problem: its prepared-frame estimate takes the maximum of repeated and
legacy conservative bounds, then adds runtime headroom. In the saved-output
PPML receipt, the 112-MiB allowance alone is about 80% of the 140.38-MiB forecast.

## Are low-degree elimination or small components responsible?

The graph audit exactly replays the integer graph formulas in the preserved
generator. This is reconstruction from the generator, not a new inspection of
empirical German data.

| Case | Vertices | Distinct edges | Components | Vertex degrees |
|---|---:|---:|---|---|
| Connected | 192 | 8,192 | one of size 192 | 64 or 128 |
| Tiny | 4,096 | 4,096 | 1,024 of size 4 | all 2 |
| Mixed | 2,240 | 10,240 | one of size 192 plus 512 of size 4 | 2, 64 or 128 |

All receipts report zero singleton drops. The connected case has no low-degree
vertices and, with 192 vertices below the default direct threshold of 700,
reaches the direct terminal before grouping/coarsening. Nevertheless the estimate
reserves 128 vertex levels. Low-degree elimination is therefore not necessary
for the large error.

Small components explain why some actual work is cheap; the estimate fails to
reflect that. They do not indicate a broken component solver. Preserved
ordinary/component comparisons have identical connected-case native peaks;
component mode lowers the tiny-case median by only 0.422 MiB and the mixed-case
median by 0.225 MiB. Those differences cannot explain errors of hundreds of MiB.
Actual hierarchy depths for the tiny/mixed cases were not recorded in these
traces, so no measured depth or elimination savings are claimed for them.

## Consequence for the next change

Keep the existing admission policy and runtime headroom separately labelled.
Replace the expected heap forecast with a separate ownership/lifetime model
using canonical edges, actual hierarchy dimensions, component factors, selected
parallel execution and coexisting capacities. Keep failed-attempt costs and
PPML parent overlap. Requested stacks, opaque runtime heap, allocator retention
and process RSS require separate uncertainty/calibration.

Do not lower the solver's `max_levels`, disable elimination, disable components,
or fit a global multiplier to improve the memory report. Removing runtime
allowances alone would still leave the tiny-case forecast hundreds of MiB high.
These findings make a substantially closer forecast plausible; they do not yet
establish the 10%-or-64-MiB acceptance target or close whole-process coverage.

Reproduce locally with Python's standard library:

```sh
python3 benchmarks/reports/development/cmg-memory-attribution-2026-09-18/audit.py \
  --cmg-source /path/to/pinned/CMG > /tmp/cmg-attribution.json
```

The audit verifies retained rows, zero singleton drops, no degree-one vertices,
and arithmetic agreement within one KiB. It binds the source files, preserved
generator, manifest, input/artifact identities, traces and comparisons by hash.
No CMG code, numerical policy, installed artifact or historical receipt changed.
