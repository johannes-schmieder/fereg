# PPML coverage inventory and sequential cluster inference — local 2026-09-25

## Outcome and identity

Adopt **pairedscan2**: replace eligible scratch-backed PPML cluster sorting/random
row reads with bounded sequential group-score accumulation. Fresh affected families
improve **31.7–60.5%**, with all 25 candidate/baseline pairs faster. The largest
fresh inference phase falls **548 -> 40 ms**. This is a substantial production
improvement, not completion of the multi-FE <=0.90-stock goal.

[ADR 0205](../../../../docs/adr/0205-sequential-paired-ppml-clusters.md) specifies
ownership, memory admission, separate certificate arithmetic and exclusions.
The computation is based on **daa0ce4e93a56337a0455ef05aa9af88760c6505**, with four
overlays: `sandwich.rs`, `sandwich/qualified_cluster.rs`,
`ols/linear_session.rs`, and `ppml/vce.rs`. All **565 captured computational inputs**
match the retained source. Candidate plugin SHA-256:
`4d4ae065abac8b9250c8f9218172ea6853f9b3d0e90a869e702a47ce53dc3020`.

Baseline is **higherdim1**, plugin
`92df56c44f5c40aabc4c297e3d2bbc863d528f0fe3f0ce395c7a7fb6723e50a2`.
The unused initial `pairedscan1` plugin, its source and evidence are preserved too.
Comparator is the pinned unmodified stock fixest archive
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.

All results are bounded local Mac development measurements. They are not frozen
suite qualification or evidence for other artifacts. No full campaign, SCC retry,
physical cap, push or release occurred.

## Matched work and new coverage

The additive development harness now covers supported factor main/cross terms,
one-/two-way clustered inference, and categorical saved outputs. Frozen 77 primary,
16 holdout and 111 native-control definitions are unchanged. Numeric/slope DGPs
retain their existing streams; factors and independent clusters use separate
streams. Positive unweighted outcomes only; zero/separation/offset performance
coverage is still incomplete. Separate licensed truth fixtures exercise offsets,
FW/PW and the existing failure boundaries.

