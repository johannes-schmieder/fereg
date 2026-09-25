# Paired coefficient operators for public PPML — 2026-09-25

**Rejected for production** under [ADR 0181](../../../../docs/adr/0181-paired-slope-coupling-closeout.md).
The accepted computation remains `711802bf`; this experiment started from
`21280286`. All 554 accepted `familyrows2` inputs were verified unchanged after
restoring the worktree. The paired implementations are preserved as evidence,
not as dormant production code. The 10%-faster multi-FE goal remains unfinished.

## What was implemented

Independent CG columns share current-weight dense-Schur or compact coupling
loads in coordinate-major pairs. The native prototype also tests four lanes.
Each column keeps its own recurrence, restart, exact observation-space curvature
fallback, fresh reconstruction and stopping. Full original-input certification
is unchanged. Finished and padded lanes remain measurable work, never a reason
to accept another column. No samples, weights, ranks, DoF, score, objective,
reconstruction, final-Hessian, inference or transaction requirements change.

The production candidates retain four coefficient banks and a small eliminated-
channel scratch per pair worker, with separate optional memory admission after
existing workspaces. They keep the scalar path when RHS count fits within the
worker count. Existing observation buffers and normalized storage remain; no
new N-by-K bank or coefficient-result cache is introduced. Low-memory ordinary
joint iterations retain their observation buffers and original-control access.

## Native complete-projection experiment

Eight frozen frames reuse tuning seed 202609347: N61440, two/three partitions,
K6/K10, two coherent positive weights/working-response states, T1/T4/T8. The
first two partitions have 439 and 615 groups with intercept plus slope; the
third has 31 intercept groups. Frame0 uses accepted means; frame1 perturbs eta
by 0.35*x1+0.15*sin(g1), then recomputes weights and working responses. Removing
the third partition changes the WLS problem, not a public Poisson fit claim.
All eight frames use **compact coupling**. They do not cover the dense-Schur
kernel used by many ordinary two-partition public designs.

Five rotating B/C2/C4 sequences per configuration yield 360 calls. Immutable
owner allocation is outside all clocks. Current geometry, normalization,
coupling preparation, batch allocations/copies, solving, physical residual
output, full fresh certificates and direct Gram are inside. This is complete
dense projection/product work, not complete canonical rank or public PPML.
Counters are present in all native variants; they are not production telemetry.
A sampled process monitor reports no overlap/errors. No build, oracle, profile
or archive ran alongside these or the public headline blocks.

