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

Concentrated one-intercept-FE PPML optionally admits its trial predictor/mean,
prior log-weight, group and finite-moment buffers in both fast and low modes
(ADR 0137). Low mode borrows original prepared numeric columns or generated
recipes; it adds no N-by-K design copy. The complete owner is charged through
strict solving and released before inference. Refusing its optional workspace
preserves the canonical route at its previous budget. Explicit allocation
audits additionally admit cold equivalence storage outside the measured window.

On macOS, automatic availability uses free plus inactive pages from
`HOST_VM_INFO64`. Mach's free count already includes speculative pages.
The original statistics prefix supports older kernel revisions, and each
query releases its host send right. If Mach statistics are unavailable, the
conservative free/speculative sysctl fallback remains. This changes neither
the planner's safety headroom nor an explicitly supplied test limit.

After FE topology is known, the selected execution plan is combined with a
route-specific solver estimate. The estimate separately accounts for CG
residual/direction/projection matrices, LSMR RHS and Krylov storage, diagonal
and Schwarz workspaces, topology/cross-tab setup, and deterministic thread
reductions. Auto may select direct LSMR only when the complete execution plus
solver peak fits. If LSMR does not fit but CG does, auto visibly selects CG. If
neither certified route fits, estimation returns `r(909)` before allocating a
large solver workspace.

Prepared direct intercept CG applies that joint check at its adapter boundary,
including explicit CG and preselected automatic CG calls that bypass the
LSMR/CG planner. It uses the installed team and charges five coefficient batches
when categorical CG retains them. Empty topology and slope adapters retain their
existing paths; see [ADR 0060](adr/0060-prepared-direct-cg-memory-admission.md).

The generic owned intercept/slope adapter separately admits code, slope and
membership construction and charges the retained design and materialized unit
weights beside each solver. Zero-control owned fits also reserve the normalized
weight, retained outcome and final residual overlap. The unified generic
dispatcher checks MAP, CG, accelerator and LSMR workspace at the route taken;
an admitted MAP pilot can finish without reserving LSMR, and an unaffordable
LSMR attempt can use the separately admitted existing CG continuation. LSMR
planning includes slope-channel whitening and signed Schur covers while leaving
the intercept-only model unchanged. See
[ADR 0062](adr/0062-generic-owned-fe-memory-admission.md). Specialized slope MAP
also admits topology validation and numeric workspace before allocation, then
checks their actual capacities. Recovery and public whole-action reporting
remain separate accounting boundaries.

Generic automatic slope absorption may retain certificate QR geometry for one
MAP/CG call (ADR 0116). Its bound adds `8*N*sum(channels)` numeric bytes and
one group descriptor plus up to `channels` vector descriptors per level. The
existing maximum-level certificate scratch stays charged alongside that bound,
covering construction and solve overlap. QR moves numeric vectors into the
cache; there is no second flat copy. Reservations are fallible. A declined or
failed cache leaves the original route available. LSMR releases it before
admission; CG releases it if the extra retained bytes do not fit. Reports keep
the maximum admitted attempt, including discarded cache construction. This
increases optional storage in both fast and low modes; it does not change the
minimum memory needed by an existing route or claim a new whole-action bound.

Parallel owned-FE certification adds at most
`8*max_effect_levels*(actual_RHS+1)` bytes beside the admitted geometry and
existing solver workspace (ADR 0117). One effect's result bank is reused within
each certificate, then released; no residual result survives that check.
Construction/size failure falls back to serial certification. Before CG, the
parallel scratch can be declined independently of the geometry cache. The
original cached or uncached route remains available at its previous budget.
Raw-moment tiles use the existing 768-byte stack payload per active worker;
no new pool or worker is created. Existing runtime/worker-stack headroom and
whole-action coverage limitations remain authoritative.

Automatic owned-slope MAP/CG projection can additionally admit
`8*(L*C*R + T*C*C) + T*sizeof(ProjectionWorker)` bytes (ADR 0118): one
coefficient bank, per-worker Grams and worker descriptors. L/C are the maximum
effect level/channel counts, R is actual RHS width and T is installed workers.
The serial workspace remains charged. Dimensions and actual capacities are
checked; allocation refusal retains serial execution. Admission follows the
existing geometry/certificate choices and cannot evict them. MAP drops its
bank before LSMR/CG; CG independently admits a new bank after choosing its
context. Earlier attempts remain in the reported maximum. Each row worker uses
the existing 256-byte fitted-value tile, with no new team. Coefficients never
survive a solver invocation or substitute for fresh certificates.

