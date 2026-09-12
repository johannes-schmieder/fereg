# Native data plane

The data plane turns a validated Stata request and candidate marker into one
authoritative compact sample, model graph, certified estimate, and complete
private result. Public estimator dispatch is active for `fereg` 0.5.0.

## Preparation order

For each original row, candidate exclusion from `if`/`in` is applied first.
On candidate rows, outcome, ordinary terms, weights, fixed effects, clusters,
and time-series materializations are checked in the registered precedence.
Candidate-excluded observations are not read for scientific domain errors.

For an unrestricted all-numeric OLS candidate universe, the Stata thread copies
bounded observation-order batches directly into Rust-owned columns. Rust
classifies the ordered numeric missing range using the SDK's runtime
`SV_missval`, including extended missing codes; no separate ado `markout`
precheck is needed. Restricted candidates and fixed-width strings retain
candidate-gated scalar SPI reads. Wide IID preparation uses the requested
worker team for independent completeness checks after loading has finished.
These checks only select the ownership shortcut; the ordinary classification
path remains authoritative for exclusions, missingness, and domain errors.

Unweighted IID inputs with exclusions or missingness use that same row
classifier to fill a native drop mask. Wide inputs classify independent
16,384-row blocks on the installed team, selecting any error in original
observation order. The retained-row map then drives stable, independent
column compaction. Existing buffers are reused only when their capacity is
no larger than the general path's candidate-axis bound; restricted inputs
otherwise receive fallibly allocated compact columns. Weighted, complex
categorical, robust/clustered, and PPML preparation retain their existing paths.

The OLS transaction preserves row order without a `sortpreserve` wrapper. Its
parser and time-series materializations do not sort; native original-row maps
control output stores on every successful estimate.

After initial compaction:

1. numeric and fixed-width string categorical sources are collision-safely
   factorized;
2. the singleton fixed point is computed, respecting frequency multiplicity
   and `keepsingletons`;
3. all aligned arrays are compacted together;
4. final categorical codes, ordinary factor/interaction recipes, absorbed
   intercept/slope topology, and cluster topology are rebuilt;
5. the original-row map and full-axis drop mask remain authoritative for
   `e(sample)` and saved outputs.

Lag, lead, and difference operators are materialized by Stata against the
declared time/panel settings before native binding; gaps retain Stata's missing
semantics.

## Weights and categorical structure

Unweighted input is structural. Analytic, frequency, and probability weights
follow their registered objective, domain, effective-N, DoF, and inference
rules. Frequency weights use exact supported multiplicities. Importance
weights are rejected with `r(101)` to match the pinned reference. Required
inference upgrades or unsupported combinations fail explicitly.

Categorical/interaction recipes reuse compact source codes. Repeated FE sources
share membership; absorbed slopes borrow aligned continuous sources. No
observation-by-arity string key table or dense dummy matrix is part of the
production representation. Disconnected and no-regressor models use the same
sample/topology contract.

## Computation and outputs

Certified MAP residualization and rank-aware finite regression operate under
the selected fast/low execution mode. Successful results require convergence,
orthogonality, normal-equation, covariance, fit, and DoF certificates. IID,
HC1 robust, and one-/multiway clustered covariance share the same canonical
coefficient/status graph.

Rust computes residuals, absorbed contribution, normalized saved FEs,
`e(sample)`, fit statistics, and the ingredients for `predict` (`xb`,
absorbed contribution, `xbd`, residuals), `test`, and
`estat summarize`. Ill-conditioned or nonunique FE normalization is judged
on fitted values, residuals, identified combinations, objectives, and
certificates.

## Failure and allocation policy

Large sizes use checked arithmetic and fallible reservations. SPI access
remains on the Stata thread; worker threads receive only Rust-owned data.
Allocation/overflow, invalid identity, nonfinite data, interruption,
nonconvergence, or failed certificates cannot produce a complete receipt.
The outer ado transaction restores prior estimates and requested output
variables on every failed or interrupted path.

Memory bounds and scratch behavior are in `memory_model.md`; version and
transaction rules are in `plugin_protocol.md`.
