# Current architecture

There is one computational runtime for OLS and PPML. Stata owns syntax, metadata
and the transaction; Rust owns loading, preparation, absorption, estimation and
inference. [SPEC](SPEC.md) defines the scientific contract.

| Owner | Responsibility |
|---|---|
| `stata/` | Public ado, parsing, result validation, publication and rollback |
| `plugin/` | Small C bridge to Stata SPI |
| `crates/fereg-plugin` | Main-thread SPI loading, FFI panic boundary and publication |
| `crates/fereg-core` | Safe numerical core, memory admission and worker policies |
| `crates/fereg-oracle` | Independent dense scientific oracles; development only |
| `crates/fereg-testkit` | Native fixtures and independent regression support |

OLS result reporting preserves the two phases and the existing 2x16 solver
matrix. The shared two-FE recurrence distinguishes OLS from PPML when it emits
method diagnostics: OLS reports CG or actual CMG-Schur application, and MAP
continuation sets fallback. PPML retains its checkpoint-engine identities.
Low-memory aggregation preserves an applied CMG method across blocks unless
a certified fallback takes precedence. Stata adds readable method macros and
prints them with the ordinary results; no trace parsing is involved.

## Preparation and storage

Canonical preparation owns the immutable sample/topology and optional cluster
metadata. Consumers borrow this owner; original factor slots and base flags
survive final-sample changes. No duplicate legacy preparation pipeline is installed.
`memory::Capacity` and `fe::workspace_capacity` share checked shapes; actual
capacities remain authoritative. `action_memory::Composition` charges simultaneous
lifetimes once while keeping admission, expected heap and coverage distinct.

Dense, scratch and implicit storage retain mathematically appropriate kernels.
The Low path keeps one row-major scratch file through fit and inference. Scratch
read failures revoke cached pages; mutation failures poison the affected owner.
Public memory choices, fallible allocation and transactional failure are preserved.

The unrestricted numeric loader can complete categorical sources first and
factorize their Rust-owned columns while the caller reads remaining columns.
The optional `PreparedOlsLoadHint` contains encodings only. Authoritative sample
classification still precedes adoption, which requires the full original row
axis, identical source identities and matching canonical decoded values.
Singleton pruning, topology, DoF, clusters and original factor/base metadata
remain products of ordinary preparation. Missingness or an unproved identity
discards the hint. Packed-RHS source conflicts, strings and insufficient
admission use the existing loader. Both scopes join on errors or unwind; the
caller polls Break while waiting, though a running factorization can finish its
current whole-column call before observing cancellation.

PPML carries the hint outside `LoadedPpmlInput` in a per-call adapter. It charges
the surviving cache through adjustment/bias reads, then excludes its exact owned
capacities from the permanent embedding snapshot. A failed subtraction drops
the hint and repeats the original snapshot. Before the first memory decision,
full-row classification and a checked bound admit cache-plus-adoption descriptors
only within the existing, not-yet-live generation topology row bank. Generation
zero takes it once, including failed attempts; adoption moves encodings before
topology allocation. Later separation generations rebuild normally. No cache
becomes sample, Hessian, basis or certificate authority, and memory policy does
not change to accommodate the optimization.

## Finite operations and accepted results

`ols/finite_operations.rs` owns guarded factor/solve/inverse and explicit rank
policies. `ols/finite_wls_kernel` owns dense, scratch and implicit moments over
immutable projected rows. `ols/prepared_fit/one_shot.rs` owns the Fast/Low fit
lifecycle. OLS has a temporary finite workspace; PPML lends retained banks.
OLS does not create a PPML session.

The plugin selects the qualified ownership/deferred-cell policy after decoding
an ordinary two-FE categorical-intercept OLS Auto/Auto request with a global
intercept. Both omitted-moment and projected-score ownership controls are set
there; core defaults stay off. Explicit injection fields bypass this public
selection even through the framed or local-bundle transport. Marker-only Break
observes the public request without selecting an experimental preset.

`ols/prepared_products.rs` owns accepted fit statistics, canonical absorbed DoF
and distinct covariance/certificate payloads. Robust/clustered inference consumes
fit-stage projected score products without rebuilding an FE solver or reabsorbing
the design. Independent score-Gram, bread, covariance, PSD and Wald witnesses
remain separate from production accumulation.