Public PPML's generic slope MAP session can retain the same optional projection
bank through its final Hessian solve (ADR 0119). R is the session's maximum
response/design or low-memory block width, and T is the explicitly resolved
team. `reusable_projection_bytes` is admitted beside the selected plan before
allocation and charged from preparation through ordinary/strict linear work;
it is released before inference. A budget or allocation refusal preserves the
serial workspace. Gram, coefficient and worker capacities enter the session
fingerprint. Only storage and immutable topology persist: every projection
rebuilds coefficients from its current weights and RHS. Specialized categorical
paths, other PPML engines and Mu separation exploration do not enter this
admission. Existing whole-action coverage gaps remain unchanged.

The action-local single-partition OLS slope cache separately admits retained
square-root weights, group Grams/scales and QR bases, plus construction scratch
or concurrent projection output, group coefficients, norms and certificate
partials. The input matrix stays untouched until acceptance. Both planned and
actual capacities are checked against the existing joint solver budget; a
declined cache uses the established separately admitted solver. The cache is
released after final residual projection. This fast-mode tradeoff increases
peak storage relative to in-place generic MAP; low memory retains its previous
route. See [ADR 0087](adr/0087-single-partition-ols-slope-geometry.md).

Topology-selected CMG uses the same joint budget and the explicitly estimated
runtime policy described in [CMG integration](cmg_integration.md): 32 MiB pool
heap, 2 MiB heap per worker and an explicitly requested 8 MiB stack per worker.
The estimate is retained with the solver until joined worker retirement. It is
engineering headroom, not a proven Rayon bound or an allocator hard cap; existing
whole-process headroom and unknown-coverage flags remain.

CMG-owned hierarchy, factor, plan and fixed solve-bank construction now use
matching upstream requested-layout admission APIs. The retained report is one
capacity union, including nested banks and shared Arc owners; the old overlapping
principal-array/container report is not added to it. Prepared PPML parents are
included before subtracting their already charged union. Fereg separately
charges its nuisance and output storage. Ordinary iterator ingestion and other
unresolved action scopes remain explicit. Memory rejection preserves existing
automatic alternatives and certified CG fallback, subject to their own admission.

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

Raw-moment/preparation overlap adds the existing three K-length result
vectors and deterministic partial bank to the preparation allowance beside
loaded RHS storage. The bound is `3*K*(chunks+1)*8` bytes, with the existing
chunk count capped at 64; a fallible reservation checks actual capacities.
Insufficient headroom leaves moment construction at its original fit phase.
No additional observation-scale matrix is allocated, and stale speculative
statistics are dropped before ordinary fit-time reconstruction.

Eligible bounded-width one-FE OLS fits and unit-weight two-FE fits with an
accepted initial Schur probe may overlap final residual projection with
reported bread/intercept completion. The residual vector transfers from the
existing QR owner into its existing one-column projection; the raw-residual
reference and moment cache already coexist with both phases. No second data
buffer or team is created. Retained packed/strided inference scores are already
charged as owners across both phases; overlapping them adds no cache or buffer.
Allocation-phase probes keep the sequential route.

The capped unit-weight two-FE Schur operator may store counted neighboring
edges in its idle group-statistics scratch. Admission checks N <= u32::MAX and
an existing capacity of at least 2N words. Codes/counts occupy the bits of those
words until the final certificate overwrites them. Parallel construction uses
disjoint membership ranges; it introduces no new allocation or resident owner.
Graph-based saved-effect recovery uses the existing admitted finishing reserve:
16 bytes per level for coefficients and a traversal queue, with the queue
released before component normalization. Reconstruction remains certified.

Categorical one-FE inference can overlap its numeric/topology validation prefix
with fitting on the same bounded team. The successful prefix retains only
immutable references. Effect-view and DoF-report allocations stay after fitting,
and interaction re-factorization stays outside this overlap. No additional
observation-sized owner or persistent validation cache is introduced.

