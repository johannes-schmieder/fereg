# Native allocation inventory and coverage

Two separate checks protect the current native source:

- `scripts/fereg_allocation_source_contract.py audit` enforces the OLS route,
  allocation syntax, literal payloads, Cargo/native wiring and exact one-shot
  boundaries against `fereg_allocation_source_contract_v38.json`.
- `scripts/action_allocation_inventory.py audit` covers every Rust source under
  both core and plugin, including PPML, separation and retained sessions, against
  `action_allocation_inventory_v23.json`. It also binds `Cargo.lock`. It does not
  exclude a file because the OLS-only route cannot reach it.

The [September 21 baseline review](../benchmarks/reports/development/slopes-baseline-local-2026-09-21/README.md)
binds v37/v22 to the unchanged estimator at `0efa78bd`, records the allocation-site
delta from the earlier inventory, and preserves its 102 historical findings.
No complete action-memory proof is implied.

The [single-slope review](../benchmarks/reports/development/slopes-allocation-local-2026-09-21/README.md)
binds v38/v23 to `d2ec66e3`. It covers action-local fixed-weight geometry,
separate candidate residual storage, bounded group reductions and unchanged-input
fallback. Previous manifests and whole-action proof gaps remain intact.

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
| Request, loading and preparation | OLS `estimate_phase2_memory`; PPML allocation-free classification followed by admitted materialization; owned compaction in `ols/prepared.rs` | Count candidate capacity, all original-row masks, compiler construction, factorization and interaction scratch. Retained OLS request/output-name capacities are admitted separately. No observation-sized PPML preparation state before admission. OLS's base heuristic is not an exhaustive capacity inventory. |
| Prepared owners | Capacity-checked OLS numeric/categorical/cluster owners; PPML transferred-input and retained-generation supplements | OLS retained owners have independent deallocation checks. PPML admits raw owners and both old/new generation bounds, then validates retained capacities before replacement. Construction and strict-session frames remain separate obligations. Do not substitute final lengths for unreleased candidate capacities. |
| Owned FE construction | `memory/owned_fe.rs`, PPML `fe_solver_topology_bytes` | All retained effect/code/member/offset/slope payloads coexist with one effect's count/cursor pair. The common PPML preflight must cover MAP and Mu as well as LSMR/CMG. |
| Numerical frames | OLS fit buffers; PPML `iteration_peak_bytes`, strict metadata and residual/predictor overlap | Establish parents at cold initialization, warm refresh, strict candidate/rollback, rank changes and failed-attempt recovery. Count old and new state when both remain live. Existing phase models alone do not prove all these paths. |
| FE attempts | `SolverWorkspacePeak`, `FeSolverMemoryBudget`, route-specific admission | Sum retained parents with a nested attempt; take maxima over sequential released attempts. Include failed setup, lazy pools, pilots, coefficient banks and certificate/recovery storage. Propagate `UNMODELED`; do not turn it into zero. Generic/unsupported reporting scopes remain unresolved. |
| Finite solve and inference | OLS checked `OlsFrameMemory` envelope beside observed parents; wide Cholesky/QR fallback; packed/strided/scratch scores; PPML retained inference result | Include failed finite attempts, cluster subset buffers, production and independent reducers, rank/status/bread/covariance metadata, and parents retained during inference. Route-specific envelopes do not establish the complete OLS action. |
| Saved outputs | Intercept recovery admission; PPML complete-channel recovery and requested-output inventory | Include parents, normalization, all recovered channels even for partial save masks, total/channel vectors and certificates. OLS slope recovery has route-specific admission; complete execution-frame coverage remains separate. |
| Publication | Core certified result and plugin shadows/validators | Count simultaneously live coefficients/covariance, row maps, result vectors, optional-cell inventories, temporary validators and request/output-name storage until the COMPLETE receipt. Preserve rollback on allocation failure and interruption. |
| Runtime/dependencies | Explicit worker counts and pinned `within`/Rayon dependencies | Model opaque retained/lazy containers or keep their coverage unknown. Distinguish native tracked allocation, private-file scratch and process RSS; Stata-owned storage is not interchangeable with native heap bytes. |

This table defines review obligations, not a grant of complete coverage. The
modelled German OLS base-plus-CG envelope and the measured native peak address
one particular route; they do not discharge every row or establish a cap breach
from the old base-only ratio. See [the reproduction](../benchmarks/reports/development/candidate-german-ols-memory-2026-09-15/README.md).

