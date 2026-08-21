# Native data plane

The first estimator-facing core is deliberately Stata-independent. The plugin
loads original-order source columns once and passes borrowed slices to
`prepare_numeric_data`. Rust then owns the authoritative initial sample.

## Drop precedence

The initial reason mask uses this deterministic precedence:

1. `if`/`in` candidate exclusion;
2. missing outcome;
3. missing finite regressor;
4. missing or invalid weight;
5. missing fixed-effect source;
6. missing cluster source;
7. missing offset or nonpositive/missing exposure.

Singleton and PPML-separation reasons are applied by later native stages. The
original-length reason mask and zero-based native-to-Stata observation map are
retained throughout estimation.

## Factor columns

Stata supplies canonical column recipes and names. Rust generates intercepts,
continuous columns, categorical indicators, categorical interactions,
category-by-continuous slopes, and continuous products. `for_each_block`
provides the bounded workspace used by `memory(low)`; plain factor variables do
not require Stata dummy tempvars.

## Weight validation

All nonmissing supported weights are strictly positive. Frequency weights must
be exact positive integers. Estimator-specific normalization and VCE semantics
remain centralized in the estimator/VCE layers rather than changing the
observation-level weight values during loading.