Eligible multi-FE OLS also overlaps the independent outcome scan with fitting.
The scan owns no buffer. Ordinary topology validation borrows existing state;
categorical multi-FE DoF construction stays after the fit. Deferred errors
retain their original order. See
[ADR 0075](adr/0075-dense-two-fe-ols-data-and-solver-passes.md).

Fast and low modes share the same rank policy, certificates, inference,
omission status, result wire, and Stata transaction. Registered tests require
scientific identity. Deterministic routes additionally require
same-architecture, same-thread-count bitwise reproducibility.

Ordinary low-memory robust/clustered OLS retains original projected generated
columns before QR mutates its working scratch. The optional file borrows the
working I/O bank and takes it after QR, then replaces inference's former column
scratch. Actual post-QR identities select the columns for bounded row tiles;
independent raw means and score/covariance certificates remain. No additional
resident N-by-K matrix is created. Logical scratch peak is `16*N*G` bytes for
G generated columns, including omitted columns; this can exceed the previous
`8*N*max(G,2*R)` peak at finite rank R. File/copy failure releases the optional
file and preserves the existing uncached path. See
[ADR 0065](adr/0065-low-memory-ols-score-retention.md).

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

Scaled retained dense moments at up to 64 active columns may use a fixed
64-double stack cache per Gram-row partition when the layout assigns multiple
columns to each partition. Its payload is 512 bytes per partition, conservatively
at most 32 KiB across eligible partitions; it creates no heap allocation,
retained partial bank or observation-sized state. Wider, unscaled and scratch
paths do not enter that helper. Heap allocation receipts exclude this transient
stack payload; see [ADR 0054](adr/0054-retained-dense-moment-row-standardization.md).

Changing-weight retained column-block projection reuses the scratch file's
existing bounded encoded I/O bank for complete-row extraction and patching.
It does not add capacity or another retained design. Repeated full-row transport
can increase sequential bytes for narrow pools; admission and headroom are
unchanged. See the [local transport evidence](../benchmarks/reports/development/german-ppml-block-transport-local-2026-09-14/README.md).

Retained low-memory finite moments own bounded complete-row tiles per existing
Gram-column partition. Preflight charges at most
`min(threads,K)*min(N*K,max(K,1024))` doubles across every selected rank and stored
width inside the admitted dimensions. This replaces the one-row bound in
ADR 0020 paragraph 10. The envelope covers partition-count increases after rank
shrinkage and does not create observation-sized preparation state or another
retained design. Positional I/O keeps its existing bounded encoded stack buffer;
see [ADR 0070](adr/0070-bounded-retained-scratch-moment-tiles.md).

Each private tile may compact and standardize its selected suffix in place
before moment updates (ADR 0138). Strictly increasing selected positions make
forward compaction safe. This adds no storage and never mutates the scratch
artifact. Current-call one-FE certificate norms may replace a duplicate norm
scan only at exactly matching normalization; they do not become retained cache
authority for a later frame.

Retained scratch QR/span and residual scans keep an 8 KiB encoded page on the
stack across row reads (ADR 0129). Each immutable cursor has its own page and
small descriptor; no heap bank or resident design is added. The longer lifetime
can overlap other stack frames under existing runtime/stack headroom. This
does not claim a physical hard cap or complete stack accounting. Mutation ends
the borrow before a new scan can start; no page survives a weight-frame update.

Robust PPML inference uses the same immutable cursor per active fixed row block
under [ADR 0135](adr/0135-immutable-ppml-inference-scans.md). Each block retains
one 8 KiB stack page and its already reserved row buffer; production and
certificate meats remain separate. There is no new heap bank or resident design.
Dense sources, clustered PPML and one-shot OLS retain their prior row access.

PPML may tighten generation-zero FE cardinalities using tuple-run bounds and
bounded exact integer counts over borrowed inputs. This uses a fixed 256 KiB
stack bitmap, at most 128 passes per source, and no observation-sized pre-admission
allocation. Unsupported domains retain conservative bounds; the selected route
and headroom remain fixed. See [ADR 0071](adr/0071-bounded-ppml-fe-cardinality-preflight.md).

## Allocation discipline