## OLS publication admission

V20 and action inventory v3 cover the incremental OLS publication model. The
preflight uses the full reported coefficient axis, including omitted slots,
for both square shadow matrices. It also charges the determinant copy,
coordinate validators, fixed-size tables, requested observation outputs and
nested saved-channel registries. Construction and final publication are
sequential maxima. The scalar model allocates no observation-sized state.

The publication reserve participates in fast/low selection before observation
loading; the established public base estimates remain unchanged. Absorption
keeps its separate solver admission because publication has not started yet.
Saved-effect recovery adds the capacities of its live caller buffers to that
base: finite-term tuples, xb/target values, empty registry and request mask.
Returned channels and recovery scratch belong to the existing recovery model;
they are not charged twice. The later xbd/shadow allocations are excluded from
that recovery parent. Before any public store, observed shadow, output and
runtime capacities must fit the preflight reservation.

These incremental owners are covered beside the existing base/solver models.
This does not prove that those older parent models are exhaustive. Compiler construction and opaque dependency growth retain their incomplete
coverage designation; the separate request-owner and slope-recovery bounds
below supplement these component models.

## OLS preparation and inference parents

V21 and action inventory v4 add retained-owner accounting at all six OLS
VCE/topology branches. Prepared numeric arrays, original-row reasons, nested
recipes, FE/cluster labels, category levels, codes and membership indexes use
their allocated capacities. Shared topology codes count at their owner only.
Inference results include residuals, bread, score caches, FE coefficients,
warnings, DoF metadata, covariance, standard errors and cluster subsets.
These observations walk container descriptors; they do not scan observations
or allocate reporting arrays. Independent allocator-deallocation tests check
the retained totals, including excess capacities.

Before fitting, the observed prepared owner replaces only the explicitly
identified prepared-payload allowance in the old base model. All other
numerical reserves remain, and a smaller observed owner never shrinks them.
Internal mode selection and solver admission use this reconciled execution
parent. Before derived outputs or recovery, admission uses the larger of that
reserve and the simultaneously live preparation-plus-inference owners. Repeated
checks do not cumulatively charge the same owner. Public base fields preserve
their previous definitions.

A separate checked preflight covers SPI value/missing arrays and the old/new
numeric banks, row maps and masks that can coexist during compaction. It takes
the maximum of classification and singleton-compaction array stages. Obsolete
singleton lists, pruning codes and availability topology are released before
replacement compaction where their data are no longer needed. This array floor
does not cover factorization dictionaries or all construction scratch.

The new dispatch body is pinned to the preserved source excerpt in
`candidate-ols-prepared-parents-2026-09-16/estimate_ols_test.rs`; both its source
hash and production-body hash are tested independently. All previous source
SHA and F0 pins remain unchanged. Refreshing a manifest still cannot authorize
a routing mutation.

These changes close retained-parent omissions, not all action-memory scopes.
Compiler construction, factorization dictionaries, complete finite/inference
transient frames and opaque dependency/lazy pool storage still need full
byte-and-lifetime proofs. Coverage and the action ratio remain
undefined until those obligations are met.

## OLS request owners and slope recovery

V22 and action inventory v5 cover retained request/output-name admission and
saved-slope recovery. Request capacities include source names, expanded-term
components and operator strings, absorbed identities/labels/save names,
bindings, cluster identities and output registries. Prepared C names include
their NUL terminators. Independent deallocation tests include excess capacities.
The descriptor walk allocates nothing. This owner remains live during loading,
preparation, fitting, recovery and publication, so each applicable admission
adds it to the simultaneous parent. The public base estimates do not change.
Compiler copies used to construct this owner remain a separate obligation.

Saved-slope recovery admits retained coefficients, LSMR and MAP separately.
The slope whitening/transform allowance reuses the pinned owned-FE solver
formula. The LSMR envelope includes both its caller's certificate workspace and
the finishing workspace; requested channel vectors, residual/total vectors,
normalization state and nested descriptors are also charged. Sequential failed
attempts contribute a maximum. A tight budget can reject LSMR and still admit
certified MAP; insufficient MAP storage fails before recovery materialization.
Topology validation scratch is checked before validation allocates. PPML keeps
its existing parent/frame recovery admission and does not enter OLS's LSMR
recovery planner.

Exact-budget/one-byte-short tests exercise unit and unequal weights with one
and four workers and an independent known-span reconstruction. Retained-product
and full/partial output checks supplement the existing allocation-failure and
interruption fixtures. These component tests do not certify the remaining
construction, finite/inference or runtime scopes and do not define a whole-action
ratio.