| Frame | T | Baseline ms | Two lanes ms | C2/B | Four lanes ms | C4/B |
|---|---:|---:|---:|---:|---:|---:|
|e2-k10-f0|1|68.267 [68.028,69.330]|50.126 [49.140,50.267]|0.734|39.405 [38.750,40.961]|0.577|
|e2-k10-f0|4|24.461 [24.314,24.524]|21.894 [21.683,22.577]|0.895|17.874 [17.591,18.001]|0.731|
|e2-k10-f0|8|19.829 [19.270,20.107]|15.704 [15.163,16.639]|0.792|16.623 [16.524,17.301]|0.838|
|e2-k10-f1|1|69.386 [68.385,70.313]|50.321 [49.746,50.649]|0.725|39.647 [39.303,42.558]|0.571|
|e2-k10-f1|4|26.938 [26.470,28.110]|21.858 [21.474,22.800]|0.811|17.955 [17.762,18.669]|0.667|
|e2-k10-f1|8|20.460 [20.359,21.628]|14.862 [14.727,15.728]|0.726|17.359 [16.830,17.987]|0.848|
|e2-k6-f0|1|44.531 [44.207,46.031]|34.023 [33.589,35.640]|0.764|26.679 [26.348,27.264]|0.599|
|e2-k6-f0|4|18.433 [18.385,18.978]|13.438 [13.154,14.992]|0.729|15.084 [14.906,16.765]|0.818|
|e2-k6-f0|8|12.909 [12.775,13.341]|13.344 [12.990,13.486]|1.034|15.264 [14.604,16.604]|1.182|
|e2-k6-f1|1|45.013 [44.528,45.699]|33.642 [33.507,34.318]|0.747|26.866 [26.559,28.407]|0.597|
|e2-k6-f1|4|18.700 [18.657,19.236]|13.568 [13.241,15.164]|0.726|15.243 [15.177,15.631]|0.815|
|e2-k6-f1|8|13.288 [12.483,13.523]|13.684 [12.654,14.635]|1.030|14.782 [14.706,15.146]|1.112|
|e3-k10-f0|1|113.397 [112.891,114.666]|78.730 [77.352,80.739]|0.694|57.629 [57.191,58.023]|0.508|
|e3-k10-f0|4|43.091 [41.599,44.370]|32.346 [31.768,33.980]|0.751|25.180 [24.790,25.533]|0.584|
|e3-k10-f0|8|33.831 [33.663,36.133]|21.942 [20.937,22.678]|0.649|23.622 [23.562,24.439]|0.698|
|e3-k10-f1|1|113.749 [112.403,115.819]|77.794 [77.018,78.782]|0.684|58.581 [57.320,58.968]|0.515|
|e3-k10-f1|4|41.892 [41.326,42.617]|33.547 [32.941,35.232]|0.801|25.242 [24.742,26.735]|0.603|
|e3-k10-f1|8|34.141 [33.748,36.992]|22.262 [20.968,22.551]|0.652|24.244 [23.765,24.563]|0.710|
|e3-k6-f0|1|74.220 [73.615,76.148]|52.615 [51.913,54.272]|0.709|39.226 [38.977,39.951]|0.529|
|e3-k6-f0|4|28.698 [28.237,30.389]|20.194 [19.826,20.909]|0.704|22.402 [21.893,23.570]|0.781|
|e3-k6-f0|8|21.574 [21.146,21.982]|19.716 [18.554,20.801]|0.914|21.656 [21.467,23.101]|1.004|
|e3-k6-f1|1|73.469 [73.038,74.327]|53.104 [51.960,54.550]|0.723|39.355 [38.970,40.212]|0.536|
|e3-k6-f1|4|28.507 [28.298,28.910]|20.285 [19.159,20.602]|0.712|22.310 [21.874,22.858]|0.783|
|e3-k6-f1|8|21.189 [21.053,21.662]|18.887 [18.399,20.498]|0.891|21.146 [20.936,22.061]|0.998|

Two lanes improve 22/24 medians but lose 8/120 paired calls. Both candidates
pass all 72 independent explicit-design configurations in total. All 144
candidate within/Gram/scale export files equal baseline bitwise. All 240 B/C
pairs have the same maximum and total column iterations, reconstructed checks,
useful lane applications and fallbacks (zero on these frames). Two lanes
perform 19,170 evaluated lane applications, including 2,055 inactive/padded;
four lanes perform 19,380 including 2,265 inactive/padded. Thus reduced traversal
counts do not imply less scalar arithmetic everywhere.

The separate oracle constructs explicit sparse FE/slopes and equilibrated
SciPy LSMR (atol=btol=2e-15). Projection error <=1e-10, original FE scores
<=1e-12, independent/replayed Gram <=1e-10, finite coefficients <=1e-7 and
inverse relative error <=1e-6 all pass. The analytic fixture covers weighted
nonzero residuals, redundant slope channels, changing weights, zero/pure-FE
columns and scales 1e-60..1e60. It is one parameterized test function, not a
claim of dozens of distinct tests. An initial compilation failure from a
private test accessor is retained with its correction and source hashes.

## Public implementations and measured attribution