Long estimation metadata uses one fallibly reserved transport buffer bounded by
16 MiB of payload plus nine bytes for the decoder prefix, truncation-detection
byte and NUL. It is released after parsing, before observation loading and PPML
generation-zero admission. This metadata cap is independent of observation count
and does not authorize pre-admission observation-sized preparation state. Stata
retains its own caller-local payload until the transaction returns; see
[ADR 0055](adr/0055-bounded-local-request-transport.md).

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

The borrowed LSMR adapter releases its column-major RHS transport before
materializing the row-major result. It then releases the returned demeaned bank
and unrequested coefficients before independent certification. The original
input remains available for reference norms and certified continuation; requested
coefficient products retain their existing lifetime. See
[ADR 0048](adr/0048-borrowed-lsmr-transport-lifetimes.md).

Categorical LSMR batches above both 64 RHSs and four times the installed team
are solved in windows of at most that team's width, retaining one weighted
operator and complete final output banks. Narrow batches retain upstream
transport; their memory estimate charges the full RHS width because nested
Rayon can suspend a recurrence and start another. The shared categorical
OLS/PPML checkpoint model includes coefficient-space recurrence/scatter state,
transformed input/output overlap, primary dense tables, weighted compensation
and Schwarz worker stacks/pools. See
[ADR 0049](adr/0049-bounded-lsmr-batches-and-memory.md).
Public OLS selected-memory reporting still omits topology-specific solver
increments; the source-bound development evidence does not close that gap or
prove universal bounds for opaque upstream factor storage.

Native solver reports carry a compact optional incremental workspace peak.
Construction, failed attempts, retained operator state, lazy pools, coefficient
conversion and fresh certification contribute to that peak. Storage retained
across a nested call is added; sequential attempts contribute their maximum.
Prepared OLS also propagates additional inference projections and discarded
Schur attempts. An unmodeled attempt makes its enclosing report unmodeled; it
cannot be reported as a zero-byte fallback. This internal report does not change
the public base-memory fields or establish a complete action-memory model.

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

PPML low-memory categorical MAP projects bounded scratch blocks with the
existing certified accelerator. Its RHS capacity is capped by generated model
width, even for oversized requested pools. Preflight charges parallel group/dot
banks and two-FE level-PCG or multiway observation-space recurrences at that
width, together with the scratch projection buffer. The retained design stays
on scratch; only fast storage fuses response and design. See
[ADR 0069](adr/0069-low-memory-ppml-categorical-projection.md).

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

The retained full reporting-axis covariance, standard errors, test statistics
and cluster/Wald identities coexist with final inference, recovery and
publication workspaces. Each of those phase sums includes the retained result;
the action envelope takes the maximum across the sequential phases. Saved-effect
recovery additionally admits its owned CG or slope-MAP workspaces, component
normalization, coefficient banks and complete-channel output construction.
Partial saves still require the complete decomposition certificate. The final
fit and prepared parent storage remain charged beside recovery. Recovery uses
the worker count admitted by the accepted fit and rejects a team mismatch
before allocating its workspace; all SPI calls remain on the caller thread.

PPML also freezes scalar metadata reservations at generation zero. Preparation
charges the copied availability request, nested term/operator strings and bound
effect identities alongside candidate/compact executable recipes. Retained
effect/cluster metadata and final coefficient/FE/cluster publication metadata
use their concrete type widths and string/source lengths; the final prepared
nested capacities must fit the admitted bound. A replacement generation includes
its own construction metadata beside the old generation. The boxed IRLS parent
and public result metadata are included in all fitted-parent phase sums. These
checks allocate no observation-sized reporting state.

The shared session inventory observes accepted-fit, retained-within and basis
capacities, including both workspace and frozen-result owners when present.
Canonical solve transfers the reusable residual into the accepted fit. A strict
candidate retains the prior residual only until commit or rollback; it belongs
to the strict overlap charge, not a second permanent observation vector. The
session row buffer is a separate K-double bank. Optional-cell validation during
plugin publication uses predicates without allocating index vectors.

## PPML separation memory

The prepared singleton fixed point uses one N-capacity machine-word queue
(reused for sorted output), two N-byte masks including its working activity
mask, and one exact `u128` multiplicity per unique FE-partition level. It also
reserves D+1 word offsets, D borrowed code/membership views and at most the
loaded-source count of categorical views. The planner bounds unique partitions
and levels by the declared FE dimensions/total levels and charges these concrete
type widths for initial, replay and separation-induced cascades in both routes.
The queue and selection mask are skipped when no singleton exists. Frequency
weights retain exact integer counting; unit and probability weights count rows.
No singleton observation state is allocated before memory admission.

