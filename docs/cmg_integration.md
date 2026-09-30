# CMG admission and estimated runtime policy

This document describes the frozen legacy adapter and its evidence. The rebuild
uses the shared FE interface in [ARCHITECTURE](ARCHITECTURE.md), with separate
immutable dependency identities in `references.lock.toml`.

The legacy runtime dependency is pinned to CMG
`1d39c113f0c397f29653d92672090e5a42f80921`. Cargo enables `parallel` and
`experimental-components`. Explicit `solver(cmg)` uses component-aware CMG:
coarse-isolate pruning, preserved unpruned stopping, and component terminal
factors together. Automatic routing still selects the ordinary engine. No new
Stata option or ordinary-CMG retry is introduced. The private
`FEREG_CMG_ORDINARY` comparison override restores ordinary CMG for development
measurements; it is not part of the supported command surface.

The bounded development comparison is recorded in
[the component/memory report](../benchmarks/reports/development/pragmatic-memory-components-2026-09-18/README.md).
It covers connected, tiny-component, mixed-component, tiny weighted bridge,
low-memory clustered OLS and saved-output PPML cases. It does not establish a
universal speedup or formal release qualification.

## Runtime and lifetimes

Each graph solver owns one executor and the returned `ParallelRuntimeEstimate`.
The explicit policy reserves 32 MiB pool heap, 2 MiB heap per worker and 8 MiB
requested stack per worker. Thread count comes from the existing fereg resource
policy. The constructor admits the full estimate before starting workers and
requests that stack size. At 16 workers the allowance is 192 MiB plus the known
handle bank. Retained capacity reports include the handles, so only the separate
runtime heap and stack allowances are added afterward. Joined shutdown keeps the
pool charged through worker retirement. Coexisting solver owners need separate
allowances in their enclosing caller frame.

This is user-approved engineering headroom, not a proven Rayon bound, allocator
cap or OOM guarantee. Runtime coverage remains unbounded in the CMG report and
in fereg's whole-action coverage diagnostics. Existing whole-process headroom
remains necessary for allocator caches, metadata, native resources and deferred
runtime reclamation. Do not add a second host charge for these same worker
stacks. `FEREG_SOLVER_TRACE` identifies the estimated policy and its separate
heap/stack quantities; public base-memory fields are unchanged.

## Admission boundaries

The existing pre-graph conservative screen remains an early rejection envelope.
After graph construction, each CMG-owned hierarchy/factor stage is admitted
against its upstream requested-layout bound plus the union of live parents and
runtime allowance. Ordinary edge iterator ingestion remains a separately modeled
legacy boundary, not a newly proven allocation scope.

Prepared PPML topology uses slice-based admitted construction, with its temporary
endpoint bank included in fereg's preparation envelope. Assembly scratch and
numeric generations use their matching budgeted APIs. PPML charges the retained
prepared owner in execution storage. Frame measurements include it in the full
union before subtracting the already charged parent union, so shared component
and lineage Arcs are not counted twice.

The routing query can initialize a plan. Fereg first checks conservative pinned
eligibility dimensions, including a possible single-RHS tail, and admits the
solver-owned plan before querying routing. It does not construct another pool
or plan. Because upstream keeps the connected-component count private, a large
disconnected graph may conservatively prepare a plan before the routing query;
the solver still makes its unchanged decision. A focused test checks routing queries cannot grow admitted solver
storage.

Both OLS and prepared PPML provision fixed PCG banks and caller-owned RHS,
solution and diagnostic arrays before solving. OLS keeps its original batch
widths and nuisance correction solves. Only prepared PPML reuses current-frame
nuisance potentials and its existing warm seeds. No tolerance, weight, rank,
certificate or fallback policy changes are intended.

Retained storage uses one `CmgStorageReport` union of the solver, fixed banks,
caller numerical buffers and applicable prepared parents. Fereg adds its own
nuisance descriptors/factors and diagnostic capacities. The old private CMG
container reconstruction is no longer a production module, and its overlapping
principal-array report is not added to this union. Independent fereg preparation,
projection, coefficient, inference, recovery and publication storage still has
its own accounting. The upstream Arc layout remains a reviewed model rather
than a stable Rust ABI, and small Arc allocations remain infallible.

Admission records stage maxima and rejects before the covered allocation.
Observation-sized reporting state is not allocated. Failure preserves existing
certified alternatives and public publication atomicity. This integration does
not establish complete whole-action coverage or release readiness.

## Expected heap reporting

The expected-use channel is independent of admission. It retains actual CMG
capacity unions, selected workspace counts and plans, and uses the constructed
hierarchy dimensions for transient forest/contraction/factor estimates. The
finest graph's spare edge capacity is counted even when duplicate observations
collapse to fewer canonical edges. Component factor scratch uses actual factor
nonzeros; ordinary factors retain their dense construction allowance. This is
an engineering model of requested heap, not a proof of complete construction.

The forecast excludes the separately admitted opaque-runtime and requested-stack
allowances. These exclusions remain explicit whole-process uncertainties, not
assertions that the runtime consumes zero memory. No solver option, stopping
rule, admission budget or dependency pin changes for reporting.

Solver diagnostics carry admission and expected peaks separately through maxima
over sequential attempts and sums with simultaneous parents. OLS polishing
charges the retained CMG heap; final correction can release it. PPML preserves
the prepared/fused parent across ordinary frames, retries and strict checkpoints,
and retains earlier peaks when rebuilding after a basis change. An opaque setup
failure makes the expected channel unavailable rather than reporting zero.
Private `FEREG_MEMORY_FORECAST` records identify model revision 2; the public
11-by-2 matrix schema and partial-coverage status are unchanged.