`batchrhs2` generalized the operator over scalar and paired lanes in flat
storage. Its early large fast median was 222 versus baseline 229 ms; low was
240 versus 244 ms. Separate matching `pairtracebase`/`pairtrace2` builds show
that pairing actually runs on the public canonical calls. For the large fast
case, scalar initial and strict coefficient solves take roughly 9.5 and
16–17 ms, while paired calls take 7.4–7.9 and 12–13.4 ms. These actual initial/
reprojected RHSs are cheaper than the cold frozen frames; source organization
and different inputs matter. The diagnostic does not establish equal iteration
work to the cold experiment.

That generic refactoring also slows ordinary large-case joint iteration spans
by about 8–10%: fast 31–32 to about 34 ms total, low about 25 to 28 ms. The
original scalar kernel is restored in `batchrhs3`, and paired state returns to
fixed arrays. Fresh409 then exposes the dense paired kernel's insufficient
independent accumulators: the serial public case loses 4.5%. `batchrhs4` restores
four-row dense unrolling while sharing each matrix load between two lanes.
No tolerance or case-specific threshold changes. Scalar code remains original.
Diagnostic spans are inclusive, separately instrumented and never substituted
for stock headline timings. All 12 diagnostic public states pass.

Headline public calls include robust inference and saved means. The stock
reference is unmodified pinned fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with glm.tol=1e-10, fixef.tol=1e-11 and per-case OpenMP/estimator thread settings.
These positive unweighted numeric comparisons use robust N/(N-1) inference.
Frequency-weight inference is checked separately by the explicit licensed
oracle, not by treating ordinary fixest weights as a Stata frequency oracle.
Default-reference accuracy failures from earlier work remain separate evidence.

## Frozen first fresh block — rejected regression retained

Seed 202609409, five rotations, artifact `batchrhs3`. Times are median
[min,max] ms; the candidate is not the accepted runtime.

| Case | N/K/T | Memory | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Paired losses |
|---|---|---|---:|---:|---:|---:|---:|---:|
|0: dense / slopes|34560/6/4|fast|120.000 [120.000,123.000]|122.000 [121.000,124.000]|124.000 [120.000,125.000]|1.017|0.984|3/5|
|1: unbalanced / slopes|69120/12/8|low|332.000 [328.000,334.000]|322.000 [322.000,323.000]|284.000 [278.000,300.000]|0.970|1.134|0/5|
|2: unbalanced / slopes|69120/12/8|fast|290.000 [288.000,294.000]|280.000 [279.000,284.000]|288.000 [286.000,291.000]|0.966|0.972|0/5|
|3: dense / slopes|38880/10/1|fast|223.000 [222.000,232.000]|233.000 [230.000,238.000]|572.000 [570.000,573.000]|1.045|0.407|5/5|
|4: weak_slope_barbell / slopes|23040/6/4|fast|70.000 [70.000,71.000]|71.000 [71.000,73.000]|2277.000 [2273.000,2278.000]|1.014|0.031|4/5|
|5: dense / multiple slopes|36480/8/4|fast|170.000 [169.000,172.000]|173.000 [171.000,174.000]|245.000 [244.000,247.000]|1.018|0.706|4/5|
|6: disconnected / slopes|32160/6/4|fast|132.000 [131.000,134.000]|131.000 [131.000,133.000]|137.000 [132.000,139.000]|0.992|0.956|1/5|
|7: dense / categorical|25920/2/4|fast|44.000 [43.000,45.000]|44.000 [44.000,45.000]|33.000 [32.000,34.000]|1.000|1.333|1/5|
|8: dense / slopes|29520/6/8|fast|102.000 [99.000,104.000]|102.000 [100.000,103.000]|72.000 [71.000,72.000]|1.000|1.417|1/5|

## Final correction: tuning screen

Seed 202609347, three rotations, artifact `batchrhs4`. The serial control is
added before the final fresh block; it does not reuse fresh409 as tuning data.

