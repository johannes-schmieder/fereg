# Source-oriented serial joint PPML products

Local macOS arm64 development milestone, September 24, 2026. Adopt
`serialproduct1` under [ADR 0150](../../../../docs/adr/0150-source-oriented-serial-joint-products.md).
The joint PPML serial finite-Hessian kernel now follows the source layout:
recipe-backed rows use the existing resolved-column product helper, while
complete direct numeric row bindings retain their fused upper-triangle kernel.

Four of six fresh cases clear the whole-public-command **0.90-times-stock**
target. Three wide serial categorical cases improve 71–74% against the previous
checkpoint and run at 0.579–0.644 times comparable stock. The unbalanced three-FE
multiple-slope case improves 61.7%, running at 0.311 times stock. Directly bound
K63, small fits, threaded wide, fast categorical and one-FE losses remain.
This closes a measured access omission; it does not complete the broad objective.

## Source and matched work

Baseline `columnflow2` matches all **542 computational inputs** at `07a4d616`.
Its plugin SHA-256 is
`f2113d96c61794a6d8b2c77cccbf687697acd73c75bf8d741343f49ef77419cb`.
Candidate `serialproduct1` overlays only
`crates/fereg-core/src/fe/slope_coefficients/joint.rs` on that commit. All 542
captured inputs match the adopted source. Candidate plugin:

`e3794b449b9b778f32362de017fc865c2075e51f68b823c5c4dff7475f70aac7`.

Pinned unmodified fixest 0.14.2 archive SHA-256:
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Use `glm.tol=1e-10`, `fixef.tol=1e-11`, matched requested threads and serial
BLAS. Public clocks include robust inference and fitted means, including stock
prediction. Startup, data generation/import, warmup and exports are outside both
clocks. Native low mode is not physical-memory-matched to stock. Separate
licensed independent oracles cover weighted/clustered inference; ordinary fixest
weights are not a frequency-weight inference oracle.

Baseline/candidate/stock order rotates in fresh processes. No compilation,
profiling, archiving or competing fit accompanies headline clocks. Development
seed 202609347 is reused. Fresh seed **202609356** was declared after code freeze
and is consumed; no source tuning used it. Frozen 77/16/111 definitions and
protected cycle seeds 211/212 remain unchanged.

The first fresh controller stopped during generation: N3072 violated the
strong_d6 generator's requirement that N be divisible by 240. Zero fits started.
The original declaration, partial generation receipts and failure log are
preserved. Only that row count was corrected to N3120, still below the existing
4096-row direct-binding admission. The code, seed, tolerances and other cases
were unchanged before the successful run. Do not relabel this failed controller
as a fit failure or silently omit it.

## Chosen computation

The previous source already resolved each numeric source and ordinary factor
column independently, but `width == 1` still forced full-row recipe evaluation
inside every Hessian-row scan. The new condition also consults the existing
`prefers_column_scans()` storage property. It introduces no new row/width/thread
threshold, solver, cache, normalized design or public option.

For recipe-backed rows, a serial product now uses the existing eight independent
accumulators and fixed final reduction. Numeric slices are borrowed directly;
other columns retain fallible source visitation. The current weighted normalized
inner product is unchanged mathematically. Eligible serial floating-point
reductions change order, so bitwise identity with the baseline is not required.
The independent weighted-contrast and Poisson/Hessian/sandwich oracles establish
accuracy separately from reference agreement.

Current-weight geometry, norms, FE/control products, finite factors, residual
reconstruction, independent FE/finite scores, objective, strict agreement,
canonical rank/bread/final Hessian and publication retain their checks and
scheduling. No tolerance, sample, weight, DoF, separation or atomicity change.
Directly bound serial rows, all multiworker products, retained dense products,
rare curvature fallback and one-shot OLS retain their algorithms.

## Whole public timing distributions

Development seed347, three rotations, medians [min,max] in milliseconds.
All cases use low mode and T1; slopes are on both partitions where indicated.

