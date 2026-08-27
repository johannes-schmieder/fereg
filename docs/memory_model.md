# 0.5.0 memory model

## Modes and preflight

`fereg` accepts `memory(fast|low|auto)`. `estimate_phase2_memory()` computes
checked fast/low estimates from observations, finite regressors, absorbed and
cluster dimensions, and the effective thread pool. `plan_ols_memory()` compares
those estimates with available memory:

- fast is selected only when its estimate fits;
- low is selected only when its estimate fits;
- auto prefers fast and falls back to low;
- if neither fits, estimation returns the allocation error mapped to `r(909)`
  before large working allocations.

The `FEREG_MEMORY_LIMIT_BYTES` test seam supplies a deterministic artificial
limit for constrained-memory qualification; it is not a public Stata option.
A future `maxmemory()` option will feed the same planner; this release adds no
public memory-cap syntax or action-wire field.

After FE topology is known, the selected execution plan is combined with a
route-specific solver estimate. The estimate separately accounts for CG
residual/direction/projection matrices, LSMR RHS and Krylov storage, diagonal
and Schwarz workspaces, topology/cross-tab setup, and deterministic thread
reductions. Auto may select direct LSMR only when the complete execution plus
solver peak fits. If LSMR does not fit but CG does, auto visibly selects CG. If
neither certified route fits, estimation returns `r(909)` before allocating a
large solver workspace.

Topology-selected CMG uses the same joint budget. Before hierarchy
construction, fereg consumes CMG's upstream checked conservative estimate for
the graph, hierarchy, parallel plan, and reusable workspace pool, then adds
fereg's simultaneous graph RHS/solution vectors, nuisance Schur storage, and
retained coefficient product. After construction it consumes CMG's exact
retained-memory report and rechecks the hierarchy, plan, workspace pool, and
nuisance factor against the joint budget. A selected CMG route that does not
fit returns to the existing auto policy before graph allocation; an explicit
route can use certified CG only when that fallback also fits.

## Fast and low execution

Fast execution holds the prepared outcome/regressor batch and in-memory score
workspaces. For large multi-thread absorbed models, its preflight also reserves
the worst-case level-by-RHS group-mean table used by deterministic parallel MAP
projection. Low execution includes the corresponding outcome-plus-pool-width
bound, generates columns blockwise, and stores the prepared QR/score columns in
private bounded scratch files. Scratch files:

- use unique process/sequence names in the platform temporary directory;
- validate every dimension, offset, read, write, and finite value;
- keep bounded fallible I/O buffers rather than a second resident design;
- are removed by `Drop` on success, ordinary error, panic unwinding, or Break
  propagation.

Fast and low modes share the same rank policy, certificates, inference,
omission status, result wire, and Stata transaction. Registered tests require
scientific identity. Deterministic routes additionally require
same-architecture, same-thread-count bitwise reproducibility.

## Allocation discipline

Observation-scale vectors and matrices use checked arithmetic, fallible
reservation, and explicit capacity validation before mutation. Preparation,
factorization, topology, MAP, QR, score, VCE, result-container, and scratch
paths have allocation/store-failure injection tests. An allocation failure
cannot produce a complete receipt or partially committed public result.

Unit weights remain structural. Categorical and FE recipes reuse compact source
codes; interaction keys and cluster subset scores are streamed rather than
retained as dense observation-by-arity or group-by-parameter tables.
Direct borrowed LSMR also reuses prepared memberships and Schwarz inputs. Its
memory model reports the code/membership copy avoided relative to the former
owned adapter. Schwarz setup and Krylov solve are modeled as non-overlapping
peaks, and setup-only buffers may be dropped before the OLS/VCE stages.

## Instrumentation and acceptance

The public result wire records requested/selected memory mode, estimated fast
and low bytes, selected-model bytes, scratch peak bytes, allocator current and
peak bytes, allocation delta/ratio, process RSS start/current/peak/delta where
the OS exposes them, and action timings. `timeit` displays the registered
summary without changing computation.

Instrumentation is evidence, not a self-certifying claim. Exact-SHA
qualification must show:

- measured native allocation within 25 percent of its model;
- successful constrained-memory solver routing (`LSMR -> CG`) when only CG
  fits, clean pre-allocation failure when neither route fits, and the existing
  execution-mode constrained case where low succeeds and fast cannot; and
- no repeated-call leak.

Matched process-RSS and the former auto/MAP or auto/CG ratios remain reported
for comparison but are not promotion blockers. The final 10m direct-LSMR route
used more RSS at eight threads while staying within its modeled native
allocation and producing a material speed gain; this is the intended separation
between diagnostic resident memory and hard allocation safety.
