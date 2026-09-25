# PPML opportunity refresh and accepted-point witness closeout

Local Mac arm64 development evidence, 2026-09-25. No qualification or release.
Decision: [ADR 0192](../../../../docs/adr/0192-accepted-point-witness-closeout.md).
The production algorithm is unchanged. The accurate early-checkpoint prototype
is rejected for public performance; its source and all evidence are preserved.

## Source and scope

The refresh uses **resgate1**, accepted at `b8f04a23`. All **556 captured inputs**
match the working source at `3552cd75` and again after prototype removal.
**goalacceptedwitness1** overlays only `ppml/prepared_irls.rs` on `3552cd75`.
**goalframeledger1** is an isolated instrumented copy of that baseline, with 557
captured inputs. Diagnostic clocks never substitute for stock headline timings.
Both new artifacts pass the actual-plugin ABI/self-test and panic-containment
harness. Identity maps, captured sources and build logs are archived.

| Artifact | Plugin SHA-256 |
|---|---|
|resgate1|9103c3092e83291e2941634ac07de06d445bfec85c39ca372a3b96f833f166d9|
|goalacceptedwitness1|073063aa196f8b80796a2b63382e23c0173587d175ddf092974e55ad74175ec6|
|goalframeledger1|22a309fcfb8154e2ec8f500008ae7cd7a1f6c0391a555b98bbc6d78f979b583f|

