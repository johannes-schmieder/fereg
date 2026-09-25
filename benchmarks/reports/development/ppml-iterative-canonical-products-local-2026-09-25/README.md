# Iterative canonical PPML products

Local Mac arm64 development evidence, 2026-09-25. No qualification or release.
Decision: [ADR 0193](../../../../docs/adr/0193-iterative-canonical-schur-products.md).
Retain **canonicalcg3**: it extends the existing two-categorical-FE canonical
product builder with certified iterative reduced solves when cubic factorization
work is too expensive. Fresh threaded disconnected medians improve **14–24%**
against the accepted baseline. The broader 0.90-times-stock objective is open.

## Source, comparator and clocks

Baseline **resgate1** was accepted at `b8f04a23`; its runtime also matches the
later documentation checkpoint `a9164bfb`. Candidates overlay that checkpoint.
The final candidate changes `ols/canonical_products.rs`, exposes the existing
`normalized_product` helper in `fe/slope_coefficients/joint.rs`, and adds
`feppml_iterative_canonical_oracle.do`. All **557 captured inputs** match the
working source at closeout. Documentation edits after capture are not computation
inputs. Exact source maps, patches, build logs and identities are archived.

**framegapledger1** is an isolated diagnostic build. **canonicalcg1** is the first
CG implementation; **canonicalcg2** adds test/format documentation without
changing its runtime arithmetic or routing. **canonicalcg3** replaces the serial
dependent matvec product with the existing eight-accumulator kernel. Only cg3 is
adopted. Each new artifact passes actual-plugin ABI/self-test and panic containment.

| Artifact | Plugin SHA-256 |
|---|---|
|resgate1|9103c3092e83291e2941634ac07de06d445bfec85c39ca372a3b96f833f166d9|
|framegapledger1|f08ed8a859b037af8d6e8d7af171bcb4d2f5ed8ebf4b63075f7a6b80a7dd2581|
|canonicalcg1|b5c27542768ef272b1a6fc9f46a25c8586ee43c90fb2edb48713c63f852c58cb|
|canonicalcg2|42163f77bcde8a8798dfdc10d324f0ce6f316586a3011c4228fad58596c06714|
|canonicalcg3|dcf1672c99e05521f5869309a3756f862025794e6bc23d95e6518e3088755d98|