| Case | N/K/T | Memory | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Paired losses |
|---|---|---|---:|---:|---:|---:|---:|---:|
|0: dense / categorical|21120/2/4|fast|38.000 [38.000,40.000]|40.000 [38.000,41.000]|29.000 [26.000,31.000]|1.053|1.379|2/3|
|1: disconnected / categorical|42240/10/8|fast|88.000 [88.000,89.000]|90.000 [88.000,90.000]|77.000 [73.000,79.000]|1.023|1.169|2/3|
|2: unbalanced / slopes|61440/10/8|fast|227.000 [225.000,232.000]|222.000 [221.000,224.000]|231.000 [228.000,231.000]|0.978|0.961|0/3|
|3: unbalanced / slopes|61440/10/8|low|243.000 [241.000,245.000]|235.000 [234.000,240.000]|231.000 [230.000,231.000]|0.967|1.017|0/3|
|4: dense / slopes|30720/6/4|fast|110.000 [110.000,112.000]|109.000 [108.000,111.000]|119.000 [118.000,119.000]|0.991|0.916|1/3|
|5: dense / slopes|38880/10/1|fast|245.000 [242.000,245.000]|239.000 [239.000,244.000]|573.000 [570.000,574.000]|0.976|0.417|0/3|

## Final independent fresh block and decision

Seed 202609410 uses new sizes and widths, five rotations and the same declared
families. Every cell is scientifically rankable; no timeouts, application
failures, isolation errors or discrepancies are hidden. Times below refer to
the tested, **rejected** candidate. Baseline is the currently accepted runtime.

| Case | N/K/T | Memory | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Paired losses |
|---|---|---|---:|---:|---:|---:|---:|---:|
|0: dense / slopes|36960/8/4|fast|137.000 [136.000,137.000]|136.000 [134.000,137.000]|162.000 [159.000,166.000]|0.993|0.840|1/5|
|1: unbalanced / slopes|74880/10/8|low|322.000 [321.000,323.000]|314.000 [312.000,319.000]|288.000 [282.000,289.000]|0.975|1.090|0/5|
|2: unbalanced / slopes|74880/10/8|fast|302.000 [298.000,320.000]|293.000 [290.000,306.000]|286.000 [285.000,293.000]|0.970|1.024|1/5|
|3: dense / slopes|43200/12/1|fast|278.000 [278.000,283.000]|274.000 [274.000,276.000]|706.000 [705.000,709.000]|0.986|0.388|0/5|
|4: weak_slope_barbell / slopes|24960/8/4|fast|79.000 [79.000,81.000]|80.000 [79.000,83.000]|2744.000 [2742.000,2759.000]|1.013|0.029|2/5|
|5: dense / multiple slopes|38880/10/4|fast|193.000 [192.000,194.000]|196.000 [195.000,196.000]|295.000 [293.000,297.000]|1.016|0.664|5/5|
|6: disconnected / slopes|34560/8/4|fast|160.000 [158.000,161.000]|159.000 [157.000,162.000]|217.000 [216.000,217.000]|0.994|0.733|3/5|
|7: dense / categorical|27360/2/4|fast|47.000 [46.000,47.000]|47.000 [46.000,51.000]|34.000 [30.000,34.000]|1.000|1.382|3/5|
|8: dense / slopes|32160/6/8|fast|109.000 [108.000,110.000]|108.000 [108.000,109.000]|75.000 [71.000,76.000]|0.991|1.440|0/5|

There are 15/45 paired losses and 5/45 ties. Six of nine medians improve,
one ties and two lose. The large three-FE gains are 8–9 ms (2.5–3%), while the
multiple-slope case loses all five pairs. This does not justify the extra owner,
recurrence and kernel specialization for production. No further batch-width,
threshold or frozen-seed tuning follows. All runtime edits and the unused test
modules have been removed after verifying their preserved artifact sources.

For the **accepted** runtime, the final fresh narrow categorical case remains
47/34=1.382 times stock, requiring 16.4 ms (35%) savings to reach 0.90. Large
low slopes remain 322/288=1.118, requiring 62.8 ms (19.5%); pairing recovered
only 8 ms. Large fast slopes are 302/286=1.056. Several other designs already
beat stock by much more than 10%, but those observations do not erase losses.