## Numeric construction and transient-frame admission

OLS v23/action v6 include the scalar numeric-source construction envelope and
`OlsFrameMemory`. Numeric factorization growth, candidate/final topology,
pruning and repeated slope metadata are charged before preparation. Finite
and covariance frames include their simultaneously live reducers and request/
prepared parents. In particular, the dense cluster score bank and parallel
partials coexist; neither can stand in for the other in the estimate.

Independent allocation checks exercise dictionary growth and simple/slope
construction, conventional/robust/clustered inference, and fast/low frames.
Each VCE test uses its own allowance. Exact/one-byte-short budget tests and
existing failure/interruption controls remain. These fixtures use numeric
inputs and a primed worker team; they do not prove untested dependency/runtime
lifetimes or string dictionaries during SPI loading.

CMG's pinned memory APIs explicitly describe principal heap arrays. Their
reports omit hierarchy/workspace descriptor banks, graph Arc owners, some
spare capacity and the separately owned Rayon executor. Full dependency and
parent-overlap proofs remain mandatory; treating these reports as complete
would make an action ratio misleading. The detailed source-bound exclusions
are in the [construction/frame review](../benchmarks/reports/development/candidate-action-frames-2026-09-16/README.md).

## Reviewed OLS source transitions

V19 preserves the v18/F0 boundary pins and adds capacity observations for the
shared session's accepted finite fit, retained within metadata and frozen basis.
The observation sums simultaneous frozen-result and workspace owners. It does
not create a PPML session in any one-shot OLS path. Action inventory v2 adds the
scalar PPML metadata planner and publication-validator changes. Neither version
changes the structural-only coverage designation.

The PPML metadata planner charges the availability request's copied model,
component/operator strings, synthetic terms, bindings and bound effect identities
as preparation scratch. Candidate/compact interaction recipes, effect and cluster
labels/source identities, the boxed IRLS parent and final result metadata have
separate lifetime charges. Generation rebasing cannot shrink the frozen metadata
reservation, and prepared nested capacities are checked against that admission.
Partial FE saving retains the complete channel metadata bound. Plugin optional-cell
validation now uses index predicates instead of allocating index inventories.

Session capacity observations distinguish a frozen step, a staged strict
candidate plus rollback, and the committed canonical fit. The canonical solve
moves the residual out of reusable storage; strict commit releases the rollback
residual. There is no extra permanent N-vector charge. The additional session
row workspace costs K doubles. These observations extend the existing strict
overlap checks without changing numerical kernels or acceptance thresholds.

Remaining byte proofs are explicit: the OLS base heuristic still needs complete
construction and finite/inference transient frames. Retained OLS topology and
request owners have capacity observations; compiler construction and opaque
dependency/worker-pool growth remain unresolved. Neither those
observations nor the typed PPML metadata charges certify the remaining scopes.
A whole-action peak/ratio must remain undefined until every applicable scope
is covered.

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

## Action lifetime ledger and proof boundary

The private plugin ledger (`action_memory.rs`) joins OLS and PPML component
models using one rule: add a frame's simultaneous parents and workspace, then
take the maximum of complete sequential frames. Taking the maximum parent and
maximum workspace separately can manufacture a combination that never exists.
Each estimator has a closed set of required stages. An absent stage is unknown,
not zero. Unknown owner bits are unioned across attempts, even when an earlier
attempt's known component bytes are smaller than a later one. Overflow is an
allocation error and cannot partially update the ledger.

OLS requires metadata, loading, preparation, fit, inference, recovery and
publication. PPML additionally requires separation, iteration and strict final
solve. These are model records, not observations that execution reached a stage.
The OLS adapter combines the reconciled execution envelope with its accumulated
solver peak, including discarded attempts and additional inference projections.
The PPML adapter preserves the embedding parent in every existing phase model;
subtracting it is checked before splitting a phase into parent and workspace.
Neither adapter allocates observation-sized reporting state.

`FEREG_ACTION_MEMORY_AUDIT=1` emits a private version-1 diagnostic before the
first public store. It is not a COMPLETE receipt, a public protocol extension,
a memory-cap certificate or proof of successful publication. A failure after
this point still fails the transaction. The observation covers only the action
prefix through precommit; publication's modeled workspace appears in the ledger
but its observed allocation has not yet happened. Public base-memory fields keep
their original definitions. Missing coverage keeps the whole-action ratio
undefined; a source inventory match cannot clear a missing-coverage bit.