| Case | Baseline | Candidate | Stock | Native paired changes ms |
|---|---:|---:|---:|---|
| N12000/K65 | 1338 [1335,1340] | 400 [394,401] | 618 [615,629] | -941/-938/-939 |
| N16320/K64 | 1829 [1827,1845] | 510 [506,514] | 810 [804,820] | -1339/-1319/-1313 |
| N15360/K96 | 3672 [3626,3687] | 858 [847,863] | 1484 [1445,1503] | -2825/-2829/-2763 |
| N16320/K24, direct rows | 243 [241,248] | 246 [240,247] | 229 [226,229] | +4/-2/-1 |
| N30720/K6, slopes on both | 174 [174,176] | 174 [173,176] | 325 [324,328] | 0/0/-1 |

The three wide medians improve 70.1%, 72.1% and 76.6%, with all nine native
pairs improving. The directly bound K24 median rises 3 ms, with paired changes
+4/-2/-1; the slope control ties. Small one-rotation independent pilots are
17→15, 16→15 and 13→14 ms against stock 14, 14 and 22 ms. Retain their loss and
do not treat one repetition as timing confirmation.

Fresh seed356, five rotations, low mode and T1, robust inference plus means:

| Case | Baseline | Candidate | Stock | Candidate/stock | 0.90 target |
|---|---:|---:|---:|---:|---:|
| Two FE N17280/K64 | 1859 [1847,1863] | 540 [538,544] | 882 [869,886] | 0.612 | 793.8 |
| Disconnected two FE N20160/K66 | 2312 [2291,2368] | 661 [657,669] | 1142 [1132,1161] | 0.579 | 1027.8 |
| Unbalanced two FE N15120/K80 | 2161 [2152,2173] | 571 [569,581] | 887 [870,902] | 0.644 | 798.3 |
| Two FE N16320/K63, direct rows | 1017 [1016,1035] | 1015 [1011,1051] | 802 [793,808] | 1.266 | 721.8 |
| Small two FE N3120/K10 | 34 [33,35] | 29 [27,30] | 26 [25,30] | 1.115 | 23.4 |
| Unbalanced three FE, multiple slopes, N19200/K64 | 2382 [2374,2386] | 913 [900,916] | 2932 [2928,2945] | 0.311 | 2638.8 |

Paired native changes by row:

* -1305/-1315/-1323/-1323/-1318 ms
* -1705/-1655/-1628/-1659/-1630 ms
* -1602/-1596/-1583/-1590/-1579 ms
* +16/-1/+22/-12/-3 ms
* -6/-5/-6/-6/-4 ms
* -1470/-1461/-1466/-1482/-1475 ms


The multiple-slope design uses g1[z1,z2] and g2[z2]. All twenty pairs in the
four winning wide cases improve. The small fit improves 14.7% against baseline
but still misses stock, needing another 19.3% reduction to reach 23.4 ms. The
K63 direct-row median is nearly unchanged and requires another 28.9% reduction
to reach 721.8 ms. These target budgets are not promised attainable times.

Excluded and threaded controls, three rotations:

| Case | Baseline | Candidate | Stock | Native paired changes ms |
|---|---:|---:|---:|---|
| Fast categorical two FE N16320/K16/T4 | 126 [125,129] | 124 [124,128] | 61 [61,62] | +2/-5/-1 |
| Low one FE N16320/K12/T4 | 41 [41,42] | 42 [40,42] | 26 [25,27] | +1/0/-1 |
| Low weak barbell N7680/K4/T4 | 32 [31,32] | 32 [31,32] | 209 [208,213] | 0/0/0 |
| Fast slopes on both N30720/K6/T4 | 126 [126,126] | 125 [124,125] | 116 [116,117] | -1/-1/-2 |
| Low two FE N18240/K64/T8 | 324 [318,341] | 324 [323,337] | 226 [222,227] | +6/-1/-4 |

| Case | Baseline | Candidate | Stock | Native paired changes ms |
|---|---:|---:|---:|---|
| OLS categorical two FE N16320/K16/T4 | 36 [36,37] | 36 [36,36] | — | -1/0/0 |
| OLS slopes on both N30720/K6/T4 | 72 [71,72] | 69 [69,70] | — | -1/-3/-3 |