Pinned stock fixest is unmodified **0.14.2**, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
All stock comparisons use `glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads,
robust inference and fitted means. BLAS/OMP/vecLib threads are one. Headline
inputs have positive unweighted outcomes and numeric regressors; weighted science
fixtures do not establish weighted performance.

Data generation/import, process startup and warm fits are outside the clock.
The complete public command, including loading, certification, inference and
outputs, is inside. Each block rotates baseline/candidate/stock, with a fourth
prototype variant in the kernel block. No compilation, diagnostic profiling,
archiving or competing fits ran alongside headline fits. Monitors record no
sampled overlap or monitoring error. This does not certify absence of all ambient
machine activity. No application timeout or application failure occurred.

Development seed **202609347** serves pilot/kernel/diagnostic cases. Fresh
**202609428** was declared before cg2 confirmation; after its serial loss, the
kernel change was developed and compared on seed347. Final cg3 confirmation uses
new, predeclared **202609429**. Both fresh seeds are now consumed/frozen. No
routing threshold, tolerance or admission rule was fitted to their results.

## Measured cause and implementation

The previous plan proposed a shared canonical builder, but source inspection
confirmed that ADRs 0168–0175 already implement fused current-weight canonical
products, component factors, column-owned RHSs, original-input certification and
bounded reconstruction. The remaining opportunity was its factor-work refusal.

In the separate baseline diagnostic on disconnected N42240/K10/T8 low, the cold
call has P7040/Q704 levels and four components of Q176. Pair assembly costs
105,600 estimated operations; component factorization costs **7,269,032**, above
the cold remaining work allowance **2,682,240**. It therefore returns to the
ordinary certified projection path. The later final call has 5,491,200 observed
operator work available, raising its allowance to 8,173,440 and admitting direct
factorization. Its complete minimum storage is 2,441,088 bytes within 6,758,400.
These are implementation work estimates, not hardware instruction counts.

The new ordinary-boundary ledger assigns 0.917 ms to original norms, 0.449 ms to
score setup, 2.432 ms to original scores and 1.208 ms to reconstruction/reporting
across three calls. That is about 5.006 ms of the previously measured 9.84-ms
boundary gap. The rest includes geometry; it was incorrect to attribute all of
it to verification. Inclusive/nested spans must not be summed into whole time.

A separate symbolic diagnostic found little room for sparse/skyline factoring:
a representative free block has dimension 175, 13,057 edges, reordered bandwidth 166,
14,931 skyline cells and 824,500 estimated products versus 893,200 dense products.
No sparse implementation was added on this evidence.

The accepted change retains existing pair assembly and full memory admission.
If cubic factor work is too high, CG solves the same grounded, diagonally scaled
component matrices independently for each response/control column. Matrices are
prepared once per current call, and completed columns stop individually. Cheap
recurrence diagnostics require finite positive curvature and a residual target
0.01 times the existing MAP tolerance. The configured iteration cap still applies,
bounded also by component dimension. The original-row projection certificate
and direct residual Gram remain mandatory; numerical or certificate refusal
retains the old projector. No state is authoritative across changed weights.

The final boundary trace shows initial/final iterative counts **13/19** at
N42240/T8 and **13/20** at N38400/T1; warm N7680 calls remain direct. Counts are
maxima over component/RHS solves, not comparable units of MAP sweeps. These four
trace fits are science/route diagnostics, not headline timing observations.

Canonical finite rank/status, inverse, finite/original scores, reconstruction,
outer objective/score, strict comparisons and exact final `q*mu` Hessian remain
unchanged. The old early-checkpoint schedule experiments stay rejected. This
change reaches existing fast/low two-categorical-FE canonical entries; ordinary
directions, slope/three-plus-FE canonical engines and OLS routing are unchanged.

## Final fresh block: canonicalcg3, seed429

Five rotations per case. B is accepted resgate1, C is cg3, S is stock. The target
is C/S <=0.90. All **120 public states and 40 C/B comparisons pass**. Two stock
cases remain scientifically unranked; their clocks are shown without speed ratios.

| Case | Design; N/K/T; storage | B/C/S medians ms | C/B | C/S | C/B wins/losses/ties |
|---|---|---|---:|---:|---|
|0|Disconnected two categorical; 49920/12/8; low|144/110/93|0.764|unranked|5/0/0|
|1|Disconnected two categorical; 61440/6/4; low|132/114/101|0.864|1.129|5/0/0|
|2|Disconnected two categorical; 38400/10/1; low|143/142/244|0.993|0.582|3/2/0|
|3|Disconnected two categorical; 49920/12/8; fast|112/93/95|0.830|unranked|5/0/0|
|4|Narrow two categorical; 24960/2/4; fast|42/41/33|0.976|1.242|4/0/1|
|5|Wide two categorical; 23040/36/8; low|92/92/124|1.000|0.742|3/1/1|
|6|Weak barbell categorical; 17280/4/4; low|35/35/750|1.000|0.047|0/2/3|
|7|Two slope partitions; 38400/8/4; fast|106/107/177|1.009|0.605|0/2/3|

There are **25 winning, seven losing and eight tied C/B pairs**. Three threaded
disconnected medians improve 23.6%, 13.6% and 17.0%. The serial median improves
only 1 ms, with two losing pairs. Four of six rankable stock medians meet the
target. The low T4 case still needs **23.1 ms** to reach 90.9 ms; narrow fast
needs **11.3 ms** to reach 29.7 ms. These misses remain priorities. Wide and weak
controls tie in median, and the slope control loses 1 ms.

**Unresolved science:** cases 0/3 disagree with stock on six fitted means in every
repetition, in both baseline and candidate. Maximum absolute error is about
**6.84987e-7**, maximum envelope ratio **1.54310**. Coefficients, covariance,
sample, likelihood and deviance pass; both packages' independent public
model-space/score/objective replays pass. Every candidate/baseline comparison
passes. This localizes the discrepancy away from a newly introduced candidate
change but does not establish which solution is more accurate. Neither stock
ratio is ranked, and no envelope is changed. This is separate from the earlier
dense three-FE coefficient discrepancy, which also remains unresolved.

## Full distributions and rejected prototype

Vectors below are repetitions in declaration order, in ms; do not pool blocks.
The initial pilot uses cg1. Fresh seed428 uses cg2, whose serial median loses
11 ms (144→155); that version is not adopted. The development kernel block
compares B/resgate1, D/cg2, C/cg3 and S/stock. All six C/B and all six C/D pairs
improve there. Final seed429 above is the independent confirmation of cg3.

### Pilot, seed347; B/C(cg1)/S

| Case; N/K/T; storage | resgate1 | canonicalcg1 | fixest |
|---|---|---|---|
|0; 42240/10/8; low|100,97,103|92,91,92|72,76,76|
|1; 21120/2/4; low|37,35,36|35,34,35|27,31,31|
|2; 16320/32/8; low|62,63,63|62,64,62|88,85,87|
|3; 30720/6/8; fast|77,78,77|76,76,75|79,80,81|

### First fresh, seed428; B/C(cg2)/S

| Case; N/K/T; storage | resgate1 | canonicalcg2 | fixest |
|---|---|---|---|
|0; 49920/12/8; low|137,138,141,139,137|113,112,111,111,113|90,92,91,92,91|
|1; 61440/6/4; low|130,129,132,128,128|124,123,121,122,124|98,94,97,98,98|
|2; 38400/10/1; low|144,144,147,148,142|155,155,154,157,151|242,242,242,242,241|
|3; 49920/12/8; fast|108,115,109,112,111|96,101,95,96,96|90,90,90,92,90|
|4; 24960/2/4; fast|40,39,41,42,40|38,39,39,38,38|35,33,33,32,33|
|5; 23040/36/8; low|87,87,86,86,88|87,86,88,85,87|126,126,129,127,125|
|6; 17280/4/4; low|31,31,30,30,31|30,31,31,31,31|633,630,631,626,635|
|7; 38400/8/4; fast|102,105,104,103,106|104,104,103,105,105|178,181,180,183,181|

### Kernel comparison, seed347; B/D(cg2)/C(cg3)/S

| Case; N/K/T; storage | resgate1 | canonicalcg2 | canonicalcg3 | fixest |
|---|---|---|---|---|
|0; 42240/10/8; low|98,97,96|91,92,95|87,87,88|76,74,78|
|1; 38400/10/1; low|141,146,152|152,155,150|138,145,138|242,235,241|

### Final fresh, seed429; B/C(cg3)/S

| Case; N/K/T; storage | resgate1 | canonicalcg3 | fixest |
|---|---|---|---|
|0; 49920/12/8; low|146,144,145,142,143|113,106,114,110,107|93,91,93,93,92|
|1; 61440/6/4; low|137,132,132,133,130|116,116,113,114,114|98,101,101,96,102|
|2; 38400/10/1; low|141,146,142,143,144|146,139,140,142,145|247,244,244,243,249|
|3; 49920/12/8; fast|112,120,114,111,112|93,97,99,93,92|95,100,94,95,95|
|4; 24960/2/4; fast|43,42,41,42,46|41,40,41,41,40|30,33,33,34,35|
|5; 23040/36/8; low|90,89,93,92,106|90,85,92,93,93|125,129,123,124,123|
|6; 17280/4/4; low|35,35,36,35,34|35,36,36,35,35|750,750,751,745,748|
|7; 38400/8/4; fast|106,104,106,107,105|106,107,106,107,109|179,175,176,178,177|

## Independent validation and controls

Final Rust results: **68 distinct passes** (12 canonical product, 53 prepared
IRLS and three joint-product tests), plus the known baseline count assertion
failure `refined_checkpoint_preserves_accuracy_on_a_small_dynamic_range_model`
(expected strict rejection count 1, actual 0). It remains visible and unrepaired.
The initial connected-cycle test expected factor-work refusal; after the new
route it now requires a certified iterative result, while a one-iteration cap
still refuses. Scientific truth assertions were not loosened.

New Rust tests compare reduced SPD products with independent known solutions
at dimensions 2/7/31 and scales 1e-200/1/1e200, including unfinished-solve refusal.
An analytic weighted two-FE design has paired nonzero residuals, known beta,
a zero control, changing weights and T1/T4. It exercises actual iterative
admission, exact minimum-budget success, one-byte-below refusal preserving the
old panel, and iteration-cap refusal. Existing rank, changed-frame, strict
rollback and PPML tests protect the surrounding machinery.

Four licensed independent fixtures each pass on cg1, cg2 and cg3: **12 executions**,
all with process RC=Stata RC=0 and an explicit terminal `END_FIXTURE` sentinel:

- New `feppml_iterative_canonical_oracle.do`: N6144/K4, two categorical partitions,
  unit/PW, known nonzero Poisson residuals, explicit-design Hessian and robust
  sandwich; current means and coefficients checked independently.
- `feppml_multipartition_slope_oracle.do`.
- `feppml_three_partition_slope_oracle.do`.
- `feppml_canonical_products_oracle.do`, including FW/PW, robust/cluster,
  offset and output controls.

Three unchanged OLS definitions protect small two categorical FE, three
categorical FE and two slope partitions. All **nine comparison pairs have exact
exports**. Performance is mixed; do not claim these routes improved.

| OLS control; N/K/T | Baseline ms | Candidate ms | B/C medians ms | C/B wins/losses/ties |
|---|---|---|---|---|
|0; 4320/2/1|10,11,9|11,10,11|10/11|1/2/0|
|1; 34560/8/4|57,60,57|58,57,56|57/57|2/1/0|
|2; 30720/6/4|107,104,106|108,107,106|106/107|0/2/1|

Across all blocks: **306 PPML applications/public states pass** (300 headline,
six diagnostic). Baseline-anchored audits have 195 passing pairs and ten unranked
stock pairs; direct candidate/stock audits have 88 passes and ten unranked pairs.
These audit categories overlap observations and must not be added as independent
fits. All fereg candidate/baseline science pairs pass. There are also 18 OLS
applications, nine exact OLS pairs and the 12 licensed fixture executions above.

## Memory and remaining work

CG scratch is three `D*W` double vectors, charged conservatively with the reduced
matrix bank in complete phase admission and released before certification.
The existing `16*N*pool` allowance is unchanged; nothing persists across calls.
The final fresh tracked-peak changes, by case 0–7, are **-1,885,920; +433,344; 0;
-2,681,856; -399,360; 0; 0; 0 bytes**. Scratch-file sizes are unchanged throughout.
Selected-model telemetry changes range -112 to +48 bytes. Lower tracked peaks
reflect admitted ownership/phase changes, not a reduced scientific workload.
No new physical-RSS/cap qualification or separate warmed-allocation campaign was
run. The exact-budget tests are narrower evidence.

The initial factor-work refusal is addressed. Remaining low-mode cost includes
ordinary current-frame preparation, independent scores/reconstruction and
canonical/final work; this milestone has not removed all repeated scans. Narrow
complete-command overhead remains substantial, so another recurrence-only tweak
cannot meet its target. Resolve the two fitted-mean comparisons independently
before ranking them. Do not revive rejected early-checkpoint schedules or fit
thread/size guards to the serial and control losses. Current implementation
priorities belong in STATUS.md.

Frozen 77/16/111 definitions, static allocation-inventory source drift,
constrained weighted-fit/nonretaining-CG issues and deferred Linux/physical-memory
work are unchanged. No universal speed, platform or qualification claim follows.

## Immutable evidence

`receipts-content-addressed.zip` contains **11,862 logical receipts**, deduplicated
into **2,612 payloads**, 28,878,841 bytes. SHA-256:
`de01092cd823b407cc239b8cda2315636f1984eafe68abdd85a7d65e96d48687`.
The adjacent hash and `archive-verification.json` bind the container. Restore to
a new directory with `python restore_receipts.py DESTINATION`; restoration verifies
the container, logical paths and every payload hash. Closeout restored all entries
and checked for local-home and license-header leakage.

Under restored `ppml-frame-gap-20260925/`, `verification.json` and `summary.json`
record counts, source identities, complete timings, paired results, phase metadata
and memory deltas. Each block includes plan/progress/complete receipts, monitors,
exports, public-state and comparison audits. Separate direct-stock audits retain
all discrepancies. `artifact-sources/` preserves the five source snapshots;
patches, diagnostic construction, symbolic analysis, build/test logs, harness
recipes, reference pins and fixture protocols are included. Archived scripts
with tokenized local paths require local path configuration before rerunning.

Raw licensed logs, datasets and plugin binaries are omitted with original hashes.
Fixture protocols preserve return codes/sentinels without license material.
Sanitized receipts record both original and archived hashes; they do not rewrite
historical source or imply earlier artifacts contained this change.