Separation method records and generation hashes reserve the admitted generation
capacity. With strictly positive outcomes, no separation detector can accept a
row; only the initial recursive singleton fixed point can commit a deletion.
At most two generations (one with singleton removal disabled) are therefore
needed. The raw outcome scan runs after authoritative classification, retains
only a scalar flag and polls interruption before each bounded chunk. An excluded
zero/nonpositive value can retain the general N-generation bound but cannot
understate it. Samples containing any zero keep that general bound. The prepared
coordinator independently bounds its actual fallible reservations on the numeric
sample and uses bounded appends. Detector replay, reports, row reasons, and
transactional publication remain mandatory in either case.

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

The common PPML FE-topology envelope also covers owned `EffectTerm`
construction on MAP and exploratory Mu routes, which do not enter specialized
LSMR/CMG admission. Charge all retained codes, memberships, offset sentinels,
slope values and descriptors, plus the maximum single-effect count/cursor
scratch. Do not sum scratch for sequential effects or omit it because the
finished design no longer owns it. Preserve a larger existing conservative
bound. Checked scalar arithmetic runs before materialization and allocates no
observation-sized reporting state.

The [allocation inventory](allocation_inventory.md) separates structural source
coverage from capacity/lifetime proof and lists the remaining proof obligations.

## OLS retained preparation and inference storage

OLS observes capacities of retained prepared data after authoritative sample
preparation and before numerical execution. This includes nested metadata,
category levels, encoded interactions and membership indexes, counting shared
codes once. The observed owner augments the old base model only to the extent
it exceeds that model's explicit prepared-payload allowance. Existing finite
and working-vector reserves are preserved. Internal admission and automatic
fast/low selection use the reconciled parent; published base estimates retain
their established meaning.

Before derived outputs, the retained preparation and inference owners are
summed. Inference ownership includes residual/bread buffers, packed or strided
score caches, retained FE coefficients, DoF metadata and VCE products. Output
and recovery admission use at least that simultaneous total, while preserving
any larger execution reserve. Exact-budget acceptance and one-byte-short
rejection are checked before output allocation or public stores.

Before SPI row reads, a scalar bound also admits the value/missing arrays and
numeric compaction banks, maps and masks. Sequential construction stages use
their maximum. Obsolete pruning and availability state is released before
replacement storage when it no longer contributes to the final sample.
Numeric-source construction also has a checked scalar envelope for dictionary
growth, candidate/final topology, pruning, cluster codes and construction
metadata. Slope-channel metadata charges repeated categorical labels and source
identities per channel. This inspection allocates no observation-sized state.
Fixed-width string dictionaries and decoder/compiler work before this boundary
remain outside its coverage. These component envelopes do not define an
action-peak ratio.

## OLS request owners and saved slopes

The request and prepared output names remain live across the entire numerical
and publication portion of OLS. Their nested capacities are added to the
loading/preparation floor and to each reconciled execution/output parent.
This does not change published base-memory fields. The scalar inspection walks
metadata descriptors only and is checked against independent deallocation,
including unused capacity. Temporary parsing/compiler copies are a separate
construction scope.

Saved-effect recovery with slopes uses a checked per-route envelope. It
includes requested channel vectors, residual/total and coefficient banks,
component normalization, topology validation and nested metadata. LSMR adds
the existing pinned slope-reparameterization model and the certificate workspace
retained in its caller frame. Every attempted route is admitted independently;
failed attempts release their workspace before the next route. Certified MAP
remains available when its smaller envelope fits and LSMR does not. The returned
incremental peak includes earlier failed attempts. PPML retains its existing
recovery admission and route policy.

## OLS transient numerical frames

Internal execution admission takes at least the sum of the observed prepared
owner, retained request/channel metadata and a checked fast/low frame envelope.
The envelope covers finite QR/Cholesky storage, bounded moment partials, score
and covariance certificates, and cluster reductions. Failed finite attempts
release their local owners before fallback; sequential fit and inference frames
contribute their maximum. FE solver admission remains separate.

