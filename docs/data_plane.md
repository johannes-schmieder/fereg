# Native data plane

`prepare_numeric_sample` is the first authoritative, Stata-independent numeric
sample boundary. It accepts borrowed source views, an `if`/`in` candidate
marker, and an explicit estimator/reference domain policy. It returns the
initial compact numeric columns and an original-length reason mask. The
developer OLS boundary then performs supported FE singleton pruning, a second
aligned compaction when rows are removed, and final categorical
re-factorization and constructs a preparation-only structural FE topology. This
is not yet the complete
Phase 2 data plane: string factorization, cluster encoding, separation, and
estimator SPI dispatch remain outside this slice. A bounded numeric
fixed-effect source preparation path is included, but no owned solver
`FeDesign` or persistent solver state is constructed. A narrow borrowed
intercept-only design view references the final prepared codes and compressed
membership without copying them. An opt-in developer check now constructs the
exact prepared Y/X absorption batch and consumes it through the separate strict
MAP path before any diagnostic store. It discards the numerical output and
does not execute or enable an estimator.

Estimator actions remain disabled. Passing these tests does not make OLS/WLS
or PPML available.

## Developer OLS preparation boundary

The internal `_stfixest_prepare_ols_test` command exercises a deliberately
bounded one-call boundary: plain continuous regressors, numeric factor main
effects, numeric all-categorical interactions, at most one analytic-,
frequency-, or probability-weight variable, zero or more unnamed single-source
intercept-only numeric fixed effects, and no time-series materialization. It is guarded
by an explicit developer confirmation token and does not post estimation
results. `stfeols` and `stfepois` still return `r(498)`.

The plugin decodes and validates model-wire 2.1, input bindings, matrix
dimensions, numeric variable types, and diagnostic destinations before reading
any observation. It reads the exact nonmissing `0`/`1` candidate marker for all
observations, then reads outcomes, numeric model sources, and any weight only
for candidate rows. Binding role 3 is generic numeric storage: the same source
may be used by `c.g`, `i.g`, and repeated ordinary or fixed-effect recipes and
is still read once. Raw numeric bits and a separate Stata-missing mask cross the
boundary. Positive finite analytic weights are retained without normalization,
zero and missing analytic weights receive drop reason 4, and a negative
candidate-row analytic weight fails the action. Frequency weights must be
finite, strictly positive, integral, and representable as `u32`; their raw
`f64` bits are retained. A value carrying the explicit Stata-missing mask
receives reason 4, while an unmasked NaN or infinity is invalid data.
Invalid candidate-row frequency weights fail the action. Any weight outside
the candidate marker is never read. Positive finite probability weights are
retained bit-for-bit. Explicitly masked missing probability weights receive
reason 4; zero, negative, or unmasked nonfinite candidate-row probability
weights fail before writes. Importance weights have no executable binding and
are rejected before observation reads. This boundary continues to count
retained physical rows only; weight sums, frequency-weight effective
observation counts, robust probability-weight VCE, and other inference are
later estimation contracts.

Supported numeric fixed effects are pruned to the reference singleton fixed
point after initial reasons 1--5 have been assigned. Ordinary, analytic-, and
probability-weighted requests count physical rows. Frequency weights use their
validated integer multiplicities, matching the pinned rule that a level is a
singleton only when its total frequency weight equals one. `keepsingletons`
bypasses deletion. Each deleted physical row receives reason 8 on the original
axis. A fallible consuming compaction rebuilds the row map, outcome, every
source, weight, and adjustment column together; final categorical sources are
re-factorized so codes are contiguous and vanished ordinary levels resolve to
zero recipes. The internal boundary may return an empty diagnostic sample; a
later estimator gate must reject insufficient samples before posting results.

