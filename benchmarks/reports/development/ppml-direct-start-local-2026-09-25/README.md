# Direct first-partition PPML initialization

Local development evidence, 2026-09-25; not qualification. Adopt **directstart1**
under [ADR 0213](../../../../docs/adr/0213-direct-first-partition-ppml-start.md).
The multi-FE 0.90-times-comparable-stock objective remains unfinished.

## Source and computational change

Baseline **familyframe1**, accepted at **fd92efc2**, plugin SHA-256
`a88eb33da856f021563105040e90a08ede50797e901cadce9b452629cab02328`.
Candidate **directstart1** is fd92efc2 plus the single Rust overlay
`ppml/prepared_irls.rs`, plugin SHA-256
`aab4f8976e164d0f48c18d1c0f6948cd0ebe435a9cc28bf12c017b317e16a832`.
All **566 computational inputs** match the measured candidate. Source snapshots,
builds, ABI checks, controllers, declarations and numerical receipts are archived.

For cold publication starts with at least two FE partitions, each containing an
intercept, compute the first conditional group optimum directly from original
outcomes, prior weights and offsets. A preceding global intercept cancels
analytically. Reuse the guarded one-partition arithmetic, then execute the same
remaining intercept and profiled-slope proposals in the same order. On refusal,
compute the former global start lazily and overwrite the complete provisional
predictor and constant coefficient. The initial finite constant may use a zero
gauge; canonical solving independently establishes the reported normalization.

Explicit starts, Mu exploration, no FE, one FE and slope-only partitions keep
their former routes. No acceptance check, tolerance, sample, weight rule, rank,
DoF, VCE, separation policy, memory allowance or transaction boundary changes.
Original-input projection/model-space/score/objective/reconstruction checks and
exact final q*mu Hessian certification remain authoritative. This adds no bank,
cache, pool or OLS implementation change.

The stock fixest archive is unmodified, SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Use glm.tol=1e-10 and fixef.tol=1e-11, matched threads, BLAS one, robust or declared
clustered inference and fitted means. Command clocks include loading, estimation
and requested output publication, excluding harness import/setup. No task-owned
compilation, audit, profiling, archiving or competing fit ran beside headline
clocks. Per-fit process monitoring and all interruptions remain visible.

## Independent attribution

Separately instrumented **startprofile1/startprofile2** supply two clean paired
rotations, not stock headline timings. On two-FE N983040/K6/T8 fast, initialization
falls **53.58 -> 20.90 ms**. Global outcome/offset log sums cost 16.42/9.47 ms in
the baseline and disappear from the admitted candidate. Conditional multi-FE
seeding falls 26.70 -> 20.40 ms; the direct first-group helper costs 5.60 ms.
These nested measurements are not additive. Plugin estimation is 913.59/878.10 ms
in this diagnostic build.

The unbalanced three-FE/four-slope N38400/K6/T8 low diagnostic reduces initialization
6.80 -> 5.42 ms and plugin estimation 118.30 -> 114.88 ms. It does not establish
a public gain on every four-slope case. Geometry worker durations sum to about
44 ms but overlap: their interval-union wall time is about 5 ms, consistent with
the parent geometry preparation boundary. Do not report the worker sum as wall
cost or the next dominant opportunity.

## Public rotating distributions

B=familyframe1, C=directstart1, S=comparable pinned stock. All complete repeated
pilot/fresh blocks have clean monitors. Medians are ms; full ordered series,
paired ratios, losses and OLS clocks are in [distributions.md](distributions.md).
The original interrupted pilot remains separate and does not supply medians.

|Development case; N/K/T|B / C / S|C/S paired range|
|---|---:|---:|
|Two-FE numeric fast;983040/6/8|945 /914 /868|1.048–1.053|
|Same low memory|1044 /1012 /868|1.157–1.171|
|Unbalanced three-FE factors, cluster;153600/19/8|584 /581 /709|0.819–0.824|
|Unbalanced three-FE, two slopes;61440/10/8|156 /161 /231|0.644–0.704|
|Two-FE, two slopes, low;30720/6/4|74 /74 /119|0.613–0.630|
|Two-FE numeric serial;245760/6/1|561 /553 /943|0.582–0.589|
|Small unbalanced three-FE;30720/6/4|74 /74 /82|0.890–0.926|
|One-FE protected control;61440/6/4|61 /61 /unranked|Mean discrepancy|

Three rotations give **15 wins, six losses and three ties** against baseline.
Four of seven multi-FE development families meet 0.90 in every pair. Million-row
fast/low gains are **3.3/3.1%**. Retain the slope loss and small-case variability.

Fresh **seed 202609443**, declared before validation, uses five rotations and no
subsequent source/guard tuning. This seed is consumed.

|Fresh case; N/K/T|B / C / S|C/S paired range|
|---|---:|---:|
|Two-FE numeric fast;737280/6/8|717 /692 /unranked|Mean discrepancy|
|Matched low memory|794 /766 /unranked|Mean discrepancy|
|Two-FE numeric fast;368640/6/4|491 /477 /660|0.722–0.728|
|Unbalanced three-FE factors, cluster;184320/19/8|696 /684 /3221|0.210–0.215|
|Unbalanced three-FE, two slopes;76800/10/8|200 /195 /296|0.641–0.678|
|Unbalanced three-FE, four slopes, low;38400/6/8|119 /119 /119|0.975–1.017|
|Two-FE numeric serial;245760/6/1|562 /554 /938|0.586–0.596|
|Weak_d3;61440/6/4|126 /124 /855|0.144–0.148|