The following source-bound obligations define what may clear each bit. Generic
allocator samples can detect an underestimate but cannot prove these bounds.

| Bit | Owner family and location | Proof required before clearing |
| --- | --- | --- |
| 0 | Transport, decoder and compiler: plugin `input.rs`, `estimate.rs`, `ppml_estimate.rs`; core metadata/compiler | Admit simultaneous local-bundle, parsed strings, decoded tables, validation registries and compiler copies before construction; count nested capacities and error branches. |
| 1 | Fixed-width string loading: `OlsSourceLoader`, `SpiSession::read_string` | Bound all live source dictionaries, old/new hash tables during growth, per-cell SPI buffers, ordering descriptors and recodes, before allocating. Numeric-only input must be identified without reading observations twice. |
| 2 | Construction: preparation, factorization, finalized topology and pruning | Tie every capacity and overlap to a checked preflight; include interactions, slopes, singleton cascades and partially constructed/error owners. Existing numeric envelopes are only component bounds. |
| 3 | Numerical frames: prepared fast/low fit, finite fallbacks and covariance | Establish live parents for cold/warm, rank changes and failed finite attempts; include reducers, certificate work and scratch buffers. Scratch-file payload remains distinct from heap. |
| 4 | FE coefficient parents: `prepared_fit.rs`, inference projection callers | Account initial products across finite/residual solves, combination, and retained final coefficients across subsequent inference projections. Uncached score reabsorption separately charges the observed final product capacity. Initial and combined products are checked at ownership handoff against their reserved parent; construction inside the producing solver still needs its separate capacity proof. |
| 5 | Dependency containers: pinned CMG/within/Schwarz | Include outer vector capacities, Arc owners, maps, metadata, construction overlap and lazily grown workspace. Principal-array reports are insufficient. Source and dependency pins must bind the proof. |
| 6 | Worker runtime: fereg `parallel.rs`, dependency executors, Rayon | Bound pool construction, live/retiring pool overlap, retained queues and lazy allocation. A primed-pool sample cannot stand for cold construction or replacement. OS stacks/RSS are separate quantities. |
| 7 | PPML parent/generation: preparation, separation runner, retained IRLS session | Sum embedding, raw/prepared and current/old generation owners, strict candidate and rollback lifetimes, including failed recovery; preserve classification before observation-sized admission. |
| 8 | Saved-effect recovery | Include caller parents, all recovered channels even for partial save masks, normalization, certificate frames and every admitted fallback. |
| 9 | PPML publication | Reconcile final core result, shadows, validators, observation outputs and Stata transaction through COMPLETE; check failure/interruption after each publication boundary. |
| 10 | Solver attempts with unknown workspace | Supply a bound for every attempted route, including rejected setup and fallback. Never substitute zero for `SolverWorkspacePeak::UNMODELED`. |

A completed proof must include a capacity/lifetime derivation and focused
independent allocation tests with exact/one-byte-short budgets, failure,
interruption and repeated calls. A full action observation additionally needs
a terminal success/failure boundary. No current ledger record grants complete
whole-action coverage.

### PPML frame after core certification

OLS v25/action v8 add a separate admitted frame after the certified PPML result
returns. Its parent is `max(embedding reserve, surviving action capacity) +
certified result capacity`; its workspace is `8 * publication matrix cells +
sizeof(&[f64]) * declared FE channels`. The embedding reserve is deliberately
not reduced when raw inputs or core request copies have been released. This
can overreserve; it cannot silently reclaim an unproven runtime owner.

The certified-result inventory counts coefficient/status maps, sample and output
vectors, the prepared-row Arc payload and control block, nested saved channels,
inference products and subsets, FE/cluster identities, and separation records
with nested row/level witnesses. All counts use capacities. The descriptor walk
does not inspect observations or allocate a reporting bank. Shadows borrow
observation products and metadata; their owned matrices and saved-channel slice
descriptors are checked against the reservation before any store. Declared
channels count even when no output is requested for that channel. The core
preflight includes those descriptors as well.

Final validation compares the effective thread count resolved during action
setup. It does not query OS parallelism during publication: that query can
allocate on Linux. PPML final-sample construction also compares the already
ordered row map with retained entries of the drop ledger directly, avoiding a
second original-row boolean mask. Neither change relaxes an identity check.