`ols/linear_frame.rs` owns retained coefficients, finite bread, residuals and
optional decomposition. Canonical and frozen proofs remain distinct. Strict PPML
transactions retain accepted/speculative frames and restore accepted authority
without allocation. PPML outputs have one requested-product constructor, one xb
representation and one immutable final-row ledger shared with the sample.
Topology, weight, response and basis identities remain explicit dependencies.

Automatic two-FE PPML may retain separately owned historical canonical geometry
inside its existing persistent Fast-products reservation. Only strict checkpoints
consume it as a fixed preconditioner for the exact current-weight implicit
operator; ordinary frames and accepted-mean Hessian refresh retain their normal
solves. `ols/canonical_products/historical_preconditioner.rs` owns compatibility,
transported inverse actions, capacity charges and the per-component/original-RHS
work ledger. The session publishes producers only after enclosing-frame
acceptance and retires them on incompatibility, rejection or inference handoff.
Speculation uses at most half the original iteration cap, with one cold retry
using each RHS's remainder. Independent current-problem certification and all
strict tolerances stay unchanged. Optional allocation misses remain cold; fatal
errors and Break use the existing rollback. Low memory, saved-FE and explicit
solver requests do not retain this cache.

PPML Auto with exactly two intercept FEs also reuses one CMG Schur hierarchy
across frames (`fe/borrowed_map/ppml_cmg.rs`). The categorical owner's two-FE
graph holds it with its build generation and at most three construction
attempts. Joint ordinary frames reach it through the retained-block hook of
`fe/slope_coefficients/joint.rs`: a recurrence that reaches step 64 may build a
missing hierarchy or rebuild a stale one, and an installed hierarchy `B`
applies as `D^1/2 B D^1/2` with current level masses `D`. Canonical products
take the owner mutably: a call first refreshes a stale hierarchy, and
`ols/canonical_products/shared_cmg.rs` builds a missing one when a 32-step
probe of the first column does not finish. It then preconditions large
implicit components with one fixed cycle per application through
`PpmlCmgView` on per-worker scratch charged to the products' allowance. Categorical
reprojections use the h16/k1 route; a hierarchy from an earlier weight
generation is set aside, charged as resident to the rebuild, and reinstated
unless replaced. Admission reserves only the application banks; each
construction admits itself against the fit's solver budget, and refusal
leaves the diagonal recurrence untouched. `canonical_products/reduced.rs`
holds the reduced recurrences, including the dense solve of small implicit
components. `fe/exact_sections.rs` holds the exact parallel forms of the joint
recurrence's helper merge and finite cross block and of the per-frame level
masses: fixed chunks, each element in its serial operation order, so bits do
not depend on the team. Developer phase records (`FEREG_PHASE_TIMING`) of joint
frames, canonical products, shared-CMG columns and PPML absorptions come from
the trace-only ledger in `diagnostics/phase_ledger.rs`. Joint frames build weighted columns and their norms and run
original-column acceptance scans in groups of `ceil(columns / workers)`
columns per task, at most four, so one row scan serves several columns
without leaving workers idle. The bank keeps the projected weighted columns
unnormalized; their norms scale only the small products. One tiled pass
(`kernels::gram_panel`) over the response and the bank forms the Gram and
the initial finite scores in 128 fixed row leaves of an admitted partial
bank, merged in leaf order; wider banks keep the pairwise
`kernels::normalized_products`. Acceptance scans
and response checks (`ols/joint_checks.rs`) reduce fixed row leaves over
`parallel::fixed_tree_reduce`, whose tree depends only on the row count. With
the projected bank `Z = P X`, reconstruction forms `P(y - D a) - Z (b/s)` and
the finite scores `Z'r / s` in parallel instead of rescanning row-major
representatives.

## Host and execution

`model_metadata.rs` owns shared model decoding and wire construction.
`input.rs` delegates numerical loading to the common admitted preparation loader.
Each estimator has one named matrix schema for layout, capacity, precommit and
publication. `publication.rs` supplies ordered scalar/matrix/output publication;
completion is written last. Workers only touch Rust-owned memory. FFI catches
panics; SPI stays on Stata's main thread.