Fast categorical, one-FE, fast slopes and threaded wide remain slower than
stock. Weak connectivity remains a substantial win. One-FE loses 1 ms at the
median with mixed paired changes; threaded K64 includes a +6-ms pair. OLS
exports are exact and medians are tied/lower, but these small controls do not
establish universal non-regression. The small one-FE comparison does not resolve
the historical large one-FE stock-mean discrepancy.

## Scientific and memory checks

The final artifact passes **58 distinct Rust tests**, production actual-plugin
ABI/panic containment and **seven licensed fixtures** with explicit Stata return
codes and terminal sentinels. Independent weighted-contrast tests exercise dense,
streamed row and streamed column organizations, changed weights/responses,
disconnected/rank-deficient slopes, T1/T4, zero response and failure revocation.
Strict retry/rollback and all 51 PPML memory tests pass.

Separate N4800 K8 and K65 explicit Poisson/Hessian/sandwich fixtures each cover
unit/FW/PW × T1/T4 × robust/cluster inference. Further fixtures cover ordinary
factors and a numeric product with PW and FE shifts 0/1000, K65 allocation
behavior, multipartition slopes, separation and low checkpoint transaction
failure. The separate 1e6-shift failure remains open.

All **201 headline applications**, **189 public-state checks**, **nine small
independent Python oracle results**, **69 native pairs**, **63 baseline/stock**
and **63 direct candidate/stock** scientific comparisons pass. There are no
application failures, timeouts, scientific exclusions, detected overlaps or
monitor errors. Thirty-two native exports are exact; the remaining 37 are
within the unchanged envelopes. Maximum PPML native differences are 1.11e-16
for coefficients, 1.91e-20 for covariance and 2.14e-14 for fitted means. The
largest candidate/stock tolerance usage is 0.191 for covariance and 0.076 for
means. Exact exports are additional evidence, not the acceptance definition.

All 63 PPML pairs retain outer/inner iterations, halvings, fallback/strict
rejections, rank/DoF, modeled peaks and scratch metadata. Tracked peaks match
in 61 pairs; two excluded fast categorical pairs fall by 641,111 and 551,063
bytes. No universal physical-memory improvement is claimed. The actual-plugin
K65/T1 audit records zero allocator events/growth over sixteen ordinary joint
steps, stable capacities and fresh/warm equivalence. It directly exercises the
changed kernel. No heap bank or admission formula is added; the existing product
helper uses eight f64 stack accumulators. Physical-cap and platform checks
remain deferred.

## Isolated phase attribution and revised next step

Baseline diagnostic `columnflowledger2`, plugin
`ea8e63b6a7d10dd5519c44b573b4e613b13575057e537ebf71f5a04db3463031`,
and candidate diagnostic `serialproductledger1`, plugin
`d7c8dac1facd21fe4c85c7b719bd368caca86ab012ec6bacbcb887b240e76ac2`,
contain the corresponding production implementation plus captured phase spans.
All eight diagnostic applications, eight public states and four native pairs
pass. These run after all headline work; stock timings use no instrumentation.

K65 has N12000, K96 N15360, and the direct-row K24/K63 controls N16320. All use
T1 and development seed347. K63 was added to the diagnostic declaration after
fresh timing exposed its remaining loss; no source changed. Its new receipt
records that reason explicitly, and fresh seed356 remains untuned.