Fresh B/C outcomes are **35 wins, two losses and three ties**. Five of six fresh
families with scientifically comparable stock clocks meet 0.90 in every pair.
The two largest numeric families independently confirm **3.5%** improvements
against fereg's baseline; they do not support a stock speed ranking. Raw stock
medians 761/760 ms remain in the distributions. The large factor advantage is
specific to the captured design and seed, not a universal fourfold claim.

## Science, failures and controls

**115 distinct focused Rust tests pass**: 57 prepared-IRLS and 58 estimator.
The IRLS module also has one failure in the pre-existing dynamic-range strict
checkpoint-count test (expected one rejection, observed zero). A fresh accepted
baseline build in a separate Cargo target reproduces the same failure. Preserve
it; this checkpoint does not repair it. New independent initialization tests cover
2/3/4 partitions, extreme varying offsets, changing weight scales, dependent
slopes, an analytically specified Poisson optimum, later-partition refusal and
complete restoration. Existing explicit-start, slope-score and boundary tests pass.

**Seven original licensed fixtures pass**: runtime validator, hidden transaction,
72-fit categorical final oracle, three-partition slope oracle, higher-dimensional
oracle, public scientific contract and weight/inference. Build, ABI and panic
containment self-test pass. No qualification campaign was run.

All **223 completed PPML public states** pass original-input sample, score,
model-space and objective checks: 209 production and 14 diagnostic states,
including six preparation-attribution fits. The four small cells supply **12
explicit-design Poisson/Hessian/sandwich arm checks**. All **73 completed B/C
pairs** pass, including four diagnostic pairs; numerical bit identity is not a
requirement for this change. Anchored comparisons pass 128/142, with 46 further
planned pairs incomplete in the interrupted pilot.

Direct C/stock comparisons, including requested identified outputs, pass **56/70**;
22 further planned comparisons are incomplete. Fourteen discrepancies remain:

- The small full-output case misses four residual/output coordinates, maximum
  absolute error 8.13e-9, tolerance ratio 2.667. Both baseline and candidate show it.
  The initial direct core/mean-only audit is supplemented by the archived full-output
  audit; its narrower pass is not full-output acceptance.
- The protected one-FE case misses five mean coordinates in each of three rotations,
  maximum error 7.37e-7, tolerance ratio 1.667.
- Fresh fast/low N737280 miss six mean coordinates in each of five rotations,
  maximum errors 3.89e-7/3.87e-7, tolerance ratios 1.089/1.078. Coefficients, VCE,
  objectives and each arm's independent state checks pass. These stock comparisons
  remain unresolved and unranked; no tolerance or sample is changed.

**12 OLS coefficient/covariance pairs are exact.** Control medians B/C are 64/66,
76/75, 14/14 and 210/210 ms. Keep the first control's small loss; do not claim
universal performance non-regression. The OLS implementation is unchanged.

Four pre-measurement stops and the interrupted five-fit pilot are retained.
The pilot's second stock call detected a foreign Stata overlap; one complete
clean first rotation is insufficient for distributional adoption. The unchanged
complete repeat supplies the reported pilot. No unrelated process was stopped.

Tracked peak, observed/modelled scratch and allocation-peak deltas are zero in
selected complete production comparisons; modeled bounds vary -48..+48 bytes.
No new observation bank or allowance. No physical-cap, platform or constrained
memory qualification. Static allocation inventory drift, the existing hidden
allocation fixture's wire-minor assertion mismatch, weighted constrained-fit and
nonretaining-CG failures remain open. Frozen 77/16/111 definitions are unchanged.

## Measured next opportunity

Separately instrumented **inputprofile1**, based on accepted fd92efc2, runs two
clean repetitions on each of million-row fast/low and the wide clustered factor
case. It changes no production behavior. Million-row core preparation is
60.3/60.6 ms: borrowed metadata is negligible, allocation-free numeric classification
26.3/26.4 ms, FE cardinality preflight 4.2/4.3 ms, post-admission selection
29.2/29.3 ms and owner transfer under 0.01 ms. Initial generation construction is
separate at 50.6/50.9 ms, including shared categorical preparation 47.8/48.2 ms.
Wide-factor classification/selection cost 8.1/8.6 ms; generation construction
33.2 ms. Do not add nested classifier or model-builder durations twice.

Source inspection confirms the post-admission selection repeats the authoritative
numeric classifier. A borrow-bound classification token can preserve immutable
input/role/weight/offset identity while constructing the existing row map and drop
ledger without repeating all numeric validation when every candidate survived.
Actual-drop paths must preserve their existing ledger and error precedence;
pre-allocation admission remains before materialization. Scalar counts alone are
insufficient cache authority. This is the next bounded implementation, not an
optimization already adopted here. Initial model construction requires further
subdivision before deciding which of its checks or copies can share ownership.

Current development large fast/low medians still need about **133/231 ms** to reach
0.90 of comparable stock. Removing the measured selection repetition alone will
not close those deficits. Four-slope low remains near stock, with canonical solves
about 45 ms and MAP absorption about 32 ms in the separate slope diagnostic.

## Receipt archive

The content-addressed archive includes complete source identities, controllers,
monitor revisions, raw ordered timings, independent audits, test outcomes,
separate diagnostic sources/traces and the next-preparation attribution. Generated
DTA/binary row outputs and compiled artifacts are hash-bound and retained locally;
licensed logs are reduced to protocol/sentinel lines. Paths are sanitized. Restore
and verify with `restore_receipts.py NEW_DIRECTORY`; container SHA-256 and verified
counts are recorded in the adjacent receipt files.
