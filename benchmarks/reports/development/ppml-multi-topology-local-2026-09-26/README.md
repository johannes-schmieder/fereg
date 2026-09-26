# Unchanged multi-partition topology ownership

Local development evidence, 2026-09-26; not qualification. Adopt **multitopology1**
under [ADR 0215](../../../../docs/adr/0215-unchanged-multipartition-topology-ownership.md).
The multi-FE goal of at most 0.90 times comparable stock remains unfinished.

## Identities and implementation

Baseline **classifiedinput1**, accepted at **5e90bb3e**, plugin SHA-256
`6a9708ab9508de9e0c91394a9b0278f023476e586ec27c62cc3fb000aeed5b96`.
Candidate **multitopology1**, base 5e90bb3e plus four Rust overlays
(`fe/topology_finalize.rs`, `fe/topology_registry.rs`, `ols/prepared.rs`,
`ols/prepared_categorical_data.rs`) and the new explicit-dummy fixture
`tests/stata/development/ols_multi_topology_transfer.do`, plugin SHA-256
`ec59c589f62fd3c0992fb2428355b6b315ee10e28f029762024bc9a4440cddb2`.
All **567 computational inputs** match the candidate. Archive identities contain
complete source hashes, builds and ABI/panic-containment results.

One ownership constructor replaces the previous one-source-only constructor.
For unweighted prepared samples of at least 16,384 rows, existing exact membership
can prove that singleton pruning leaves every referenced single-source partition
unchanged. Explicitly kept singletons also preserve that axis. Move existing codes
and membership into final topology; preserve candidate ordering, first-use registry
ordering and repeated-recipe aliases. Every declared level, row and code/member
relationship is validated, and the final immutable owner retains its full check.
Weighted samples, interactions, small samples and singleton deletions retain the
generic path. Availability owners not referenced by the final effects are dropped.
Compatible one- and multi-partition slope availability is included; a separate
large one-FE slope oracle checks that extension and the frequency-weight exclusion.

No numerical geometry, weights, responses, solver state or certificate is reused
across calls. Samples, rank policy, tolerances, weight semantics, DoF, VCE,
separation, current-weight score/model-space/objective/reconstruction certificates,
final Hessian and transactional publication remain unchanged.

ADR 0202's earlier small-case PPML-only transfer remains rejected. This successor
uses a new million-row cost budget and consolidates the existing constructor,
with an explicit initial gate around ten public milliseconds. No source or guard
changed after the pilot or fresh validation; no seed or benchmark-name guard.

Stock fixest is unmodified, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Use glm.tol=1e-10, fixef.tol=1e-11, matched threads, BLAS one, declared robust or
clustered inference and fitted means/full outputs. Public command clocks include
loading, estimation and requested publication, excluding harness setup/import.
There was no compilation, audit, profile, archive or competing fit alongside
headline timings. All completed measurement monitors are clean.

## Public distributions

B=classifiedinput1, C=multitopology1, S=comparable pinned stock. Medians are ms.
[Ordered distributions](distributions.md) retain every timing, loss and protected
OLS block. Each row below is a separate design; do not extrapolate across seeds.

|Development case; N/K/T|B / C / S|C/S paired range|
|---|---:|---:|
|Two-FE numeric fast;983040/6/8|875 /861 /880|0.978–0.997|
|Same low memory|980 /967 /872|1.109–1.110|
|Unbalanced three-FE factors, cluster;153600/19/8|571 /567 /695|0.809–0.834|
|Three-FE, two slopes;61440/10/8|153 /152 /232|0.641–0.655|
|Two-FE, two slopes, low;30720/6/4|73 /71 /120|0.592–0.617|
|Two-FE serial;245760/6/1|545 /539 /937|0.573–0.585|
|Small unbalanced three-FE;30720/6/4|72 /72 /80|0.900–0.914|
|Protected one FE;61440/6/4|60 /61 /unranked|Mean discrepancy|

