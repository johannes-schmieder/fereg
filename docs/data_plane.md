# Native data plane

The data plane turns a validated Stata request and candidate marker into one
authoritative compact sample, model graph, certified estimate, and complete
private result. Public estimator dispatch is active for `fereg` 0.5.0.

## Preparation order

On macOS, bounded numeric column batches validate observation bounds, numeric
variable types and destination pointers before using the SDK's direct `vdata`
callback. Every callback return code is checked. Scalar reads and other
platforms retain the checked accessor; this does not enable global fast mode.

Eligible unrestricted Mac OLS calls with ordinary categorical intercept effects,
at least 16,384 rows and 16–64 loaded numeric columns may overlap
loading with native conversion. The Stata thread fills two Rust-owned 4,096-row
tiles; one job on the resolved native team writes the existing source columns
and missing masks. Scratch capacity is admitted and charged to the input peak
before reading. Insufficient optional scratch, a single requested thread, or an
unsupported shape selects the existing column reader before any tiled read.
The input-stage ledger retains the explicit worker-runtime coverage gap for
opaque Rayon/native thread storage, as later native phases already do.

For fast-memory calls whose generated design is the loaded continuous sources
in order followed by an intercept, the worker may also append each tile to an
admitted RHS owner. This extends overlap to 9–64 loaded columns. The worker
copies all source values before writing the constant into the RHS tile. The RHS
capacity is charged beside input scratch and throughout preparation, then
transferred into the fit's existing matrix allowance. The prepared RHS builder
retains its validation order and checks every value bit against the final
prepared design before consuming this owner. A changed sample or recipe
releases the speculative matrix before ordinary construction. This is a
same-call ownership transfer, with no reusable data cache or extra SPI call.
Each absorbed effect must have one categorical source and no slopes. Robust,
clustered and saved-output requests can use the same input mechanism; inference
and publication retain their separate contracts. See
[ADR 0075](adr/0075-dense-two-fe-ols-data-and-solver-passes.md). Supplied AW/FW/PW
weights occupy an additional numeric column and use the same transport and raw
RHS path. Their domains, normalization and sample remain preparation-owned;
raw unit-weight moment overlap excludes supplied weights. See
[ADR 0078](adr/0078-weighted-native-input-overlap.md).

For each original row, candidate exclusion from `if`/`in` is applied first.
On candidate rows, outcome, ordinary terms, weights, fixed effects, clusters,
and time-series materializations are checked in the registered precedence.
Candidate-excluded observations are not read for scientific domain errors.

For unrestricted OLS, the transaction reuses its private candidate marker as
the result-sample shadow after metadata parsing. The request already declares
all rows as candidates, so the unread candidate input slot can bind the numeric
outcome while its ordinary outcome slot remains unchanged. All scientific
inputs remain disjoint from output writes. Native exclusion stores clear the
private marker, and successful publication transfers it to `e(sample)`;
restricted calls retain the separate candidate and result variables.

For an unrestricted all-numeric OLS candidate universe, the Stata thread copies
bounded observation-order batches directly into Rust-owned columns. Rust
classifies the ordered numeric missing range using the SDK's runtime
`SV_missval`, including extended missing codes; no separate ado `markout`
precheck is needed. At 16,384 original observations, restricted and mixed-source
input also batches numeric columns over retained runs. Runs shorter than 64 rows
use scalar reads; all batches are bounded by 65,536 rows. Fixed-width strings
keep candidate-gated scalar reads and original encounter order, while their
numeric peers can batch. PPML offsets/exposures share the retained-run reader.
No excluded source cell is read or additional observation-sized state allocated;
see [ADR 0042](adr/0042-candidate-run-numeric-loading.md).
Wide OLS preparation uses the requested
worker team for independent completeness checks after loading has finished.
These checks only select the ownership shortcut; the ordinary classification
path remains authoritative for exclusions, missingness, and domain errors.

OLS inputs with exclusions, missingness or supplied weights use that same row
classifier to fill a native drop mask. Wide inputs classify independent
16,384-row blocks on the installed team, selecting any error in original
observation order. The retained-row map then drives stable, independent
column compaction. Existing buffers are reused only when their capacity is
no larger than the general path's candidate-axis bound; restricted inputs
otherwise receive fallibly allocated compact columns. Raw weight values and
their kind follow the same retained-row map. Conventional, robust and clustered
OLS, including interacted/slope/saved absorbed terms, consume the loaded source
buffers through this boundary. Cluster missingness remains part of classification
before the existing singleton and final coding passes.

PPML first classifies borrowed numeric sources without observation-sized
allocation, then selects its generation-zero memory route. After admission the
same visitor builds the retained-row map and drop ledger, checking exact agreement
with preflight counts. It forms canonical weight reports without copying the
outcome/model sources into an unused temporary sample. The generation owner keeps
those raw original-axis sources for later categorical rebuilds and consumes the
offset/exposure allocation using stable compaction within the candidate-capacity
bound. Exposure is logged exactly once. Preparation remains serial; scientific
domain precedence, per-generation pweight normalization and categorical/cluster
construction remain unchanged under [ADR 0038](adr/0038-ppml-numeric-selection-ownership.md).

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

For one effect with unit weights, no cluster sources and at least 16,384 active
rows, preparation may construct the ordinary final membership index first.
If every group has at least two members, that index certifies the singleton
fixed point and transfers directly into final topology construction. No active
mask or separate count table is needed. If a singleton exists, the index is
released before the ordinary pruner allocates. The existing final topology
validator still independently checks every membership and singleton invariant.
This moves one owner within the existing topology allowance; it adds no
retained index or observation-scale buffer.

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

Categorical interaction coding uses a bounded exact tuple mapping when it fits
the row-order workspace, otherwise the existing lexical sort. Both produce the
same final codes and retain component validation and fallible allocation; see
[ADR 0046](adr/0046-bounded-interaction-coding.md).

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

Eligible ordinary-intercept FE loaded RHS storage can carry its raw-moment cache through
preparation. Both are private fields of one consumed native owner. Cache reuse
requires exact agreement with the authoritative prepared rows, source recipes
and all values; a nonunit frame or any mismatch uses ordinary fitting. The
cache is formed only from the raw input matrix, and independent final residual,
FE and inference certificates remain unchanged.