Inputs/imports and scientific CSV/binary export lie outside the clocks. Headline
clocks include the complete public estimator, inference and requested output
construction. Native preparation includes native loading/preprocessing; native
runtime phases separate estimation, inference, FE recovery and publication.
Stock uses `glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads, `K.adj=FALSE`,
common-minimum-G clustered correction or the registered robust N/(N-1) correction.
The all-output worker computes mean, finite predictor, total absorbed effect,
linear predictor, response residual and individual FE contributions inside the
clock; `fixef()` recovery also receives 1e-11. Individual arbitrary FE gauges are
not compared directly: each source's group membership/reconstruction is checked,
and identified total outputs are compared at the declared bounds.

All headline monitors are clean. No compilation, audit, profile, archiving or
competing fit ran alongside them. Separate ordinary Rprof/native-trace diagnostics
are not headline clocks. Rprof attributes most stock time to native `.Call` but
does not by itself identify a specific C++ inner kernel.

## Inventory before implementation

This separate three-rotation seed347 screen measured **higherdim1**, not the new
candidate. Do not transfer these timings to pairedscan2. K below is numeric width
unless a finite total is shown.

| Case | fereg / stock median ms | Status |
|---|---:|---|
|Two FE, N122880/K17 factors/T4, fast, robust means|357 / 531|Competitive; 0.672|
|Same, two-way clusters and all categorical outputs|421 / 598|Unranked: residual discrepancy|
|Three FE, N122880/K21 factor cross/T8, low, two-way clusters|1768 / 1771|Measured target: 0.998|
|Unbalanced three FE, N122880/K10/T8, fast, all outputs|387 / 373|Measured loss: 1.038|
|Two slope partitions, N122880/K10/T8, low, robust means|347 / 497|Competitive: 0.698|
|Two FE, N983040/K6/T8, fast, robust means|1244 / 901|Measured loss: 1.381|
|Same, all categorical outputs|1581 / 1039|Unranked: residual discrepancy|
|Two FE, N24960/K2/T4, fast, robust means|37 / 31|Measured loss: 1.194|

Full inventory distributions and native phases are in `measurement-summary.json`
and the source-bound attempt receipts. Existing unresolved five-FE/three-FE means,
static inventory drift and constrained-fit failures remain separate open issues.
Unsupported RHS products were discovered by the admission check (native warm-up
r(498)); the harness now rejects this documented unsupported surface. The failed
attempt is retained, not converted to a performance observation.

A small nested-FE/cluster comparison agrees on b/V/mu/objective but native structural
DoF is 1908 versus explicit-design 1588 (N1920, FE rank 327, finite rank 5); both
reported cluster df values are 319. Native absorbs a nesting deduction into its
reported rank. This requires a focused policy/implementation adjudication; neither
the inference algorithm nor the oracle was changed to hide it. That comparison
stays unranked.

For the two all-output inventory discrepancies, xb/d/eta pass but response
residuals fail `rtol=1e-7, atol=1e-9`: 116 coordinates, max 2.058e-8 in the smaller
case; 6908 coordinates, max 9.737e-8 at N983040. Both arms pass their own output
identities and mean bounds. These facts do not establish which result is more
accurate near zero residuals, and the declared bound is unchanged.

## Mechanism and bounded correction

Code inspection finds that PPML's paired independent cluster route always sorted
rows and called pointwise `fill_row`. Scratch robust inference already had an
immutable sequential page reader, while OLS had its own bounded group-table route.
The new path generalizes that data-access mechanism while retaining PPML's separate
multipliers and certificate group banks. It neither removes certificates nor
changes any solve: outer/inner iterations, solve route and final-state authority
remain unchanged in the comparisons.

For each admitted subset, read original rows once; accumulate separate compensated
production and certificate group scores; merge their meats in opposite group
orders. Plan the complete bank, flags, row, three square arrays and descriptors
against the **two N-word arrays already coexisting during old radix construction**.
The bound also implies old radix eligibility for positive rank. Actual bank
capacities are checked. Large/overflowing products use the old route. No N-by-K
bank, cross-weight reuse or new persistent owner is introduced. OLS and dense/
robust PPML remain on their prior paths.

The initial one-order-array bound improved large/small targets from 1718/120 to
1447/77 ms, but left intersection subsets using random reads (stock 1772/62).
One coherent correction admits the full workspace under the displaced two-array
high-water mark. It improves the corrected pilot to 1321/53 ms. Initial/corrected
blocks remain separate; no further cutoff tuning followed the correction.

Corrected pilot, three B/C/stock rotations, seed **202609347**:

| Case | Baseline ms | Candidate ms | Stock ms | C/stock medians |
|---|---|---|---|---:|
|Three-FE factor/cluster, N122880/K21/T8 low|1748, 1765, 1771|1326, 1321, 1320|1779, 1776, 1765|0.744|
|Two-FE cluster, N30720/K6/T4 low|133, 123, 126|53, 53, 53|64, 56, 59|0.898|
|Two-FE factor/cluster, fast excluded|373, 383, 367|369, 380, 379|527, 541, 542|0.701|
|Two slope partitions, robust excluded|73, 74, 74|74, 73, 72|75, 79, 79|0.924|

Primary median inference falls 482.111 -> 35.264 ms; the small target falls
76.521 -> 3.544 ms. The excluded fast case retains a six-ms median loss (373/379),
while the robust slope control improves one ms. The small target's ratio of
medians is 0.898 but one paired ratio misses 0.90; this alone is not robust completion.

## Fresh frozen-source confirmation

Seed **202609436** was predeclared before execution and is now consumed/frozen.
Seven cases, five B/C/stock rotations, means and two-way clusters unless labeled.
No source or guard tuning followed these cases. Every scientific state and all 35
**direct** candidate/stock pairs pass. Baseline agreement is not used as a substitute
for direct comparison.

| Case | Baseline median | Candidate median | Stock median | C/stock | Saving to 0.90 stock |
|---|---:|---:|---:|---:|---:|
|Three FE, unbalanced, factor cross; N153600/K19/T8; low|1610|1100|720|1.528|452.0 ms|
|Two FE; N38400/K4/T1; low|140|83|110|0.755|0.0 ms|
|Two FE, weak barbell; N38400/K6/T4; low|167|66|2491|0.026|0.0 ms|
|Two FE, disconnected; N38400/K6/T4; low|164|74|70|1.057|11.0 ms|
|Two slope partitions; N38400/K6/T8; low|203|95|92|1.033|12.2 ms|
|Three FE; N38400/K6/T4; fast excluded|118|118|325|0.363|0.0 ms|
|Two slope partitions, robust; N38400/K6/T8; low excluded|88|88|88|1.000|8.8 ms|

All 25 changed-path baseline pairs improve. Of the seven fresh families, serial
two-FE, weak two-FE and excluded dense three-FE clear 0.90 in every pair. The other
four do not. The weak-reference result is specific to this strict stock/design
combination, not a general fixest claim.

All time vectors in repetition order (ms):

| Case index above | Baseline | Candidate | Stock |
|---|---|---|---|
|1|1562, 1576, 1636, 1689, 1610|1100, 1099, 1105, 1162, 1095|720, 717, 729, 715, 723|
|2|142, 143, 138, 140, 139|83, 82, 83, 82, 84|111, 110, 109, 110, 111|
|3|160, 149, 174, 167, 177|66, 67, 69, 66, 66|2491, 2480, 2535, 2493, 2488|
|4|162, 164, 192, 181, 164|74, 75, 76, 73, 73|68, 69, 71, 70, 70|
|5|203, 175, 146, 237, 279|95, 91, 91, 113, 113|88, 92, 92, 96, 100|
|6|120, 118, 126, 112, 116|118, 119, 118, 114, 115|325, 326, 341, 324, 322|
|7|87, 89, 88, 88, 89|87, 87, 88, 90, 88|87, 88, 89, 90, 87|

The clustered-slope baseline's146–279-ms variation and candidate's91–113-ms range
remain visible. Clean sampled monitors do not exclude every ambient OS effect.
No rerun was used to obtain a more favorable median.

## Independent science and regression controls

- **26 distinct Rust tests** pass: new separate integer group-score oracles at
  widths 1/6/21, varied dyadic weights, missing/unbalanced groups, intersections,
  T1/T4, complete-bank boundary/overflow and allocation/certificate failures;
  existing VCE/weight/rank-zero tests, OLS scratch tables and immutable reader tests.
- **13 Python surface tests** pass, including independent full-design influence
  covariance, known Poisson truth, generated-term/output boundaries and deliberate
  per-FE corruption that preserves the sum but fails its declared group span.
- **Three licensed fixtures** pass with explicit sentinels/return codes:
  `feppml_higher_dimensional_oracle` (unit/FW/PW, offset, robust/cluster, fast/low),
  `feppml_scientific_contract`, and `ols_cluster_batch` (independent explicit
  regressions and multiway covariance, weights and rank deficiency).
- Across the nine PPML screens, **266/267 public states**, **160/167 anchored
  pairs**, and **36/38 independent-oracle checks** pass. The unsupported-product
  application failure, nested structural-DoF discrepancy and six all-output
  cross-estimator residual discrepancies account for the failures. None is hidden.
  All 24 corrected small-oracle checks (initial/corrected candidate screens) pass.
- All **59 direct stock comparisons** across initial pilot, corrected pilot and
  fresh validation pass. Both builds pass native ABI containment/self-tests.

OLS controls have exact scientific exports in all nine pairs. Their baseline and
candidate vectors (ms) are: two slope partitions **70,72,69 /69,68,69**; low three
FE **83,87,84 /80,83,86**; narrow serial **10,10,11 /12,11,11**. Four wins, three
losses and two ties; narrow median loses one ms. Dense and robust PPML exclusions
also retain their individual losses. These checks protect the changed boundary;
they do not establish universal timing non-regression.

## Memory and remaining work

All 35 fresh tracked peak pairs, modeled selected bytes and observed scratch sizes
match. The large corrected pilot has tracked peak differences **-1,426,664,
+1,705,496 and +2,019,560 bytes** (set of differences), although its modeled bytes
and scratch are unchanged. The report does not attribute that variation to an
unmeasured cause. The local complete-workspace bound is relative to displaced
radix construction; it is not physical-cap qualification or a repair of the
previous static allocation inventory drift. No new warmed-allocation campaign ran.

The hard fresh unbalanced factor case remains **1100/720 ms**. Its candidate
median native ledger is preparation 103.9, projection 436.2, finite 40.8,
objective/line-search 96.9, strict-final 241.6, inference 40.1 and publication 23.6 ms.
The required saving to 0.90 stock is **452 ms**. Further cluster-table tuning cannot
provide it. The next subsystem is current-weight weighted-linear work and data
traversal for low-memory generated controls: decompose the 678-ms projection/strict
budget on separate development inputs, compare frozen-weight work and trace which
accepted coefficient/joint/RHS mechanisms serve it before choosing a replacement.
Retain fresh seed 436 only for validation.

The earlier million-row inventory also needs remeasurement on the new artifact
before optimization or a current-source claim. Its accepted-baseline gap required
433.1 ms; preparation, projection, strict solves and publication all contributed.
Saved-output discrepancies and nested structural DoF need separate scientific
adjudication. Zero/separated and harder-offset timing coverage remains untested.
The active plan in STATUS prioritizes substantial removable work over another
small disconnected/narrow threshold search.

## Receipts

The content-addressed archive contains exact artifact inputs, build controllers,
all declared cells/rotations, import fingerprints, attempt results, solver/scientific
exports, audit implementations/results, unit logs, sanitized fixture protocols,
separate diagnostics and every failure. Generated DTA, binary row outputs and
`saved_outputs.csv` are hashed and omitted to avoid duplicating large synthetic
row banks; raw copies remain local. `restore_receipts.py` verifies every restored
logical receipt. Archive SHA/count/privacy verification is recorded in
`archive-verification.json` and `receipts-content-addressed.sha256`.