The scratch-backed full-rank trial uses the same finite frame allowance as
dense execution. Its largest simultaneous square owners are the caller Gram,
trial Gram, factor, inverse factor and canonical inverse; the inverse-factor
owner drops before finite bread is allocated. These five square banks and
linear metadata fit inside the existing `128*q*q + 1024*q + 4096` finite
allowance, where `q` is the larger generated/reported width. There is no added
row-sized buffer, and a rejected trial drops before scratch QR begins. See
[ADR 0111](adr/0111-guarded-scratch-finite-solve.md).

Dense cluster subsets can use the reducer's `max(32*N,64 MiB)` ceiling while
parallel partials remain live. Both banks are charged together. Sorted fallback
charges its two row orders. Low mode additionally reserves bounded scratch I/O
and score-cache buffers; private file payload is reported separately from heap
bytes. Published base-memory fields retain their established meaning.

Focused allocator fixtures check numeric construction and all three VCE frames
in fast/low modes. They begin with loaded inputs and a primed team. Such checks
do not establish cold dependency-pool coverage or all retained-coefficient
parent overlaps, and do not certify complete action memory.

## OLS publication storage

OLS admits a separate checked publication reserve before reading observations.
Both square shadow matrices use the reported coefficient count, including
omitted slots, rather than the smaller generated finite-column width. The
reserve also covers determinant/coordinate scratch, fixed result tables,
requested derived vectors and their registries. It takes the maximum across
output construction, shadow construction and final publication. Existing public
fast/low/base fields keep their original meaning.

This reserve and absorption workspace are sequential. Absorption's route
admission uses the reconciled execution parent without adding future publication
storage. Saved-effect recovery
adds its actually live caller capacities to that parent; the recovery estimate
already owns returned channels, normalization and certificates. Final
publication checks actual shadow/output/runtime capacities before any store.
These are additional component bounds, not a complete OLS parent certificate.

## Instrumentation and acceptance

The public result wire records requested/selected memory mode, estimated fast
and low bytes, selected-model bytes, scratch peak bytes, allocator current and
peak bytes, allocation delta/ratio, process RSS start/current/peak/delta where
the OS exposes them, and action timings. `timeit` displays the registered
summary without changing computation.

For OLS, selected-model bytes and the existing allocation ratio use the base
execution estimate. The separately admitted topology-specific solver workspace
is not included in that denominator. A ratio above one therefore does not by
itself establish a breach of the execution-plus-solver budget. The internal
optional solver peak is also insufficient to certify the whole action: its
preparation, inference, saved-output and publication parents must be reconciled
before a complete-action peak or ratio can be reported. Unknown coverage must
remain distinct from a known zero-byte scope.

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

## Development action-lifetime diagnostics