Three rotations give **16 wins, six losses and two ties** against baseline.
Large fast/low gains are **1.6/1.3%**. Four of seven multi-FE families meet the
10%-faster target in every pair; the small-three-FE family does not meet it in
all pairs. The one-FE loss remains visible.

Fresh **seed 202609445**, five predeclared rotations, is consumed. All **45** fresh
direct C/stock comparisons pass. The disconnected control is additional coverage;
no earlier definition or frozen benchmark matrix was changed.

|Fresh case; N/K/T|B / C / S|C/S paired range|
|---|---:|---:|
|Two-FE numeric fast;737280/6/8|669 /660 /760|0.859–0.875|
|Matched low memory|741 /735 /761|0.953–0.971|
|Two-FE numeric fast;368640/6/4|466 /461 /665|0.684–0.703|
|Unbalanced three-FE factors, cluster;184320/19/8|681 /681 /3159|0.214–0.217|
|Three-FE, two slopes;76800/10/8|191 /190 /293|0.633–0.657|
|Three-FE, four slopes, low;38400/6/8|115 /116 /119|0.958–1.000|
|Two-FE numeric serial;245760/6/1|544 /541 /934|0.574–0.595|
|Weak_d3;61440/6/4|130 /130 /762|0.168–0.172|
|Disconnected two FE, low;61440/6/4|96 /96 /98|0.969–1.000|

Fresh outcomes: **26 wins, 12 losses and seven ties**. Six of nine families meet
0.90 stock in every pair; low numeric, four-slope low and disconnected low do not.
Fresh large gains are **1.3/0.8%**, smaller than the development pilot. Retain the
four-slope median loss and the factor/weak/disconnected ties. Earlier seeds'
scientific discrepancies remain separate and are not retrospectively accepted.

## Attribution and remaining work

Separate **modelprofile1/modelprofile2** builds use the same instrumentation over
the respective production sources. Two alternating diagnostic pairs per case:

|Boundary|Million fast B/C ms|Million low B/C ms|Factors B/C ms|
|---|---:|---:|---:|
|Initial model generation|49.30 /35.28|49.05 /35.77|33.11 /29.82|
|Containing final categorical preparation|25.10 /9.91|25.01 /10.39|7.53 /3.86|
|Original availability membership|8.81 /8.80|8.84 /8.94|1.77 /1.68|

Baseline final topology construction costs **16.05/16.03 ms** and pruning
**2.50/2.49**, inside the second row. Candidate transfers owners and retains exact
validation; these generic construction calls disappear. Nested phases are not
additive. No diagnostic timings enter the stock headline table. This confirms
roughly 14 ms of model construction removed; stop tuning this boundary here.

The same candidate diagnostics identify the next substantial subsystem. Large
fast/low IRLS is **576/660 ms**, containing canonical retained-rank construction
**222/276**. Two-FE canonical product building takes about **119 ms** total in
each mode, including **40 ms setup**, **43 ms solve** and **21 ms fresh certificate
and product work**. Low-mode finite/rank work takes **65 ms**, versus fast **25**;
low retained-control writing adds **23 ms**. These are containing and nested
spans, not independent quantities to sum. Group worker sums are not wall time.
Original-input score certification costs about **40 ms**; loading about **42**,
public output publication about **24**, and inference **6/11**.

Code inspection of `ols/canonical_products.rs::build` shows connected-component
planning on every call and repeated group-neighbor accumulation/sorting for the
weighted matrix/diagonal and implicit incidence. Component identities and edge
structure are immutable within a sample generation; weights, anchors, scaling,
factors and responses are not. Measure those subparts before adopting a cache.
The coherent next implementation is a canonical-frame lifecycle that owns that
immutable structure once and feeds current-weight finite products/reconstruction
through the rank builder without redundant low-storage passes. Keep independent
current-weight and final-Hessian acceptance. This is a concrete cost target,
not a claim that the entire measured setup or finite span is removable.