## Scientific, memory and regression scope

All 414 headline public states and all 12 separately traced states pass the
sample, objective, finite/FE-score and model-space public-state checks. All 276
headline baseline comparisons, 138 direct candidate/stock comparisons and six
diagnostic pairs pass. The final candidate passes **99 distinct Rust tests**
across the two focused filters; repeated runs are not added to that count.
Four licensed science executions cover an explicit Poisson/Hessian/sandwich
oracle with unit/FW/PW weights and robust/cluster inference in fast/low modes,
two-partition slope science, categorical canonical products and three-FE OLS.
Two real allocation-audit executions at T1/T4 cover 16 warmed ordinary iterations,
zero allocator events/growth, stable capacities and cold/warm agreement.
They do not separately certify zero allocation for every canonical pair call.

Pair payload is coefficient-sized, not N-by-K. In the early large fast case,
both modeled and tracked peak rise by 349,992 bytes. The scalar workspace and
all observation buffers stay present; transient native-prototype allocations
are distinct from the retained public banks. Physical RSS is sampled but is
not a reliable attribution of this small payload. No constrained-memory or
cross-platform qualification is claimed.

The generic owner also serves four-plus categorical low-mode checkpoints. A
focused explicit-oracle control at seed 202609411 was declared but **not run**
after rejecting the candidate; it is not claimed as coverage. Production is
restored, so no such routing change is adopted. Existing allocation inventory
drift, constrained weighted-fit/nonretaining-CG issues, comparator limitations,
SCC/Linux/thread and physical-memory gates remain open. Frozen 77/16/111
benchmark definitions and all earlier receipts are unchanged.

## Exact artifacts and preservation

The three production candidates and both diagnostic builds start at `21280286`
with source-bound overlays. The archive includes the accepted baseline source,
all candidate sources, build/ABI receipts, declarations, controllers, raw numeric
summaries, diagnostics, rejected regression block, source hashes and independent
checks. No plugin/test binary, raw licensed log or large binary dataset is
committed; hashes preserve those local artifacts. Licensed protocols and statuses
are extracted separately. The final candidate's 556 source inputs were verified
before restoration; the accepted baseline's 554 inputs were verified afterwards.

| Artifact | Plugin SHA-256 |
|---|---|
|familyrows2|`4a326b8d3b210084620478dfd857ffdca672db5aacfc7acbd82fbc3194b98af8`|
|batchrhs2|`24255c03a5eae2846861faa43c0885ab8e823b4e70fe8c983c3955b1deb0155f`|
|batchrhs3|`62afd00e204483833aebd191ac4940293717c133365eeb6e0cfc4321f6e0b1ee`|
|batchrhs4|`27f1bc3c3a4fbeaeccedbc13da293679eaa758e283882cd584314bd922022a84`|
|pairtracebase|`418f3c3c0d2dc22f999d3bbf013aa092e5734dc75b4ebf7b2bab58076cfa983b`|
|pairtrace2|`2434f7a3a98e7b669df02f0d6f568f2bf08e490e6fbe00cb2fb528caf6932e43`|

The verified archive contains 15,222 logical receipts,
2,953 distinct payloads and 26,657,853 bytes. SHA-256:
`0cc7f6236836882b8c94a3f4fea07c815c647f0e12fa0bcc7da0a200b3f6c7bf`. Use `restore_receipts.py` to verify and restore the sanitized
logical receipts. Native and public blocks have no detected process overlaps;
this is sampled local development evidence, not absence of all ambient load or
release qualification.

Next: profile narrow categorical Stata preparation/publication and native setup
on the same public command, then remove demonstrated repeated work. Do not
reopen these pair layouts or the preceding retained-coefficient experiment.
STATUS.md remains the sole mutable handover record. No push, release, SCC retry,
stopped campaign or qualification occurred.