The plugin's opt-in `FEREG_ACTION_MEMORY_AUDIT` trace combines existing OLS and
PPML models in a fixed-size, versioned ledger. It adds simultaneous parents
and workspace and takes maxima over sequential stages/attempts. Missing stages
and unproven owner families remain explicit; they cannot produce a complete
whole-action denominator. The model trace precedes publication. An additional
version-1 terminal record observes action return/unwind before ABI error
reporting; neither is a public protocol change. See the source-bound
[proof obligations](allocation_inventory.md#action-lifetime-ledger-and-proof-boundary).

The private expected-use forecast is a separate channel from that conservative
ledger. CMG's revision-2 forecast composes requested capacities and actual
hierarchy dimensions with separately estimated construction scratch; it does
not substitute the allocator-observed peak for its model. Runtime heap headroom
and requested worker stacks remain in admission and outside the expected native
heap figure. Solver attempts carry both values so a larger admission reservation
cannot overwrite a smaller expected heap estimate. Unknown failed-constructor
temporaries invalidate the expected channel independently of admission. See
[CMG integration](cmg_integration.md#expected-heap-reporting).

Fast fits retaining FE coefficient products additionally admit their initial
and combined coefficient banks before solver-session construction, using actual
prepared levels and channels. The retained parent is included in the returned
solver envelope relative to the original execution base. The residual solver
therefore cannot silently use the budget already occupied by the initial
product. Uncached robust/clustered score reconstruction separately charges the
capacity of the retained final FE product. These parent reservations leave
the public base-memory fields unchanged. Complete inference and dependency/runtime coverage remains
an independent obligation.

## Input and metadata construction admission

Before native input construction, OLS and PPML admit value/missing/candidate
arrays, source-loader descriptors and SPI batch descriptors. Fixed-width string
sources additionally maintain a capacity ledger for all live dictionaries.
Dictionary growth includes old and new storage together; final ordering releases
buckets before admitting recodes. The production SPI checks the per-cell buffer
limit before reservation and returns that same buffer after truncating its NUL
terminator. Workers never access the SPI.

Argument transport and decoder/compiler tables have separate metadata-only
reservations before construction. Declared registry/recipe counts cannot request
storage beyond their supplied payloads. Original argument owners remain charged
after the temporary expanded local bundle is released. These reservations use
the existing safety headroom; they do not allocate observation-sized accounting
state or alter the public base-field definitions. The capacity inventory and
remaining whole-action exclusions are in `docs/allocation_inventory.md`.


The PPML retained-generation inventory separately bounds transferred raw input
and categorical/model capacities. It supplements existing allowances only where
those bounds are larger, and charges both current and replacement owners during
an atomic rebase. Original coefficient-slot capacity survives final-sample
rebasing even when logical slots disappear. Core callers receive the same raw
owner charge as plugin callers; an embedding observation already covering that
owner is not duplicated. These additions leave the existing base field meanings
and numerical policies intact. Initial generation construction is attributed to
preparation, including its allocation-failure path. Complete action coverage
still requires the remaining construction, numerical and dependency proofs in
[the allocation inventory](allocation_inventory.md).

## Retained OLS coefficient product admission

Fast OLS fits that retain FE coefficients reserve the initial all-RHS product
and the later combined one-RHS product before constructing the solver session.
For `K` generated controls, `D` ordered effects and `L` level/channel directions,
the parent reservation is `8*L*(K+2) + 2*sizeof(usize)*(3*D+1)`.
Repeated effects and slope channels retain their own axes; a virtual structural
constant has one direction. The active solver's own result/workspace remains
separately admitted. Initial and residual solver reports each include this
parent once, and sequential reports combine by maximum. Uncached inference
reabsorption charges the actual capacity of the final retained product.

At the initial solver return, the actual capacities of the coefficient values
and all three layout arrays, plus the future combined product, must fit that
reservation before finite-solve storage is allocated. Combination rechecks the
reservation before allocation and checks the returned capacities before handing
the final product to the fit. An excess fails with an allocation error; it does
not borrow unreported space from a retained session's captured budget. These
checks allocate no observation-sized reporting state and leave arithmetic,
solver-selection policy, tolerances and public base-memory fields unchanged.
Corrected admission may reject a tight budget or select an existing certified
fallback when the previously understated parent would have appeared to fit.

This establishes the retained-product handoff bound, not a proof of allocations
inside the solver that produced the initial product or of allocator/process
resident memory. Whole-action coverage remains incomplete and its ratio remains
undefined until all outstanding owner families are covered.

## Practical whole-action reporting

The schema-2 expected-use estimate follows ADR 0074 separately from strict
coverage and allocation admission. Native execution and Stata posting occur
sequentially: native action owners are released at ABI return before ado calls
`ereturn post`. The incremental forecast therefore takes their maximum. It
adds the pre-plugin Stata allocation and preexisting native live bytes once,
and also checks the Stata entry allocation. Resident bytes do not enter the
additional reserve, because the OS available-memory figure already excludes
them. No observed action peak is used to construct its own forecast.

Stata sample and saved-output variables already exist at the pre-plugin
boundary. Posting transfers the sample variable and renames saved outputs.
Numeric matrix posting receives 24 bytes per result cell plus 64 KiB for
descriptors and stripes. Factor-variable stripes additionally trigger
`buildfvinfo` temporary data storage and retained allocator capacity. The
Stata-19 engineering estimate rounds the larger of 20 bytes per original row
and ten percent of the pre-plugin Stata allocation to 32 MiB. These terms
describe measured publication behavior; they are not a proof of Stata's
private allocator or a multiplier fitted to total native memory.

Practical status 2 requires measured baselines, Stata 19, a valid expected-use
forecast and no missing action stage. Unknown scopes stay partial. The empirical
target is the larger of ten percent and 64 MiB on the fixed development cases;
small cases passing the absolute allowance do not establish ten-percent
relative accuracy. Calibration covers licensed Linux estimator runs and Mac
Stata posting probes with 1/32-MiB segments. Other datasets, allocator settings,
platforms and long-lived process fragmentation are extrapolations, not new
qualification claims. Requested stacks, runtime heap and allocator retention
retain explicit uncertainty and separate safety headroom. Strict gap bits,
strict action-peak ratios, scientific routing and conservative admission are
unchanged by practical calibration.

Direct single-partition slope recovery adds a target copy, two coefficient
layouts, an ordered group-certificate bank and bounded worker workspace
capacities to its established finishing envelope. Construction owners are
released before certification; actual capacities and rejected-attempt peaks
remain checked. A tighter budget can retain the existing MAP fallback. See
[ADR 0088](adr/0088-direct-single-partition-slope-recovery.md).

## PPML multipartition slope workspace

ADR 0123 admits a coefficient-space Schur solver instead of the optional repeated
projection bank. In addition to current-weight QR geometry, it reserves an N-by-RHS
normalized input bank, N square-root weights, and per worker four coefficient
vectors and one N-vector. The number of workers is bounded by requested RHSs
and effective threads. Group descriptors, worker descriptors and the boxed owner
are charged explicitly. Dense response fusion adds one N-vector for the prior
working response. Coefficient and QR reservations are fallible and checked against actual
capacities. Prior-response storage uses the existing fallible exact-reservation
session helper and enters its capacity fingerprint. No Krylov step grows storage.

ADR 0124 independently admits `8*sum(FE levels)*(maximum RHS+1)` bytes for
ordered parallel certificate results after the solver itself is admitted.
Budget/allocation refusal keeps the solver with serial fresh certification.
The vector descriptor is charged with its coefficient owner, and the explicit
sample/topology/weight-generation descriptor is charged with QR geometry.
Generation equality permits reuse only through the owning PPML slope session;
fresh residual checks remain mandatory and errors clear reuse authority.

The PPML plan charges this optional storage throughout the linear session,
including refresh, ordinary solves, line search and strict checkpoints, and
re-admits it after fresh telemetry. It is released before inference/recovery.
Failure to admit it preserves the existing generic route. Explicit low-memory
execution still uses bounded blocks, with the coefficient workspace sized to
its admitted RHS width. This development change does not repair the documented
static-allocation inventory drift or establish a whole-action allocation proof.

### Joint ordinary PPML slope directions

[ADR 0130](adr/0130-joint-ppml-slope-directions.md) borrows retained dense controls
and reuses the slope owner's normalized RHS bank, roots and worker scratch.
Its optional payload is `8*[P*K+2*K*K+(q+2)*K+5*(P+K)+N]` bytes for P retained
FE coordinates, K controls and q eliminated channels, plus owner descriptors.
The existing combined slope reservation is increased before allocation; refusal
keeps the previous solver. Actual capacities enter the session fingerprint and
fit-lifetime telemetry. Under
[ADR 0134](adr/0134-bounded-low-memory-joint-ppml-slopes.md), low storage streams
finite-product tiles through the existing per-worker observation buffers. Its
joint finite width is independent of the bounded RHS pool. Temporary norm state
and row scratch reuse preconditioner/solution banks before their solve lifetimes;
there is no additional N-by-K bank. One scan-local 8 KiB stack cursor follows
ADR 0129's immutable-read rule. All joint state is released before inference.
Fixed-configuration process audits remain exact-zero; the documented delayed
Crossbeam frees in mixed-thread audits remain unresolved. Static allocation
drift and physical-cap/platform gates remain separate.

[ADR 0136](adr/0136-low-memory-categorical-joint-ppml.md) admits the same joint
owner for low categorical MAP. For two/three effects, the canonical borrowed
projector coexists with the generic joint owner. The base session envelope
already includes categorical observation/graph banks and generic projection/
certificate storage. Additional admission charges both boxed owner descriptors,
borrowed effect records and seven RHS scalar buffers. The owned FE topology
exists once; canonical capacities enter the same session fingerprint. Optional
geometry/workspace/prior-response refusal consumes the generic owner to restore
the categorical route without a topology clone. Both owners end before inference.
The additional joint/coefficient payload follows the existing checked dimensions
and fit-lifetime reservation; low mode still retains its bounded scratch pool.