Remaining development deficits to 0.90 stock are **69/182 ms** fast/low. Fresh low
numeric needs about **50 ms**, four-slope low about **9**, and disconnected low
about **8**. Prioritize the canonical-frame redesign over another small topology
or selection adjustment. Existing preparation and slope-solver gains remain.

## Science, controls, memory and failed attempts

**121 distinct Rust tests pass**, including topology aliases/ownership, malformed
axes/unused levels, every injected reservation failure, singleton weights/cascades,
prepared sample ownership, changing PPML generations and estimator boundaries.
**Ten distinct licensed fixtures pass**: the new large multi-FE explicit-dummy
fixture, existing singleton oracle, seven PPML scientific/transaction fixtures,
and an additional large one-FE slope oracle under unit/FW weights, fast/low and
T1/T4. All production and diagnostic ABI/panic-containment checks pass.

All **231 PPML public states** pass independent original-input sample, score,
model-space and objective checks: **219 production plus 12 diagnostic**.
There are **12 explicit Poisson/Hessian/sandwich comparisons**. All **79 native
PPML pairs** have exact coefficient/covariance/means and requested full-output
exports. Anchored comparisons pass **154/158**; direct C/stock pass **69/73**.
No completed pair is missing. Four older-seed stock discrepancies remain unranked:
one small full-output residual comparison and three protected one-FE mean comparisons.

All **48 OLS pairs** have exact coefficient/covariance exports. The original
three-pair medians B/C are **63/67, 75/75, 14/14, 207/210, 159/158, 17/16 ms**
(slopes, three-FE low, small numeric, factors, large numeric, one FE). A separately
predeclared five-pair repeat on unchanged source/cells/seed gives
**65/65, 76/75, 14/14, 207/209, 158/159, 16/17**. The original slope loss did not
reproduce; the small remaining losses remain visible. Neither block is discarded
or pooled to imply universal performance non-regression. Independent licensed
oracles supply the separate scientific authority.

No N-sized bank is added. Model-sized ownership/maps replace repeated compact-code
and membership construction. No memory allowance changes. Whole-fit tracked peak
deltas are **-240..0 bytes**; scratch deltas zero; modeled bounds **-48..+112**.
Preparation allocation removal need not reduce the later whole-fit maximum.
No physical-cap or constrained-memory/platform qualification is claimed.

Retain three setup/collector failures: initial sandbox refusal to expose process
metadata (no fits, resolved by execution permission), an invalid disconnected
harness label (no data or fits, corrected in separately named `fresh-valid`), and
a supplemental fixture filename/sentinel mismatch (Stata assertions and rc passed,
but collector failed; identical content under the matching filename then passed).
These are not missing or successful first attempts. All native timed applications
succeeded; no timeout, interruption or contended measurement is suppressed.

Static allocation-inventory drift, hidden-allocation wire-minor mismatch, the
existing strict checkpoint-count failure, weighted constrained-fit/nonretaining-CG
failures and SCC/platform deferrals remain open. Frozen 77/16/111 definitions,
stopped campaigns, reference pins and scientific envelopes are unchanged.
No push, release or full qualification campaign.

## Receipts

[Receipt container](receipts-content-addressed.zip) preserves **11,984 logical
receipts**, sources, declarations, all attempted outcomes, audits, ordered clocks,
exact-export checks, separate diagnostics and recipes. **14,445,639 bytes**;
SHA-256 `8aa0a8ad8e4218c6756c81b681ee83a1cb5ba47e57b10ecded7c45ba09552461`.
Run `restore_receipts.py` with a new destination to verify every logical hash.
Generated data and binary row exports are hashed rather than committed; independent
results and input identities remain archived. Licensed logs are protocol-only,
paths are sanitized, and public/synthetic source literals are explicitly reviewed.