This proof boundary starts **after core certification**. It does not cover the
core's result construction, PSD/Wald certification overlap, or dependency/pool
lifetimes. Those remain explicit obligations above; the publication gap bit
and whole-action ratio are not cleared by this narrower frame.

When the private audit is enabled, the ABI additionally emits a version-1
`FEREG_ACTION_MEMORY_TERMINAL` record after the action returns or unwinds, before
ABI error-report allocations are measured. The record includes the original
return code, start/current/peak bytes and peak delta. An action-start sequence
prevents a failure before estimator entry from borrowing a previous baseline.
The precommit model trace retains its original meaning. Terminal diagnostic I/O
is best effort, performs no Stata SPI calls, and cannot change a completed
transaction's return code. Neither record certifies complete model coverage.

### Retained FE products in fast fits

Let `D` be the ordered effect count and `L` the sum of level count times channel
count over effects. For `K` generated controls, the initial `K+1`-RHS coefficient
bank remains live through the residual projection and can coexist with the
combined one-RHS bank. Their parent reservation is
`8*L*(K+2) + 2*sizeof(usize)*(3*D+1)`. Empty structural-constant models use one
virtual level. Repeated effects retain their ordered coefficient axes; slopes
contribute their actual channel count. This scalar walk allocates nothing.

The parent is added to the execution budget before constructing the retained
solver session, so a session's captured budget also applies to the final
residual projection. The active solver's own result/workspace is charged by its
existing model. Its reported workspace is translated back to the original
execution base by adding the same parent once. Sequential initial/residual
reports still combine by maximum. The reservation is conservative while the
initial solver constructs its own result; it is not an exact allocation count.
This does not alter floating-point operations, solver tolerances or sample rules.
Low execution and calls that request no retained coefficient product do not
enter this added fast-fit reservation. Uncached robust/clustered score reconstruction charges the actual retained final
product capacity before building its next solver, and translates that solver
peak to the original execution base once. Cached and retained-scratch paths
keep their existing admission. Session/dependency obligations in the table
remain separate.

### Core PPML certification temporaries

OLS v26/action v9 retain explicit allowances for the independent core
certifier's original-row generation mask, candidate-row separation replay and
reporting-slot/saved-channel masks. The first two use the immutable original
and candidate axes even when the final retained sample is small. The slot and
saved-channel masks coexist; their bytes are added. These validators run
sequentially with PSD/Wald replay, so their workspace contribution is a maximum.
The method-attribution mask is released before hash replay starts its own active
and reason vectors.

Once the result input owns every certification product, the caller releases
the obsolete final Hessian/fit, prepared generation, original separation reasons
and mapping/view temporaries. Their lifetimes no longer extend through the
independent certifier. The checked preflight retains its conservative earlier
parent reserve; this lifetime reduction does not silently reclaim an unproved
owner or turn the whole-action coverage bitset into a complete proof. Base-field
definitions and wire layout remain unchanged; affected computed publication
allowances include the previously omitted validator storage.

### Input dictionaries and metadata admission

OLS v27/action v10 bind the input-memory transition separately from the preserved
OLS preparation boundary. The new `input_memory.rs` owns explicit entry and
bucket vectors for fixed-width string factorization. Before growing either bank,
it checks retained arrays, all other source dictionaries, both old banks, complete
new banks, and the incoming string buffer together. Rehashing reuses stored
hashes. Buckets are released before the entries are sorted in place; the recode
vector then coexists with entry descriptors and key capacities. Byte ordering,
empty-string missingness, candidate masks, row ordering and main-thread SPI
access remain unchanged. Production SPI reads reserve one admitted buffer and
return it directly. No observation-sized reporting array is added.

`metadata_memory.rs` checks typed decoder-table and compiler reservations before
reading tables. Table descriptors include sentinel rows. The model includes four
request/decoded owners, compiled columns and component recipes, nested string
capacities and bounded validation registries. ABI argument descriptors/payloads
are admitted before ownership; transport framing/registry copies receive a
separate checked reservation. Registry cardinalities and cluster recipe arities
must agree with actual payload/table sizes before they become allocation counts.
OLS and PPML apply the same existing headroom formula. The expanded local bundle
is released after parsing; the original borrowed argument capacity is carried
through the subsequent admission parents. The OLS base fields retain their
original definitions; PPML adds that previously excluded parent to its embedding
reservation.

