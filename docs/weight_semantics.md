# OLS weight-semantics contract

Weight behavior is frozen by the exact pinned reference plus independent
weighted least-squares and frequency-expansion oracles. The ado layer parses
the expression and source; Rust owns candidate-row domain validation, sample
effects, objective scale, singleton multiplicity, DoF, VCE, and metadata.
Values outside `if`/`in` are not inspected for scientific domain errors.

## Public kinds

| Stata kind | Candidate-row domain | Effective N | Public inference |
|---|---|---:|---|
| none | structural unit weight | physical retained rows | IID/robust/cluster |
| aweight | finite; zero/missing drops, negative errors | physical retained rows | IID/robust/cluster |
| fweight | finite, strictly positive integer within supported multiplicity | exact frequency sum | IID/robust/cluster |
| pweight | finite and strictly positive; missing drops | physical retained rows | robust or cluster; IID rejected |
| iweight | not accepted by pinned reghdfe OLS contract | none | exact `r(101)`, no result |

Frequency-domain errors map to the registered `r(401)`; negative analytic
weights map to `r(402)`. Candidate-excluded invalid values are ignored.
Singleton deletion counts physical rows, but fweight singleton multiplicity is
the exact checked integer weight.

## Objective and reference scale

Prepared OLS minimizes positive weighted squared residuals. A common
maximum-weight normalization may be used internally for stability but cannot
change coefficients, scores, fitted values, residuals, or reported reference
scales.

- aweights and pweights use `raw * physical_N / sum(raw)` for reference
  cross-products;
- fweights use raw integer multiplicity and exact summed effective N;
- unweighted models use one without allocating a pseudo-column.

A common aweight/pweight rescaling leaves coefficients, bread, VCE, and fit
statistics invariant while preserving the supplied raw sum metadata. Integer
fweight scaling matches literal row expansion.

## IID, robust, and cluster rules

For supported IID models:

`V = (RSS / df_r) * bread`.

Probability-weight IID is rejected because the pinned contract requires robust
inference. HC1 uses `N_effective / df_r`; score-meat multipliers are one for
unweighted, squared normalized reference weights for aweights/pweights, and
frequency multiplicity for fweights, matching literal expansion.

Cluster scores use `omega_i * e_i * z_i`. Each nonempty declared-cluster
subset is aggregated collision-safely, inclusion–exclusion combines subset
meats, and the common finite-sample correction uses effective N, the registered
DoF denominator, and minimum cluster count. Nesting affects absorbed rank and
the correction denominator under the centralized DoF policy.

## Required evidence

Exact equality is required for success/error status, final sample, weight type
and expression, effective N, raw weight sum, singleton identities, DoF/VCE
policy, and cluster counts. Registered tolerances apply only to numerical
coefficients, covariance, residuals, and fit values. The frozen cases cover
constant rescaling, zero/missing/negative/large/fractional weights, invalid
weights outside the candidate sample, `if`/`in`, and weighted singleton
cascades.
