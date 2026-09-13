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

Ordinary fast OLS robust/clustered inference uses a packed score cache up to
32 MiB. Larger eligible Cholesky fits retain the existing solve matrix through a
strided view instead of adding a second score-value matrix. QR score rebuilding
and low-memory scratch remain governed by their existing plans. The selected
memory estimates are unchanged; a lower cache cutoff does not itself admit a
previously over-budget fit. See
[ADR 0043](adr/0043-ols-inference-strided-cache-policy.md).

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

Low-memory OLS cluster subsets may replace their `N*sizeof(usize)` tuple-order
allocation with compensated group scores and presence flags only when the
entire table fits within that same byte budget. The two representations do not
overlap. Larger or unrepresentable Cartesian products retain tuple sorting;
scratch tiles and independent per-group bread checks remain unchanged. This
does not increase admission or retain a second observation-by-control design;
see [ADR 0044](adr/0044-sequential-low-memory-cluster-scores.md).
For multiple dimensions, all subset score tables may instead share one row pass
when their combined values, compensation, presence flags, range descriptors and
row-code scratch fit the same ordering budget. Planning is all-or-none, the bank
is action-local, and effective frequency counts never enlarge its physical-row
budget; see [ADR 0045](adr/0045-batched-low-memory-cluster-scores.md).

Above 64 finite columns, fast preflight additionally reserves
`32*K^2 + 128*K` bytes for guarded Cholesky's four live square work/inverse/bread
arrays and linear metadata, beyond the existing cross-product allowance.
The wide moment builder writes disjoint Gram rows without a partial bank or
another observation-sized design. This increment is independent of thread
count and uses checked arithmetic. It does not change low-mode estimates or
the at-most-64 execution policy (ADR 0030).

## Allocation discipline

Observation-scale vectors and matrices use checked arithmetic, fallible
reservation, and explicit capacity validation before mutation. Preparation,
factorization, topology, MAP, QR, score, VCE, result-container, and scratch
paths have allocation/store-failure injection tests. An allocation failure
cannot produce a complete receipt or partially committed public result.

Unit weights remain structural. Categorical and FE recipes reuse compact source
codes. Interaction keys are streamed rather than retained as a dense
observation-by-arity matrix; cluster score tables obey the bounded alternatives
above.
Categorical FE/cluster interactions may replace row sorting with a u32 mapping
whose complete Cartesian table fits the displaced physical-row ordering bytes.
The output codes double as temporary tuple slots; sparse/overflowing products
retain sorting without partial table allocation. See
[ADR 0046](adr/0046-bounded-interaction-coding.md).
Direct borrowed LSMR also reuses prepared memberships and Schwarz inputs. Its
memory model reports the code/membership copy avoided relative to the former
owned adapter. Schwarz setup and Krylov solve are modeled as non-overlapping
peaks, and setup-only buffers may be dropped before the OLS/VCE stages.

ADR 0032 retains the borrowed LSMR operator only within the low-memory
categorical OLS projection phase. Its finalized topology and weights are fixed;
weighted construction reserves one owned weight copy fallibly, while unit
weights allocate none. Existing joint execution/solver admission applies to
each batch. The operator is dropped before a CG continuation and before
finite QR, VCE and saved-output recovery, so those stages do not overlap a
new retained solver. RHS transport and certification workspaces remain per
batch; this is setup reuse, not an allocation-free projection path.

PPML joint and checkpoint LSMR preflight also charges the pinned Schwarz
builder's dense cross-tabulation scratch. Its dispatch depends on both the
product of retained FE level counts and the sparse observation-count cost;
a sparse graph can therefore use a dense transient table. All eligible pair
tables are charged concurrently for checkpoint construction, while ordinary
three-or-more-FE joint LSMR charges only its selected core pair. These are
construction peaks, not fit-lifetime retained matrices.

Weighted categorical Schwarz assembly additionally uses compensated diagonal
and edge sums. OLS and PPML preflight charge `8*(R*C+R+C)` transient bytes for
each dense intercept pair and `8*(R+C)` for each sparse pair, with all
concurrent pairs included. Sparse construction reuses column correction
storage for its edges. These corrections are released during construction;
they do not increase fit-lifetime retained state or weaken graph validation.

## PPML result and saved-FE ownership

PPML publication retains observation-scale products strictly from the request
mask. A fit-only transaction returns no `xb`, absorbed predictor, `eta`, `mu`,
or residual vector. A requested `xb` is formed once rather than retaining its
offset and finite-predictor inputs, and the plugin publication shadow borrows
every retained prediction, residual, and saved FE channel from the certified
core result. It does not construct a second observation-length shadow copy.

The composite result constructor needs one final-mean vector to replay the
accepted objective independently. Requested `mu` reuses that same vector. If
`mu` was not requested, the vector is released immediately after the composite
certificate closes; a requested residual already owns only its exact final
residual vector. The publication plan therefore charges outcome, prepared
weights, and the one certification mean, plus exactly one vector for every
distinct requested output other than `mu`.

Saving any absorbed channel triggers recovery of the complete normalized FE
decomposition under the accepted final Hessian weights. During that phase the
planner charges one transient total absorbed predictor and every declared
intercept/slope channel, even when only a subset was requested. Recovery
compares that transient total with the strict fit's authoritative absorbed
predictor and certifies the sum of all channels, retaining only the two scalar
errors afterward. The transient total and unrequested channels are released;
publication owns only the requested channels (and the authoritative absorbed
predictor only when it was itself requested). Preflight and final planning use
the same saved-channel and requested-output dimensions, so a saved intercept
or slope cannot create an uncharged post-preflight vector.

## PPML separation memory

The PPML path adds its own fit-lifetime plan without changing OLS
memory modes or thresholds. Its simplex detector operates on the complete
prepared model span, including finite columns, one indicator per finalized
intercept-FE level, and one level-by-raw-slope column per absorbed slope
channel. Before allocating the detector's explicit N-by-P design or phase-I
tableau, checked arithmetic bounds the maximum jointly live design,
topology/FE-code construction scratch, null-space work, tableau, and
certificate/result vectors in eight-byte cells.

An over-cap detector records `SkippedMemory`. It may continue only when a
requested comprehensive iterated-rectifier run certifies no unhandled
separation on the same sample-ledger generation. If IR accepts rows, the
coordinator rebases and restarts the full chain; if comprehensive IR is absent
or cannot close that final generation, the request fails. A simplex-only
over-cap request returns `r(909)` before the N-by-P design or tableau is
allocated. The bounded explicit design is detector scratch and is released
before PPML IRLS, inference, or publication.

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