For numeric factor main effects, Stata remains authoritative for expanded
levels, base selection, canonical coefficient names, and whole-column omission.
Rust does not rediscover or change the base. Available candidate factor values
must be nonnegative integers; masked or nonfinite values follow the ordinary
missing-regressor precedence. Rust first classifies and compacts the initial
numeric sample after availability checks, then deterministically factorizes
its active values for singleton pruning. If singleton rows are deleted, it
performs a second aligned compaction and re-factorizes the surviving values.
Signed zero is canonicalized only in factor codes;
the retained raw numeric source remains bit-exact. Every Stata coefficient slot,
including an omitted base, remains represented. A level removed by later
outcome, source, or weight exclusions is retained as an all-zero recipe for
later rank handling. All-categorical interactions retain a small ordered
component recipe per coefficient column and evaluate it against the already
compact source codes. They do not allocate an observation-scale interaction
column or tuple table. If any requested component level vanishes, or if the
requested tuple is absent, the interaction recipe is identically zero.
Categorical-by-continuous and continuous products, strings, fixed-effect
interactions/slopes/saved outputs, cluster encoding, an owned solver
`FeDesign`, and full estimator solving remain outside this checkpoint. A supported
numeric fixed-effect source accepts any finite `f64`, including negative and
fractional values. It is factorized after initial numeric compaction for
pruning, then re-factorized after any singleton compaction. Prepared
absorbed-term recipes reference the shared final encoded source; they contain
no copied codes. After the final singleton compaction, Rust constructs a
structural prepared FE topology in wire-term order. Repeated absorbed terms and
ordinary-plus-absorbed uses of one source share a single topology-source index.
Each unique FE source has one fallibly built compressed membership index over
the final codes; the topology does not own another code or level vector. Its
validator proves row alignment, contiguous observed final levels, exact
membership, term/source identity, checked total-level counts, and either the
post-pruning singleton fixed point or the explicit `keepsingletons` bypass.
An empty final sample preserves its absorbed-term recipes with zero-level empty
membership sources; a later estimator gate, not preparation, decides whether
the sample is sufficient.
The structural adapter preserves that empty representation. Construction uses
a fallibly reserved validated-source table and the returned view owns only a
fallibly reserved, wire-ordered effect table. Every effect borrows its code
slice from the final categorical-source table and its membership index from the
prepared topology. Each unique topology source receives one complete
structural validation of dimensions, levels, codes, and membership. Effect
construction then performs only constant-time source-identity, dimension, and
pointer checks, so repeated absorbed terms share both pointers without
repeating observation-scale validation. Its intercept-only type cannot
represent slopes, its coefficient-slot total uses checked arithmetic, and it
allocates no observation-scale storage.
The adapter itself does not call MAP, LSMR, the generic OLS core, or any plugin
estimator action. The generic `absorb_borrowed_intercept_map_certified` entry
point preserves its empty-design identity contract. Prepared OLS instead uses
an internal structural-constant wrapper: an empty explicit topology is a
virtual one-level intercept and is projected by weighted global demeaning.
The wrapper consumes its dense RHS matrix and uses only the borrowed intercept view.
It does not construct `EffectTerm`/`FeDesign`, call `within`, or copy FE codes
or membership, and it does not allocate N-row constant codes. Unit weights remain structural rather than an allocated vector
of ones. Supplied weights are validated as finite and positive and normalized
by a common maximum; a positive weight whose normalized representation
underflows to zero fails closed.

`build_prepared_ols_rhs` first revalidates the prepared sample axis, retained
row map, weights, categorical source encodings, coefficient-to-generated-column
mapping, and every supported continuous, categorical, interaction, omission,
and intercept recipe. It checks `1 + K` and `N * (1 + K)` before allocation,
fallibly reserves one row-major buffer, checks the returned capacity, and fills
it directly. Column zero is Y. Later columns are exactly the generated
`PreparedOlsModel.columns` in order; syntax-omitted coefficient slots are not
materialized and the intercept stays in its existing last position. A vanished
categorical level or tuple remains its validated all-zero column. The builder
accepts the valid zero-row preparation representation, including the zero-FE
case; the solver remains responsible for rejecting an empty numerical batch.
No intermediate observation-scale model columns are allocated.