`kernels.rs` holds hot `mul_add` loops and the borrowed-MAP group-mean passes.
Baseline x86-64 code calls the library
`fma` per element, so the plugin's `fused_kernels.rs` installs FMA-compiled
instantiations of the same bodies when it detects FMA and AVX at runtime;
otherwise callers run the portable bodies inline. FMA is exactly rounded, so
results match wherever the platform `fma` is exactly rounded.
Processor features may select different arithmetic under SPEC's independent
scientific checks; both the portable and accelerated paths remain tested.

The general three-FE LSMR gather, elimination, sparse products and Schwarz setup
live in the pinned `within`/`schwarz-precond` dependency. Changes there obey the
same scientific and performance evidence rules and enter through a pinned
revision bump.

Automatic unit-weight, symmetric three-FE OLS may first attempt at most four
MAP sweeps when complete, outward-rounded pair-incidence geometry predicts
convergence at a hundredth of the request. Exact integer marginal bounds
can prove a saturated pair without scanning its contingency cells; otherwise
moments accumulate once per occupied cell, with outer levels split across
workers into contiguous membership-balanced ranges whose integer partials merge
in lane order. Geometry chooses only additional certificate checkpoints: the
predicted sweep and the one before it, because the bound is conservative. The
unchanged requested certificate alone accepts; initial, configured-cadence and
fourth-sweep checks remain. A pilot that certifies after sweeping hands the
finite solve its column norms, computed by the dense LSMR norm kernel. Failed
attempts drop their candidate/norm/coefficient storage before LSMR starts on
untouched inputs, debiting actual work and preserving memory admission. The
pilot is skipped once a binding holds a prepared LSMR solver. Explicit solvers,
weighted requests and PPML retain their routes. See
[validation](VALIDATION.md) for the policy status and performance limits.

The public many-FE preset retains a Jacobi LSMR solver first. Each RHS runs
under an `EscalationPolicy` that projects its remaining iterations from recent
contraction; escalated sessions finish, the trial solver is released, the
additive solver is built once and seeded sessions continue each escalated RHS.
Window sessions start and finish in parallel; results keep RHS order. A
certificate miss on this preset reuses the retained solver for one seeded
correction before the MAP fallback.
[General routing](../crates/fereg-core/src/fe/borrowed_map/routing_lsmr.rs)
owns the trial and its escalation.

Thread counts are worker budgets. Shared planned lanes retain deterministic
arithmetic leaves and ordered merges. One-FE, two-FE and general-FE algorithms
remain distinct where mathematics warrants them; benchmark-specific route fences
are prohibited. Explicit LSMR/CMG and both bias-correction modes remain supported.

### CMG host-team and stage admission

CMG never creates a pool inside fereg. The bounded CMG builder enters upstream
operations through `Work::cmg` and `parallel::with_cmg_execution`, which lends
the actual installed fereg registry as a borrowed `cmg::Team` after verifying a
live action lease; a foreign Rayon pool or a caller outside fereg's registry
gets the serial path. Explicit `cmg`, `cmgschur` and PPML `cmg` requests borrow
the same action team through `parallel::with_cmg_team`, which requires the
team's width to equal the requested thread count (outside any native action,
for standalone tools and tests, it installs fereg's own process-local team of
that width). A team is a copyable borrow: it creates no pool, retains no owner
and is never stored in a hierarchy, plan, solver or workspace; CMG's solver and
plan record only the team's shape and reject a differently shaped team. CMG's
standalone owned `ParallelExecutor` is not used by fereg.