Pinned stock fixest is unmodified **0.14.2**, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Use `glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads, robust inference and
saved fitted means; BLAS/OMP/vecLib threads are one. This screen uses positive,
unweighted outcomes and numeric regressors. Samples, objective, coefficient,
covariance and mean comparisons are audited before ranking. It does not establish
weight, separation, factor/generated-term, cluster or other-output performance.

All blocks reuse development seed **202609347**. Previously frozen validation
seeds are not tuning inputs; reserved seeds **202609428/429** remain unused.
Generation/import/process startup/warm fits are outside the command clock;
public loading, estimation, certification, inference and outputs are inside.
The first seven-case refresh is followed by three distinct historical boundaries,
not repeated favorable reruns. The larger disconnected low case is added to the
candidate declaration before any candidate headline timing.

## Current-source opportunity inventory

Three baseline/stock rotations per case. Vectors list repetitions 0, 1, 2 in ms.
Each table row belongs to its own captured block; do not pool it with another
run or transfer it to differently sized fresh cases.

| Block/case | Design; N/K/T; storage | Fereg ms | Stock ms | Median F/stock | Target met? |
|---|---|---|---|---:|---|
|screen/0|Two categorical; 12000/3/1; fast|30,30,30|34,36,39|0.833|yes|
|screen/1|Disconnected two categorical; 20160/12/4; fast|49,48,48|64,63,62|0.762|yes|
|screen/2|Unbalanced three categorical; 23040/8/4; fast|73,72,72|82,83,84|0.867|yes|
|screen/3|Wide two categorical; 16320/32/8; low|65,68,66|88,86,87|0.759|yes|
|screen/4|Three categorical; 14400/7/4; low|51,51,52|119,120,117|0.429|yes|
|screen/5|Two slope partitions; 30720/6/8; fast|81,81,83|78,78,78|1.038|no|
|screen/6|Narrow two categorical; 21120/2/4; fast|38,39,38|27,27,27|1.407|no|
|appendix/0|Narrow two categorical; 21120/2/4; low|36,37,36|32,29,27|1.241|no|
|appendix/1|Disconnected two categorical; 42240/10/8; low|97,99,98|77,76,77|1.273|no|
|appendix/2|Dense three categorical; 61440/10/8; fast|247,253,246|504,499,507|unranked|unresolved science|

Five of nine scientifically comparable medians meet the 0.90 target. The four
misses require the following reductions on these inputs:

| Case | Fereg / stock median | 0.90 stock | Required fereg reduction |
|---|---:|---:|---:|
|Disconnected low categorical|98 / 77 ms|69.3 ms|28.7 ms / 29.3%|
|Narrow fast categorical|38 / 27 ms|24.3 ms|13.7 ms / 36.1%|
|Narrow low categorical|36 / 29 ms|26.1 ms|9.9 ms / 27.5%|
|Threaded two slope partitions|81 / 78 ms|70.2 ms|10.8 ms / 13.3%|

This is additional current-artifact evidence, not a replacement for the prior
[resgate1 fresh block](../ppml-residual-gate-local-2026-09-25/README.md), where
eight of ten larger/fresh cases meet the target. Previously successful weak,
multiple-slope, serial, unbalanced and four-FE anchors are protection controls.
Fast disconnected success does not establish low disconnected performance.

The dense three-FE discrepancy recurs in all three comparisons: two coefficient
coordinates fail, maximum absolute error **2.1222049996234144e-10**, maximum
tolerance ratio **1.441860854303519**. Covariance, means, objective and sample
comparisons pass. Mean maximum error is **9.249556676138582e-8**, tolerance ratio
0.1662. Both packages' public states pass the independent model-space, score and
objective audits. This does not adjudicate the coefficient disagreement; neither
timing ratio nor speed win is assigned. No comparison envelope is changed.

## Diagnostic work budget

Two diagnostic/baseline rotations on each of the two leading initial misses,
then two on disconnected low. All 12 public states and six baseline comparisons
pass. Below are medians of totals from the final full plugin invocation, excluding
warm fits. Spans are **inclusive**; rows nest and must not be added. Parallel QR
worker sums, which exceed wall time, are deliberately absent from this table.

| Span | Two slopes | Narrow fast categorical | Disconnected low categorical |
|---|---:|---:|---:|
|Whole plugin|74.779|32.260|94.016|
|Loading|2.641|1.812|3.642|
|Core preparation|2.341|1.071|3.485|
|Initial generation/model preparation|6.373|1.183|3.064|
|IRLS including initialization and checkpoints|54.247|21.918|70.715|
|Initial state|4.980|2.121|3.980|
|Initial/final canonical preparation, two calls|19.725|7.316|31.511|
|Full retained rank work, inside canonical preparation|5.884|3.005|9.487|
|Ordinary direction boundary, three calls|16.950|5.065|21.029|
|Inner joint solve, inside ordinary boundary|9.201|—|11.188|
|Ordinary recurrence, inside solve|3.865|2.731|5.325|
|Strict candidate, overlaps canonical preparation|10.073|2.755|12.664|
|Fresh outer scores, six calls|2.340|1.248|2.853|
|Independent result certification|3.863|2.688|5.405|
|Inference|0.726|0.321|1.349|
|Plugin publication|0.940|0.656|1.320|

All three fits take four accepted outer iterations. Narrow categorical uses its
augmented LSMR direction; low categorical uses the existing streamed coefficient
owner. The latter has about **9.84 ms** between the ordinary boundary and its
inner streamed solve. This difference includes preparation, independent checks
and reporting; the instrumentation does not allocate it all to certificates.
Its initial retained-design construction is 13.447 ms and finite rank work 5.107
ms, nested in the canonical budget. These point to the complete current-frame
lifecycle as the next implementation boundary. Eliminating the narrow recurrence
entirely would still fall far short of its 13.7-ms headline target gap.

Source inspection confirms existing direct original-column scans and scalar
reported-intercept assembly already avoid some plausible redundant NxK work.
The remaining work must be localized before attributing new savings. Existing
geometry/coupling and row-tiled reconstruction mechanisms are already active;
this ledger is not evidence for simply removing their guards.

## Accurate earlier accepted checkpoint: implementation and rejection

Unlike ADR 0191's incorrect older-point publication, this prototype runs strict
Newton after the first ordinary convergence pass and, if its existing strict
point/basis comparison passes, **accepts the new point** through normal bounded
objective line search. A fresh score and relative-deviance check must establish
the second pass at that accepted point. The iteration cap counts the real step.
The final Hessian is rebuilt/certified under its exact changed `q*mu`; same-weight
reuse of the preceding point's products is prohibited. Refusal retains bounded
refinement/resume and existing fail-closed behavior. No tolerance is loosened.

The final focused Rust run has **54 passes and one known baseline count-assertion
failure**, `refined_checkpoint_preserves_accuracy_on_a_small_dynamic_range_model`
(expected strict rejection count 1, actual 0). The four accuracy failures of the
earlier experiment now pass. A new analytic weighted-slope test exercises fast/
low, T1/T4, actual early acceptance, fresh score/objective reports, exact final
weights and iteration-cap refusal. The existing same-point reuse test explicitly
disables the new mechanism to continue testing its own boundary; no truth or
reuse assertion is relaxed. The new test covers the accepted-point path.

Three licensed independent fixtures pass, each with process RC=Stata RC=0 and
an explicit `END_FIXTURE` sentinel:

- `feppml_multipartition_slope_oracle.do`
- `feppml_three_partition_slope_oracle.do`
- `feppml_canonical_products_oracle.do`

Three baseline/candidate/stock rotations then reject the candidate:

| Case | Baseline ms | Candidate ms | Stock ms | B / C / stock medians | Paired C W/L/T |
|---|---|---|---|---|---|
|Two slopes 30720/K6/T8 fast|80,76,77|103,105,101|77,79,80|77 / 103 / 79|0/3/0|
|Narrow categorical 21120/K2/T4 fast|37,36,39|39,37,37|32,31,27|37 / 37 / 31|1/2/0|
|Disconnected categorical 42240/K10/T8 low|97,98,97|106,105,108|78,77,80|97 / 106 / 78|0/3/0|

All 27 public states, 18 baseline comparisons and nine direct candidate/stock
comparisons pass. There is **one paired win and eight losses**. No fresh holdout
is spent on this losing prototype. Baseline timings vary between the inventory
and pilot; all distributions remain visible instead of pooling favorable runs.

On two slopes, strict/final phase time rises from median **10.037 to 30.347 ms**
and outer iterations from four to five. A separate one-fit solver trace explains
why: the first-pass accepted point has score 2.74409e-10, but strict Newton moves
eta by **1.19664993469825731e-8**, failing the unchanged point-consistency rule.
The existing recovery retires the shortcut, tightens inner tolerance from 1e-12
to 1e-14, resumes iterations and certifies final products. Basis/rank do not
change. The warm N7680 fit does accept the early point; it must not be mistaken
for the full N30720 case. The final trace state independently passes public audit.

On narrow categorical, ordinary projection falls but strict/final rises from
3.317 to 7.056 ms; on disconnected low it rises from 13.374 to 28.532 ms. Both
retain four outer iterations. Accurate changed-point acceptance requires current-
weight work that defeats this particular scheduling shortcut. This is not proof
that current-frame ownership or a different canonical solver cannot help.

No observation bank or memory credit is added. All nine pilot pairs have equal
reported scratch and tracked allocation peak. Selected modeled bytes are equal
except one narrow pair with **+48 bytes**. No physical-memory-cap or separate
warmed-allocation admission claim is made. Since the candidate is rejected,
there is no new OLS control campaign or production route to qualify.

## Completeness, failures and restoration

There are **87 headline applications** (60 inventory and 27 pilot), plus 12
instrumented-ledger/baseline applications and one candidate-only solver trace.
All **100 public states** pass. Of 54 baseline/stock-or-candidate comparisons,
51 pass and the three dense-three-FE coefficient discrepancies remain unranked.
Nine additional direct candidate/stock comparisons pass. All process monitor
records show success, no sampled overlap and no monitor error. This is not a
claim of zero ambient OS load. Builds, profiling and archiving did not overlap
headline benchmarks. No application failure or timeout is hidden.

Preserved development failures include the initial diagnostic build name collision
(before alteration of the existing artifact), the first profile parser attempting
to read an uninstrumented baseline, an intermediate test-only compile error from
`unwrap_err` requiring Debug, and the initial 52-pass/two-failure candidate test
run before the same-point reuse test was scoped correctly. The final 54/one run
and the already documented baseline failure are reported separately. These are
development failures, not failed headline fits.

The candidate patch SHA-256 is
`217443cd49307aa84be465f01cc84dace92beb558281bde3483adc3074cbf323`;
its edited source SHA-256 is
`e09d8c8fc2a3c824749cd1eae650b0f2017e9131b9a64c82a36ed3bfd2b29151`.
The production file is restored exactly, with all 556 resgate1 inputs verified.
The known allocation-inventory drift, constrained weighted fits and nonretaining
CG issues are unchanged. No SCC attempt, stopped campaign, push or release occurs.

## Replaying receipts

The content-addressed archive contains **4,342 logical receipts** in 1,315
payloads, 10,636,181 bytes. SHA-256:
`f26ed4aa10b21937e13f5d8b4263ab1c3311dcea665de6c4399c1724d571cf4d`.
`restore_receipts.py NEW_DIRECTORY` verifies the container and each restored file.
Restoration of all logical files was checked. Original and sanitized hashes are
retained; machine/home paths are replaced, licensed logs retain protocol lines
only, and data/means/plugin binaries are omitted with hashes.

Restored roots:

- `ppml-opportunity-refresh-20260925`: declarations, complete application/monitor
  receipts, science audits, summaries, diagnostic spans and build/runner recipes.
- `ppml-accepted-witness-20260925`: exact candidate patch/source, captured artifact
  inputs and identities, unit logs, licensed protocols, pilot and trace receipts,
  direct stock audit, summary, verification and closeout/archive recipes.

This closeout accepts no new runtime optimization. The evidence identifies
disconnected low-memory canonical/ordinary frame work as the largest measured
remaining local opportunity. The revised implementation plan lives in STATUS.md.
