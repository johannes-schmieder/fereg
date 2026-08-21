# Native data plane

`prepare_numeric_sample` is the first authoritative, Stata-independent numeric
sample boundary. It accepts borrowed source views, an `if`/`in` candidate
marker, and an explicit estimator/reference domain policy. It returns compact
numeric columns and an original-length reason mask. It is not yet the complete
Phase 2 data plane: string factorization, interactions, fixed effects, clusters,
singleton deletion, separation, and estimator SPI dispatch remain outside this
slice.

Estimator actions remain disabled. Passing these tests does not make OLS/WLS
or PPML available.

## Developer OLS preparation boundary

The internal `_stfixest_prepare_ols_test` command exercises a deliberately
bounded one-call boundary: plain continuous and numeric factor-main-effect
regressors, at most one analytic-, frequency-, or probability-weight variable,
no fixed effects, and no time-series materialization. It is guarded
by an explicit developer confirmation token and does not post estimation
results. `stfeols` and `stfepois` still return `r(498)`.

The plugin decodes and validates model-wire v2, input bindings, matrix
dimensions, numeric variable types, and diagnostic destinations before reading
any observation. It reads the exact nonmissing `0`/`1` candidate marker for all
observations, then reads outcomes, numeric model sources, and any weight only
for candidate rows. Binding role 3 is generic numeric storage: the same source
may be used by both `c.g` and `i.g` and is still read once. Raw numeric bits and
a separate Stata-missing mask cross the
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

For numeric factor main effects, Stata remains authoritative for expanded
levels, base selection, canonical coefficient names, and whole-column omission.
Rust does not rediscover or change the base. Available candidate factor values
must be nonnegative integers; masked or nonfinite values follow the ordinary
missing-regressor precedence. Rust compacts the numeric sample once and deterministically
factorizes active values. Signed zero is canonicalized only in factor codes;
the retained raw numeric source remains bit-exact. Every Stata coefficient slot,
including an omitted base, remains represented. A level removed by later
outcome, source, or weight exclusions is retained as an all-zero recipe for
later rank handling. Interactions, string factors, FE and cluster encoding, and
model solving remain outside this checkpoint.
After native sample preparation and a final interruption check succeed, the
plugin writes only three developer diagnostics: original observation number,
retained status, and the authoritative drop-reason code.

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
4. first unavailable regressor: `MissingRegressor`;
5. unavailable weight: `InvalidOrMissingWeight`;
6. weight-policy drop: `InvalidOrMissingWeight`;
7. unavailable offset or exposure: `MissingOffsetOrExposure`;
8. offset- or exposure-policy drop: `MissingOffsetOrExposure`;
9. otherwise: `Retained`.

Candidate-excluded rows invoke no policy hooks. A policy error is fatal rather
than converted into a drop. The policy has no default implementation: the
caller must explicitly provide the scientific domain rules. Empty original
data, an empty candidate set, zero regressors, and an all-dropped numeric sample
are valid preparation results. A later estimator-specific policy decides
whether an all-dropped sample is an estimation error.

Policy hooks receive an immutable policy reference: domain decisions must not
drift with row history or traversal order. Tests may record calls through
interior mutability, but that instrumentation cannot define the decision rule.

Fixed-effect and cluster availability are intentionally not part of this
precedence yet. Those source types require their own native representations and
will extend the authoritative reason mask without changing the locked numeric
ordering above.

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

## Model construction remains separate

The developer boundary now binds numeric factor main-effect recipes to compact
codes, but it does not invoke the estimator or the generic model-matrix builder.
Factor interactions, FE/cluster source loading, and production one-call result
posting require later independently tested work.