CMG solves and applications supplied identical inputs, hierarchy, options and
fixed team thresholds produce identical bits across worker ceilings and solver
modes (milestone 3d). Explicit `cmg` and `cmgschur` carry the team: PCG
dot products, norm sums and single-component centering use CMG's fixed-chunk
tree whenever a length reaches 131,072 entries (fereg's default thresholds).
That includes a one-worker team. Serial and concurrent right-hand-side solves
run on `Team::single_lane()`, a one-lane view of the same pool. This CMG contract
does not make complete fereg estimates bit-identical across workers: surrounding
fereg arithmetic can supply different buffers, as the explicit CMGSchur and
PPML replays demonstrate. Scientific agreement and stable discrete decisions
remain required. Which hierarchy levels carry row
operators still follows the worker count; that is a memory and speed choice,
since row operators reproduce the edge-list products bit for bit. The OLS
automatic route borrows whichever host team is installed for its cycle
(`with_cmg_execution`), because its plan records no team width; PPML reuse
and reduced pairs keep a team-free scalar cycle with serial arithmetic.

CMG construction, application and PCG receive one context each
(`BuildContext`, `ApplyContext`, `SolveContext`): the borrowed team, the
host's cancellation query and, for construction, the memory budget. fereg's
CMG routes pass `interrupt::cancellation_requested` to graph construction
(including PPML topology preparation and numeric assembly), hierarchies,
plans, PCG workspace banks, cycles and solves. CMG counts long traversals (a
CSR row, a terminal-factor row, a reduction leaf, a team sort partition) in
segments while they run; serial meters poll every 65,536 counted units and
each parallel lane polls the same query at that cadence inside its rows and
leaves. All lanes join and the coordinator polls before `Cancelled` is
returned, which fereg maps to an interrupted estimation with atomic rollback;
a cancellation is never retained as a CMG setup or projection failure and
never selects a CG fallback. Only a numerical breakdown or failed
certification (`NonConvergence`), a resource refusal (`Allocation`) or
nonfinite numerical data (`InvalidData`) selects the explicit route's
certified CG fallback; internal failures (including CMG's runtime and
hierarchy-invariant errors), caller-contract violations and unsupported
requests fail the estimation atomically. A reduction lane that observes
cancellation stops its own and every other lane's remaining leaves, and batch
rows wider than 1024 lanes are counted in lane tiles. fereg's own traversals on
these routes (endpoint and weight-ratio rows, seed and input finiteness scans,
RHS accumulation, nuisance moments and corrections, residual updates and
bank fills) poll the host flag every 1,024 rows or 65,536 entries and once
after each fill. Every CMG stage runs on at most the team's worker ceiling of
lanes. Allocator, operating-system and join latency are not bounded. CMG's public
`ControlledStage` validates fixed logical chunk geometry on one coordinator
before dispatch; chunk boundaries depend only on the slice length and chunk
length, never on the number of workers. The caller has
already admitted and allocated output and any scratch; dispatch uses disjoint
slices and bounded recursive lane splits, with no heap banks or atomic
work-quota race. Kernels must do bounded work per chunk, avoid
allocation/nested parallelism and leave outputs private on failure. Workers
poll the sticky cancellation query before every chunk, and the coordinator
polls again after joining. All dispatched tasks join; the earliest logical
kernel error wins, and cancellation dominates after joining. Panic unwinding
also joins. The host admits its runtime and stacks separately.

Serial construction steps count conservative primitive units and poll
cancellation whenever the count crosses a fixed cadence (65,536 units in both
fereg's bounded builder and CMG) and before every allocation, carried across
row, group, pivot and level boundaries. Work between polls is bounded; allocator,
operating-system and join latency is not. There is no construction-work ledger,
allowance, host-context charge or budget-dependent factor screen. A memory
refusal before installation resumes the incumbent; cancellation and fatal errors
propagate as an interrupted or failed estimation with atomic rollback.

Parallel stages partition by fixed item counts independent of worker count,
admit transient banks before dispatch and fall back to the arithmetic-identical
serial path before any numerical write when admission declines. They cannot
retry a partially executed or failed numerical stage. No controller decisions,
native results or qualification gates are cached.

Milestone 2 uses this interface for the initial graph. The synchronous
incremental-two construction borrows the full admitted team. At 65,536 raw
edges and above, fereg can emit independent edge arithmetic into its admitted
output bank. Canonical cell aggregation and compensated suffix sums stay serial
and retain their numerical order. A sampled cache adds eight bytes per
aggregated cell and row for suffix storage; optional scratch is admitted before
reordering or dispatch. Indexed SplitMix draws reproduce the existing sequential
stream. Logical edge chunks depend on dimensions and the per-edge kernel bound
(at most 1024 primitive units per chunk), not the number of lanes. Output slots
temporarily carry task metadata, avoiding a separate task vector or scratch
proportional to lanes times the largest row.

Milestone 3c moves the remaining graph-construction passes onto the same
borrowed team without changing any bit. With the canonical cell cache (the
automatic route) and at least 65,536 observations, the census groups
consecutive workers into tasks of at least 4,096 raw cells (a longer row is
its own task). Each task fills, sorts and compensates its rows in its own
range of the admitted cache bank. Lane counters poll every 65,536 units,
including inside one long row. The rows are then compacted in worker order
into the serial census's exact layout, and the earliest failing worker's
error is reported. Sampled-row suffixes are prepared by the same kind of
task. Edge emission locates each edge's worker from prefix offsets instead
of a serial placeholder pass. It checks the next offset, or bisects the
remaining offsets across a run of edge-less workers, so a lookup takes at
most one probe plus the bit length of the worker count. These steps are
part of the declared per-edge work. Only the small task tables are new
scratch; they are admitted, and the serial stage runs when they do not fit.
With a team, the automatic upgrade builds its hierarchy on that team, which
contracts large levels and selects heavy forest edges in row-owned lanes,
whenever the parallel construction bound is admitted. Otherwise it builds
serially under the serial bound. The parallel bound's extra contraction slot
bank is transient. It counts toward the admitted construction peak only, and
the installed hierarchy keeps the serial bound as its retained charge. Graphs, labels, hierarchies, factors,
repeats and cycle bits are identical either way and at every worker count.
Explicit `cmgschur` keeps its uncached two-pass census and therefore its
serial census and emission. Its hierarchy already used the team.

The upstream initial-graph constructor maps edges and performs bounded merges
with the same fixed threshold for one or many lanes. Runs of at most eight
edges and bounded merge packets keep each kernel within 1–1024 primitive units
between cancellation polls. Two edge banks, the diagonal and three Arc layouts
are jointly admitted; the spare bank is dropped before serial duplicate
reduction and diagonal accumulation in canonical order. Successful graph bytes
and downstream arithmetic are identical to the serial constructor. Hierarchy
construction, numerical application and strict polishing are unchanged.

The separate explicit-CMG polishing correction preserves the FE-range invariant
of its semidefinite observation-space CG operator through three internal FE
coefficient banks. Residual/direction cancellation occurs in those banks before
compensated observation evaluation, including the initial and subsequent
operator products. The existing optional output bank remains separate. Joint
admission prices all live banks; the reconstruction loop uses the ordinary
fixed row Break checkpoints. This changes strict polishing arithmetic and cost,
not the shared-executor foundation or its construction policy. See the
[numerical contract](SPEC.md) and [validation evidence](VALIDATION.md).

Experimental OLS `cmgschur` reuses the two-FE implicit Schur operator and its
observation-space reconstruction/certificate. It shares the automatic route's
one Schur graph adapter (`fe/cmg_schur/bounded.rs`: census, clique/tree edge
emission, initial graph and hierarchy pipeline, all polling cancellation);
the two differ only in installation. The explicit request contracts large
levels and builds row operators for the eligible levels of its host team, and
keeps up to four interleaved lanes; the automatic upgrade builds on the team
when the parallel bound is admitted (serially otherwise, with identical bits).
For OLS it also installs the row-operator plan and up to four interleaved
lanes, each only when its bound is admitted after the one-lane apply storage
(`fe/cmg_schur/bounded/install.rs`), and otherwise applies the same hierarchy
as one team-free scalar cycle per lane, as PPML reuse always does. With
default hierarchy options, graphs, hierarchies and cycle bits equal the
pre-consolidation explicit builder's (a frozen-digest witness covers one and
four workers); the two-FE Schur routes themselves build with a zero
low-effective-degree threshold (see SPEC).

After the CMG handoff, OLS Auto applies the Schur operator through the
deferred (worker, firm) cells (`fe/routing_kernel.rs`). The kernel keeps a
firm-major copy of its worker-major cells, built by a stable count so each
firm's cells stay in increasing worker order. Workers own their left-scratch
rows and firms their output rows (`routing_kernel/parallel.rs`), so the
parallel product reproduces the serial bits at every lane count. Low-degree levels
of the larger FE are eliminated into weighted pairs on the smaller FE; higher
degrees remain auxiliary graph vertices. Degree is counted after repeated
worker-firm rows are collapsed. A fixed CMG cycle receives component-centered
firm residuals and zero auxiliary loads. Restriction and firm-component centering
produce the outer preconditioned residual, used in every PCG energy/direction
update. The operator bank is reused for this residual after its Schur product
is consumed. The session validates the explicit request eagerly but builds the
hierarchy only after an initial requested-tolerance certificate fails. It admits
the construction alongside the retained owner and any earlier norm handoff,
then reuses that hierarchy across projection blocks. Active columns are packed
into bounded vertex-major tiles for fixed-cycle matrix and transfer traversals;
converged columns do not consume cycles. A projection with one original RHS uses
an admitted scalar workspace, including when it follows a wider projection in
the same session. This choice stays fixed throughout the projection; compacting
a multi-RHS projection to one active column does not switch its cycle path.
Component summation is fused with
gathering, and output centering with scattering. Direction updates use the
ordinary parallel stage policy. A half-tolerance proposal is certified first;
one stricter residual correction is allowed within the same iteration budget.
With `cmgsamples(k)` positive, above-cutoff stars instead become the average
of `k` weighted trees on their distinct firm neighbors. The low-degree cliques
remain exact and no high-degree auxiliary vertices are retained. A reusable,
admitted sampler orders cells by positive weight and firm ID, forms compensated
suffix sums, and draws one later neighbor for each nonterminal neighbor. Each
tree connects all of that worker's firms. Fixed SplitMix64 streams keyed by the
request seed, worker and tree make construction deterministic; the resulting
graph and CMG hierarchy remain fixed throughout every projection and correction.
Sampling affects only the preconditioner, never the exact Schur operator or
observation-space certificate. Zero samples retains the original hybrid graph.
The sampling count, seed, edge census and scratch are validated/admitted before
emission; edge-weight underflow is an explicit error rather than a lost link.
This small bounded sampler is owned here because the pinned upstream
clique-tree API does not expose fallible, budget-admitted scratch storage.
An explicit request has no MAP fallback, does not reuse historical
PPML weights and never changes the automatic route. The current support envelope
and numerical scope are in [SPEC](SPEC.md#solver-support-and-dispatch).

### Fixed-leaf norms and outer scores

Reusable MAP norms use 512-row leaves and merge their scaled sum-of-squares
summaries in ascending leaf order, including with one worker or no available
scratch. RHS tiles are bounded at 64 and waves at 128 leaves; scratch capacity
changes scheduling only. Categorical MAP lends its idle dot/group-statistic bank,
and slope MAP lends its group-column bank, distinct from cached certificate
geometry. Every later consumer rebuilds these banks before reading them. The
fallback uses bounded stack storage; no additional capacity or memory route is
required. A nonzero leaf keeps the maximum computed weighted element as its
scale and a sum of at least one. Merging retains the larger scale and its
nonnegative contribution, so the final norm still dominates every computed
weighted element. The initial rejection screen's proof below remains applicable.

The independent PPML outer score uses the same fixed row leaves for its scale
and finite-column moments. Each leaf retains its compensated sum and correction;
both components enter the ordered compensated merge. Rounding a leaf to a single
total before merging is insufficient under cancellation. The two existing FE
score banks hold partials before their unconditional clearing and unchanged
FE-group accumulation. Their capacity controls tiles/waves, never the arithmetic
tree; serial work avoids submitting to the team. Scores still read original
outcome, mean, prior weights and original/generated controls. No projected design
or cached Newton score substitutes for the independent certificate. Single-leaf
inputs retain their former totals. Strict PPML solving and final-Hessian checks
remain separate and fresh.

### Initial FE rejection screen

The reusable categorical MAP owner may reject its zero-iteration shortcut from
one exact group, but that rejection grants no certificate. Every successful
shortcut still runs the complete independent certificate, as does every final
solver acceptance. A partial `FeCertificate` is never published.

Skipping the initial gather must also preserve its errors. Both domains require
immutable validated nonempty memberships, checked maximum-level and total-moment
dimensions, and norms freshly computed from the unchanged initial matrix.
Every nonzero norm must lie in `[1e-20, 1e20]`; these are conservative
finite-arithmetic bounds, not convergence tolerances. Genuine unit weights may
include zero norms. Supplied weights require every norm to be positive, at most
`2^32` observations, raw scale in `[2^-32, 2^32]`, and every computed normalized
weight in `[2^-32, 1]`. One raw-minimum scan and a division by the fixed positive
scale establish the last bound by monotonicity, without allocating a bank.
Every unproved case uses the original complete certificate.

The finite-error proof uses binary64 round-to-nearest, ties-to-even. For a
power-of-two bound `A`, rounded accumulation from zero of terms with magnitude
at most `A` cannot cross `2^54 A`: spacing at that boundary is `4 A`, and spacing
immediately below it is `2 A`. Thus the bound does not require a small `n epsilon`.
For Neumaier accumulation the primary sum is bounded by `2^54 A`; each correction
increment is bounded by four times that, and its accumulated correction by
`2^110 A`. These bounds include cancellation and subnormal rounding.

The fresh unit-weight scaled norm is at least the column maximum, and zero norm
implies an all-zero column. Hence inputs are smaller than `2^67`, shifted inputs
than `2^68`, shifted primary sums than `2^122`, and corrections than `2^178`.
Including final addition and division, one-pass means are bounded by `2^180`.
The scaled fallback accumulates inputs of magnitude at most one, so its final
means are bounded by `2^178`. Unit group weights remain positive and finite;
their square roots are at most `2^32` for supported 64-bit observation counts.
Positive norm denominators exceed `2^-67`. Relative moments are therefore
bounded by `2^279`, their exact squares by `2^558`, and the entire ordered FMA
square accumulation by `2^612`, below binary64 overflow. Zero columns have zero
means, so the zero-denominator error is impossible. Validated structure and
checked moment counts discharge the remaining certificate errors.

For the weighted domain, let `u = 2^-53`, let `m <= 2^32` be a nonempty group's
size, and let `W` be the exact sum of its normalized weights. Since `m u <= 2^-21`,
the positive primary accumulation stays in `[W/2, 2 W]`. In each Neumaier update,
the larger operand and rounded sum satisfy Sterbenz's condition; the computed
correction increment has magnitude at most `8 u W`. The accumulated correction,
including its own rounding, is bounded by `2^-17 W`. These normal weights and
their intermediate sums lie on a grid no finer than `2^-84`, so underflow does
not invalidate this argument. The computed group weight therefore lies in
`[W/4, 4 W]`, hence in `[2^-34, 2^34]`.

Fresh weighted norms dominate every computed `sqrt(q) * x`. Since `sqrt(q)` is
at least `2^-16`, raw input magnitudes are bounded by `2^83`. The saturation
lemma then bounds shifted weighted terms by `2^84`, their primary sums by
`2^138`, and corrections by `2^194`. Including division by the group weight,
one-pass means are bounded by `2^230` and scaled-fallback means by `2^228`.
Relative certificate moments are bounded by `2^314`, their exact squares by
`2^628`, and the entire ordered FMA square accumulation by `2^682`. Raw absolute
moments are bounded by `2^296`. All power-of-two upper bounds are inclusive and
remain below overflow; positive norm denominators remove the zero-scale error.

The weighted zero-norm exclusion is necessary even in this weight window.
With singleton groups, weights `[1, 0.25]` and a column `[0, 2^-1074]`, the
weighted norm is zero, while the second group's scaled mean is `2^-1074`.
The original certificate must retain its nonzero-moment/zero-scale error.
Tiny values in a column with a positive moderate global norm remain supported,
including their whole-group scaled fallback. No tolerance or acceptance rule
changes in either domain. Solver tracing records successful screen rejections
and whether they used weighted, parallel or wide arithmetic; normal runs add
no screen output or clock.

The screen runs inside the existing pilot-scratch loan and follows the exact
route that the full certificate would use: one-pass means with whole-group
fallback for serial/narrow work, scaled tiles for wide parallel work. It computes
every RHS of the first group, because a tiny RHS can trigger fallback for all
RHSs, and folds it with the unchanged certificate arithmetic. Residual norms,
RHS scales and per-RHS maxima are initialized as before. Downstream solvers
initialize their required scratch; retries receive fresh complete certificates.

## Repository boundaries

One plugin per platform serves both public commands. Solver-route diagnostics
report the method and certified fallback. The local benchmark is descriptive.
Tests, pinned independent references and the complexity ratchet remain maintained.
Builds go under ignored `target/` and task captures under `target/tmp/`, which is
deleted after use; only one current local plugin is retained in `dist/`.
