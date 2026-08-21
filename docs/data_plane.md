# Native data plane

`prepare_numeric_sample` is the first authoritative, Stata-independent numeric
sample boundary. It accepts borrowed source views, an `if`/`in` candidate
marker, and an explicit estimator/reference domain policy. It returns compact
numeric columns and an original-length reason mask. It is not yet the complete
Phase 2 data plane: model-wire binding, factorization, interactions, fixed
effects, clusters, singleton deletion, separation, and SPI loading remain
outside this slice.

Estimator actions remain disabled. Passing these tests does not make OLS/WLS
or PPML available.

## Numeric source representation

Each numeric source contains its raw `f64` values and may contain a separate
missing mask. An entry is unavailable if either:

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

The existing column-recipe primitive is not wired to the model protocol or to
this numeric sample. Native factor-variable compilation, FE/cluster source
loading, and one-call SPI integration require later independently tested work.