| Phase, ms | K65 baseline→candidate | K96 baseline→candidate | Direct K24 baseline→candidate | Direct K63 baseline→candidate |
|---|---:|---:|---:|---:|
| Norms, six calls | 17.256→17.389 | 32.027→32.523 | 5.392→5.503 | 15.495→14.298 |
| Product tiles, six calls | 1084.274→110.312 | 3156.245→279.439 | 104.201→101.105 | 709.527→683.350 |
| Reconstruction, six calls | 19.512→19.156 | 35.847→35.562 | 11.601→11.963 | 32.878→31.856 |
| Exclusive recurrence | 2.241→2.233 | 4.434→4.337 | 0.951→0.992 | 2.368→2.181 |
| Whole joint directions | 1172.523→198.604 | 3316.692→437.486 | 151.816→149.094 | 825.435→794.545 |
| Canonical rank, two calls | 36.980→37.298 | 85.537→87.326 | 12.749→12.719 | 38.882→38.774 |
| Retained reprojection, one call | 34.456→33.197 | 66.262→66.475 | 15.377→15.081 | 42.478→42.123 |
| Outer score, seven calls | 21.269→21.860 | 40.376→39.453 | 4.525→4.506 | 10.512→10.265 |
| Inference | 20.080→20.117 | 53.290→53.391 | 4.643→4.737 | 22.909→23.041 |
| Core publication wrapper | 16.351→15.237 | 21.695→27.071 | 3.355→3.575 | 9.526→8.802 |
| Native command root | 1366.550→390.826 | 3706.291→832.426 | 236.450→234.011 | 1037.262→999.753 |

Rows nest and must not be summed. These are single intrusive clocks, not
headline ratios. The 974-ms and 2877-ms product reductions account for nearly
all of the corresponding native-root reductions. Stable call counts and
unchanged iteration metadata support an access/reduction cost explanation,
rather than fewer or weaker solves. K96's publication wrapper rises 5.4 ms in
this single diagnostic; its cause is not attributed.

**Next: efficient serial finite products across direct storage too.** The K63
control still spends 683.350 ms, 68% of its diagnostic root, in product tiles.
Its 2.181-ms exclusive recurrence is not the priority. Direct binding removed
recipe dispatch but did not establish that the retained row reduction is the
best product organization. A coherent next candidate should compare bounded
column/block Gram formation with that fused row kernel on identical current
frames. Keep norms/reconstruction decisions separate: the earlier rejection of
unconditional column scans there did not test this product replacement. Preserve
the independent acceptance, finite-scale tests and low-memory allocation bound;
protect narrow serial, generated, multiworker, slope and OLS paths. Do not tune
the accumulator count or add benchmark-specific K63/K64 guards. The fresh K63
budget requires about a 29% whole-command reduction, so this is a substantial
opportunity rather than a residual one-millisecond chase.

After that, resume accepted-frame canonical/product ownership, fast categorical
organization and one-FE/public-output work. Current control budgets are 54.9 ms
for fast categorical (current124), 23.4 for one FE (42), 104.4 for fast slopes
(125), and 203.4 for threaded K64 (324). The existing previous-milestone threaded
phase evidence suggests canonical construction, scores and inference matter,
but obtain current-source attribution before using those older spans as a new
budget. Ordinary joint products never supply rank or covariance authority.

The historical large one-FE stock-mean discrepancy, separate 1e6-shift failure,
weighted constrained-fit failures, insufficient-budget nonretaining-CG span
instability and static allocation source drift remain unresolved. SCC/Linux
T1/8/16 and physical-cap confirmation remain deferred. No stopped campaign,
SCC authentication retry, push, release or qualification was performed.

## Immutable receipts

[Receipt archive](receipts-content-addressed.zip),
[container checksum](receipts-content-addressed.sha256) and
[verifying restorer](restore_receipts.py) preserve **8,102 logical files**, deduplicated
into 2,064 payloads (17,310,218 bytes). Container SHA-256:

`840f12ae2cc4cfe9b408d4461d5205763f201e6657e2e8631ac0689d8a081878`.

The archive includes exact computational sources, artifact identities/build logs,
controllers and declarations, failed-generation receipts, all numerical exports,
public-state/oracle audits, timing/metadata summaries, licensed-fixture results
and isolated diagnostic spans. Raw licensed logs, datasets and binaries are
excluded with hashes. Machine paths are sanitized. The index maps logical files
to content-addressed payloads; every restored payload is checked against the
sealed manifest. Source snapshots and measured identities, not a later commit's
label, bound the timing claims.