Focused tests check an independent sorted string oracle, simultaneous dictionary
growth across capacity boundaries, repeated-call allocation peaks, exact and
one-byte-short budgets, malformed metadata, interruption and absence of public
stores. They supplement, rather than replace, the existing allocation-failure
and publication-rollback fixtures. See the source-bound
[input-memory report](../benchmarks/reports/development/candidate-input-memory-2026-09-16/README.md).

The production loading ledger no longer reports an unknown string dictionary;
its actual capacity model is retained as a phase maximum. Transport/compiler
coverage remains explicitly incomplete: this component admission does not yet
prove the entire ABI/error-report and runtime parent lifetime. Construction,
solver/dependency containers, numerical/generation overlaps and worker retirement
also remain separate obligations. No whole-action peak or ratio is certified by
this transition.

### CMG supplemental container admission

OLS v28/action v11 add a checked capacity supplement to the pinned CMG principal
array estimate, before ordinary and prepared hierarchy construction. It covers
hierarchy, centering-plan, CSR-plan and workspace descriptor banks, shared-owner
headers, component/permutation spare capacity and a terminal LDL construction
overlap. The latter constructs its packed/sparse factor while both dense input
matrices are still live. Scalar preflight dimensions use the reviewed default
fill/depth limits; retained observations use the actual hierarchy and current
lazy workspace count. Growth counts old and replacement banks together.

The local retained helper is named a bound, not an exact heap report. This
supplement does not establish all hierarchy-construction temporaries or Rayon
runtime coverage. The pinned CMG executor drops its pool asynchronously;
retiring workers can still coexist with a subsequent action. The source remains
pinned, and a separate joined-shutdown draft must be reviewed before any upstream
publication or pin change. No unknown action-coverage bit is cleared by this
component model.

### Worker retirement boundary

OLS v29/action v12 bind the native cached-team owner to explicit thread handles.
Replacement creates the next pool, releases the cache mutex, and drops the old
cache owner. Its last owner signals Rayon termination before joining workers,
including on partial construction failure. The old and new pools can coexist
until that join; their runtime storage remains a separate coverage obligation.
Scoped Stata work releases its last team owner on the calling thread. The public
Rust API also permits external asynchronous work; last-owner drop on one of the
pool's own workers detaches handles to avoid joining itself or waiting peers.
That escape does not establish a complete memory bound for external async users.

These lifetime changes are not a byte model for Rayon/crossbeam queues, TLS or
remaining dependency owners. CMG's separate pool still uses the published pin;
the tested joined-shutdown dependency draft is intentionally unintegrated. The
whole-action ratio remains undefined.

### PPML transferred and prepared generation capacities

Action inventory v13 binds a complete retained-generation capacity observation
and a separate scalar bound. The bound includes the numeric column descriptor
bank, finite/cluster-only categorical codes and level tables, compacted raw
sources, registry and recipe banks, partition keys, membership offsets, both
row maps, original-axis drop reasons, compact offset/prior weights, and the
original reported coefficient-slot capacities after rebasing. Shared immutable
row/drop-reason Arcs remain charged once at their parent, outside the generation.

The common parent adds only the excess above its existing generation allowance;
a replacement adds the same excess while the old generation survives. Each
constructed generation must fit its bound before it can enter the coordinator
or replace the committed generation. This validates capacities as well as logical
lengths. It does not substitute a post-allocation check for construction admission:
the scalar bound is included in the initial preflight before row materialization.
Temporary factorization/build work remains a distinct construction obligation.

The transferred request/input owner is counted from actual nested capacities
before admission, without reading observations or allocating reporting arrays.
Only its excess above the embedding-resident observation is added, so plugin
callers do not pay for the same owner twice and core-only callers do not omit it.
Initial coordinator/generation construction now has an explicit preparation
allocation/timing window, closed on both success and ordinary errors. Subsequent
rebuilds remain attributed to the separation method that requested them.

This closes the named raw/prepared owner omissions. Bit 7 remains unresolved
because its original definition also includes retained IRLS strict/rollback
frames; it is not renamed or cleared to imply those distinct lifetimes passed.
Construction, runtime/dependency and whole-action exclusions remain explicit.

The installed strict-checkpoint metadata bound also includes the specialized
solver caller's active-column identity copy. It survives candidate installation
and rollback through engine refresh. This is a fifth K-sized index bank beside
the candidate fit, candidate basis, candidate active positions and rollback
positions; it cannot borrow the mutually exclusive earlier assembly reserve.
Slot-heavy models exercise the installed branch of this maximum. The scalar
admission change leaves the strict solve and its acceptance policy unchanged.