`absorb_prepared_ols_rhs_map` builds the borrowed design before the dense RHS,
borrows prepared weights when present, and consumes the single batch through
the strict MAP entry point. Thus a construction, solver, or certificate error
cannot expose a partially residualized prepared object. Nonempty no-FE input
returns a certified one-pass virtual-constant projection under both default
reporting and `noconstant`.

The strict MAP path rejects empty observation or RHS axes and nonfinite values
before solving. All O(K) norm, scale, compensated-sum, and certificate buffers
are fallibly reserved before the first projection; no allocation occurs during
the iteration or final certificate. It consumes the RHS matrix, so a failure
cannot expose partially residualized caller state. A successful return always
has a passing fixed-effect projection certificate recomputed from the returned
residuals using pre-absorption RHS norms. Exhausting the iteration budget or
failing that certificate is fatal. Independent explicit-dummy tests cover
multiple RHSs, repeated sources, disconnected components, unit/analytic/
probability/frequency objectives, random designs, row permutations, and common
weight scaling. The empty-topology structural branch has fail-at-every and
undersize-at-every coverage for all six O(K) workspaces, with exact reservation
labels and sizes proving that no N-scale membership or code buffer is created.

The hidden developer option `mapcheck` maps to action key
`run_borrowed_map=1`; the key defaults to false. When enabled, the plugin checks
for interruption, constructs and certifies the prepared Y/X batch with fixed
private MAP settings, and checks for interruption again before the first
diagnostic store. Empty input, allocation/capacity/overflow failure, prepared
identity corruption, nonfinite input, weight failure, interruption,
nonconvergence, or failed certification returns before any SPI write or
display. When the option is absent, the existing empty diagnostic preparation
remains successful. On checked success, the plugin discards residuals, writes
only the same three developer diagnostics (original observation number,
retained status, and authoritative drop reason), and displays only small MAP
summary metadata. It does not write numerical estimation results, post `e()`,
or enable `estimate-ols`, `stfeols`, or `stfepois`.

The core also has an isolated prepared coefficient-fit consumer that is not
reachable from this developer action or any plugin dispatch. It accepts only a
nonempty `PreparedOlsData` value and the strict borrowed-intercept MAP design,
then solves finite coefficients with scaled deterministic pivoted Householder
QR. It uses the existing prepared recipes to reconstruct one raw residual
vector, residualizes that vector again, and independently recomputes every
generated-column score. It saves weighted within-X and within-Y norms after
the initial MAP and before destructive QR, so raw variation explained only by
the fixed effects cannot weaken the finite-score gate. An exactly zero within
norm uses the nonzero raw-column norm rather than skipping the redundant
moment; only an identically zero raw recipe has no score information. Canonical coefficient
slots distinguish syntax
omission, fixed-effect collinearity, numerical rank deficiency, and an
absorbed-model intercept whose normalized value remains deferred. The no-FE
case uses the same virtual structural intercept: a requested `_cons` is
reported only after QR as `NormalizedAbsorbedIntercept`, while `noconstant`
omits only that coordinate. Constant regressors are FE-collinear and full
factor-indicator spans have one fewer finite direction. Any allocation, MAP, rank, finite-value, or score
failure returns no fit object. This adds no SPI read, write, diagnostic, result
container, or public estimator path.

## Numeric source representation

Each numeric source contains its raw `f64` values and may contain a separate
missing mask. Except for the stricter frequency-weight preflight described
above, an entry is unavailable if either:

- the explicit missing-mask entry is true; or
- the raw value is NaN or positive/negative infinity.

This preserves Stata missingness even when the bridge uses a finite placeholder.
Every retained value is copied bit-for-bit. Preparation does not normalize
weights, transform exposures to logs, canonicalize signed zero, or otherwise
change the source bits.

Unweighted input and absent offset/exposure input are represented as sum-type
variants, not allocated pseudo-columns. Offset and exposure cannot be supplied
together because they are mutually exclusive variants.

## Numeric drop precedence

For each original row, the current numeric stage applies this exact order:

1. candidate false: `ExcludedByIfIn`;
2. unavailable outcome: `MissingOutcome`;
3. outcome-policy drop: `OtherModelExclusion`;
4. first unavailable ordinary source: `MissingRegressor`;
5. unavailable weight: `InvalidOrMissingWeight`;
6. weight-policy drop: `InvalidOrMissingWeight`;
7. first unavailable FE-only source: `MissingFixedEffect`;
8. unavailable offset or exposure: `MissingOffsetOrExposure`;
9. offset- or exposure-policy drop: `MissingOffsetOrExposure`;
10. singleton fixed-point deletion of a row retained by steps 1--9:
    `Singleton`;
11. otherwise: `Retained`.

A source used by both an ordinary and a fixed-effect recipe is classified at
the ordinary-source step, so its missing value is reason 3, never reason 5.
An FE-only missing source is reason 5. This locks the externally documented
reason ordering `1, 2, 3, 4, 5, 7` for the roles currently executable; cluster
reason 6 will be inserted between FE and adjustment when cluster preparation
is implemented. Initial numeric classification performs the first compaction.
FE factorization itself does not filter rows, but singleton pruning may trigger
one additional aligned compaction followed by final re-factorization.

Candidate-excluded rows invoke no policy hooks. A policy error is fatal rather
than converted into a drop. The policy has no default implementation: the
caller must explicitly provide the scientific domain rules. Empty original
data, an empty candidate set, zero regressors, and an all-dropped numeric sample
are valid preparation results. A later estimator-specific policy decides
whether an all-dropped sample is an estimation error.

Policy hooks receive an immutable policy reference: domain decisions must not
drift with row history or traversal order. Tests may record calls through
interior mutability, but that instrumentation cannot define the decision rule.

Cluster availability is intentionally not part of this precedence yet. It will
extend the authoritative reason mask without changing the locked ordering
above.

## Row map and allocation boundary

`original_row_by_active` stores zero-based original row numbers in strictly
increasing order. Accessors expose the row slice and per-row reason without
allocating a second mask or map.

All output vectors are dimension-checked and explicitly reserved with
`try_reserve_exact` before the first push: the reason mask, row map, outcome,
outer regressor table, each regressor column, and supplied weight or adjustment
column. Reservation failures map to native `Allocation` and Stata return code
909. Deterministic injection tests fail each reservation point, including
zero-sized paths. No partially built sample escapes an error.

Singleton pruning applies the same fail-closed rule to every observation- or
level-scale workspace: the active mask, per-level weighted counts, flat
membership offsets and observations, cursors, work queue, and dropped-row
list. Each buffer is fallibly reserved before it is populated. Deterministic
injection tests exercise every reservation point, including the
`keepsingletons` active-mask copy; capacity overflow and allocation failure map
to native `Allocation` and Stata return code 909.

Prepared FE membership construction also reserves its per-level counts,
offsets, cursors, and observation permutation fallibly and checks every count,
prefix sum, and cursor. The topology source/effect tables are reserved before
population and have explicit capacity guards. Deterministic injection tests
exercise every reservation and capacity boundary. Repeated FE terms share one
membership index, so this checkpoint adds no observation-scale code copy.
The borrowed FE adapter separately reserves an O(U) validated-source table and
an O(G) effect table with `try_reserve_exact`, where U is the number of unique
topology sources and G is the number of wire terms. It guards every push against
unplanned growth and maps both reservation and checked term-slot overflow to
native `Allocation`. Deterministic tests exercise both tables' reservation and
capacity failures. A repeated-source scale test instruments source validation
and proves one validation call covering N rows for 128 wire terms, rather than
128 observation-scale scans. Codes, level metadata, and membership arrays are
never cloned into the adapter.

## Model construction remains separate

The developer boundary now binds numeric factor main-effect,
all-categorical-interaction, and simple numeric fixed-effect recipes to compact
shared source codes and validates their structural prepared topology, but it
does not construct the solver `FeDesign`, invoke absorption, invoke the
estimator, or use the generic model-matrix builder. It can construct only the
non-solving borrowed intercept-only design view described above. Mixed
interactions, general FE/cluster construction, solver
allocation/certification hardening, and production one-call result posting
require later independently tested work.
