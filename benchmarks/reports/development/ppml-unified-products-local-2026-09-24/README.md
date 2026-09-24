# Unified streamed finite products for joint PPML

Local macOS arm64 development milestone, September 24, 2026. Adopt
`unifiedproduct1` under [ADR 0151](../../../../docs/adr/0151-unified-streamed-joint-products.md).
Every streamed joint PPML Hessian entry uses the established resolved-column
product helper. Remove the separate serial direct-row loop and its temporary
borrowing of factor storage. Norms, reconstruction and residual moments retain
their separately validated storage policy.

Fresh direct-storage K62 and disconnected K32 fits improve **51.0% and 35.9%**
against the previous checkpoint, running **41.2% and 32.3% faster than comparable
stock fixest**. Unbalanced K12 and three-FE multiple slopes also improve, though
K12 still loses to stock. Four of six fresh cases clear the 0.90-times-stock
target; one is an already competitive excluded wide path. Narrow timings show
unexplained dispersion and do not establish a gain after a fixed-input repeat.
The broad objective remains unfinished.

## Source and comparable work

Baseline `serialproduct1` matches all **542 computational inputs** at `48f9c7ef`.
Its plugin SHA-256 is
`e3794b449b9b778f32362de017fc865c2075e51f68b823c5c4dff7475f70aac7`.
Candidate `unifiedproduct1` overlays only
`crates/fereg-core/src/fe/slope_coefficients/joint.rs` on that commit. All 542
captured inputs match the adopted source. Candidate plugin:

`8bc61627701864b4bd88c2ed7307c8014ea2cffbc47cd28a389f35e2d8db3bda`.

Pinned unmodified fixest 0.14.2 archive SHA-256:
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Use `glm.tol=1e-10`, `fixef.tol=1e-11`, matched requested threads and serial
BLAS. Whole public clocks include robust inference and fitted means, including
stock prediction. Startup, generation/import, warmup and export are outside
both clocks. Native low execution is not physical-memory-matched to stock.
Weighted/clustered inference has separate independent licensed checks; ordinary
fixest weights are not a frequency-weight inference oracle.

Baseline/candidate/stock order rotates in fresh processes. No compilation,
profiling, archiving or competing fit accompanies headline clocks. Development
seed 202609347 is reused. Fresh seed **202609357** was declared after code freeze and
is consumed; no source tuning used it. Same-input repeats use that seed without
changing any source or case. Frozen 77/16/111 definitions and protected cycle
seeds 211/212 remain unchanged.

The initial development screen failed during generation because its K1 request
lacked x2, which the existing outcome generator requires. Zero screen fits had
started; the nine independent pilot applications had already passed. Preserve
the original declaration, controller and failure log. Only the narrow screen
case was changed to K2 before retry; no source, generator, seed, tolerance or
other case changed. This is a failed controller, not an application failure.

## Computation and scientific boundary

Direct row binding removed recipe lookup but still gathered the full stored
model row for every Hessian row, including columns no longer needed by that
upper-triangle suffix. The candidate uses the existing per-source product
visitor and eight fixed accumulator chains. Contiguous numeric slices are
borrowed; generated sources retain fallible visitation. This removes the complete-
row gather and dependent reduction without a new solver or width/thread guard.
The removed factor-bank zip is no longer needed during product preparation.

Each entry is still the current weighted, normalized projected/original inner
product specified by ADR 0148. Newly affected serial direct-storage reductions
change order; independent mathematical oracles and unchanged numerical envelopes
supply acceptance. The product helper's reduction tree is independent of team
width, without promising cross-thread bit equality for the full estimator.
Dense retained products, multiworker product arithmetic and rare curvature
fallback retain their prior algorithms. Public OLS does not use this changed path.

Current-weight geometry, stable norms, FE/control products, factorization,
residual reconstruction, independent FE/finite scores, outer objective, strict
agreement, canonical rank/bread/final Hessian and publication retain their checks
and schedule. No sample, tolerance, weight, DoF, rank, separation or atomicity
change. Ordinary Newton products still cannot authorize rank or inference.

## Whole public timing distributions

Development seed 347, three rotations, medians [min,max] in ms. Low mode, T1,
robust inference and fitted means:

| Case | Baseline | Candidate | Stock | Native paired changes ms |
|---|---:|---:|---:|---|
| N16320/K2 | 56 [56,57] | 55 [55,58] | 37 [34,37] | -1/-2/+2 |
| N30720/K6 | 136 [136,137] | 130 [130,132] | 128 [128,130] | -5/-6/-6 |
| N16320/K24 | 243 [241,265] | 171 [170,179] | 229 [228,230] | -86/-73/-70 |
| N16320/K63 | 1060 [1019,1157] | 480 [477,628] | 799 [786,801] | -542/-580/-529 |
| N16320/K64, excluded product path | 528 [520,535] | 530 [524,535] | 820 [790,838] | 0/-4/+10 |
| N30720/K6, slopes on both | 178 [176,180] | 171 [170,171] | 325 [324,341] | -7/-10/-5 |

K24 and K63 medians improve 29.6% and 54.7%, with every pair improving despite
the K63 candidate's 628-ms outlier. K6 improves 4.4%; narrow K2 has mixed pairs.
K64's unchanged product path loses 2 ms at the median, including a +10-ms pair.
One-rotation small independent pilots are 16→15, 15→15 and 14→13 ms against
stock 15, 14 and 23 ms; they are not timing confirmation.

Fresh seed 357, five rotations, low mode and T1:

| Case | Baseline | Candidate | Stock | Candidate/stock | 0.90 target |
|---|---:|---:|---:|---:|---:|
| Two FE N17280/K62 | 1010 [975,1015] | 495 [494,497] | 842 [840,855] | 0.588 | 757.8 |
| Disconnected two FE N20160/K32 | 451 [443,520] | 289 [281,302] | 427 [420,428] | 0.677 | 384.3 |
| Unbalanced two FE N15120/K12 | 114 [114,116] | 96 [94,105] | 79 [79,80] | 1.215 | 71.1 |
| Narrow two FE N12000/K3 | 69 [50,98] | 56 [48,201] | 47 [34,88] | 1.191 | 42.3 |
| Unbalanced three FE, multiple slopes, N19200/K10 | 201 [200,205] | 185 [183,189] | 449 [447,457] | 0.412 | 404.1 |
| Two FE N18240/K65, excluded product path | 580 [575,598] | 579 [573,592] | 944 [935,970] | 0.613 | 849.6 |

Paired native changes by row:

* -513/-519/-478/-520/-491 ms
* -218/-145/-161/-162/-174 ms
* -19/-20/-9/-20/-19 ms
* -14/+121/-42/+39/-2 ms
* -16/-14/-16/-18/-20 ms
* -19/+12/-2/-5/+6 ms


The multiple-slope design uses g1[z1,z2] and g2[z2]. All twenty native pairs for
K62, disconnected K32, unbalanced K12 and multiple slopes improve. K12 improves
15.8% but needs another 25.9% reduction to meet its 71.1-ms stock target. Multiple
slopes improve 8.0% and retain a substantial stock win. The excluded K65 median
is nearly unchanged, with mixed pairs.

The narrow K3 distribution is unusually dispersed in all three implementations.
Its apparent 69→56-ms median improvement is not accepted as reliable evidence.
No contention was detected, but the cause of the dispersion is not established.
The same-input repeat below is additional evidence, not a replacement receipt.

Controls, three rotations:

| Case | Baseline | Candidate | Stock | Native paired changes ms |
|---|---:|---:|---:|---|
| Fast categorical two FE N16320/K16/T4 | 136 [135,136] | 135 [133,140] | 61 [61,62] | +5/-1/-3 |
| Low one FE N16320/K12/T4 | 42 [41,43] | 42 [41,42] | 26 [25,26] | 0/-2/+1 |
| Low weak barbell N7680/K4/T1 | 34 [34,35] | 35 [34,35] | 377 [377,380] | 0/0/+1 |
| Fast slopes on both N30720/K6/T4 | 125 [124,126] | 125 [125,126] | 118 [117,122] | +2/-1/0 |
| Low two FE N18240/K64/T8 | 325 [324,364] | 326 [320,328] | 222 [221,230] | -5/+2/-36 |

| Case | Baseline | Candidate | Stock | Native paired changes ms |
|---|---:|---:|---:|---|
| OLS categorical two FE N16320/K16/T4 | 36 [35,38] | 36 [35,36] | — | +1/-3/0 |
| OLS slopes on both N30720/K6/T4 | 71 [70,72] | 73 [71,83] | — | +1/+13/0 |

Fast categorical, one-FE, fast slopes and threaded wide remain slower than stock.
Weak connectivity remains a large win but loses 1 ms at the native median.
Threaded wide loses 1 ms, with mixed pairs. OLS exports are exact; categorical
medians tie, and slopes lose 2 ms with an 83-ms outlier. Preserve all these losses.

## Fixed-input follow-up of dispersed controls

After the complete headline blocks, freeze two five-rotation repeats using the
same source, seed 357 and literal case definitions. No code tuning or additional
candidate is introduced. These are independent fresh-process runs, not selected
subsets of the earlier distributions.

| Case | Baseline | Candidate | Stock | Native paired changes ms |
|---|---:|---:|---:|---|
| Narrow PPML N12000/K3/T1 | 48 [48,50] | 48 [47,49] | 35 [35,39] | 0/0/+1/0/-3 |

| Case | Baseline | Candidate | Stock | Native paired changes ms |
|---|---:|---:|---:|---|
| OLS slopes N30720/K6/T4 | 71 [69,72] | 70 [68,71] | — | -2/-2/0/0/0 |

Narrow PPML medians tie at 48 ms, with four non-losing pairs and one +1-ms pair;
stock is 35 ms. It needs about 34.4% further reduction to reach 31.5 ms. OLS
slopes are 71→70 ms, with no losing pair. The earlier large outliers do not
reproduce, but their cause remains unattributed. These repeats support retaining
the broad gain without a reproducible OLS regression; they do not establish
universal timing non-regression or a narrow PPML win.

## Scientific and memory validation

The final artifact passes **58 distinct Rust tests**, production actual-plugin
ABI/panic containment and **seven licensed fixtures**, each with explicit Stata
return codes and terminal sentinels. Independent weighted-contrast tests exercise
dense and both streamed organizations, disconnected/rank-deficient slopes,
changed responses/weights, T1/T4, zero response and failure revocation. Extreme
finite-product scales, strict retry/rollback and all 51 PPML memory tests pass.

Separate N4800/K8 and K65 explicit Poisson/Hessian/sandwich fixtures each cover
unit/FW/PW × T1/T4 × robust/cluster. Further checks cover factors/products with
PW and FE shifts 0/1000, multipartition slopes, separation, checkpoint failure,
and a **K24/T1 allocation audit** that directly exercises the changed path. It
records zero events/growth across sixteen ordinary joint steps, stable capacities
and fresh/warm equivalence. The separate 1e6-shift failure remains open.

All **210 main applications plus 25 fixed-input repeat applications** pass:
**213 public-state checks**, **nine small independent Python oracle results**,
**82 native pairs**, **71 baseline/stock** and **71 direct candidate/stock**
scientific comparisons. There are no application failures, timeouts, scientific
exclusions, detected overlaps or monitor errors. Thirty-four native export pairs
are exact; the remaining 48 pass unchanged envelopes. Maximum PPML differences
are 1.11e-16 for coefficients, 8.48e-21 for covariance and 2.14e-14 for means;
objectives are exact. Maximum candidate/stock tolerance usage is 0.191 for
covariance and 0.085 for means. Eight diagnostic applications are separate.

All 71 PPML pairs retain iteration, halvings, fallback/strict rejections, rank/
DoF, modeled peak and scratch metadata. Tracked peaks match in 68. Three excluded
fast categorical comparisons change by +1,100,983, +510,968 and -351,322 bytes.
These allocator differences remain unattributed; no universal peak-memory
non-regression claim. The implementation adds no heap bank, normalized design,
cache, worker team or admission formula. Existing product stack state is eight
f64 values. Physical-cap and cross-platform confirmation remain deferred.

## Isolated phase attribution

Baseline diagnostic `serialproductledger1`, plugin
`d7c8dac1facd21fe4c85c7b719bd368caca86ab012ec6bacbcb887b240e76ac2`,
and candidate diagnostic `unifiedproductledger1`, plugin
`336ed5eb5dd3e1dcbc7c5f3373346fb3083c84952520a590e72644803755fd6a`,
contain their corresponding implementation plus captured spans. All eight
applications, eight public states and four native pairs pass. No diagnostic
artifact supplies a stock headline time. Diagnostic work follows all headline
and follow-up measurements.

Development seed 347, T1. K24/K63 have N16320; K6 has N30720. Inclusive ms except
where labeled exclusive; six joint calls and two canonical rank calls each:

| Phase | K24 baseline→candidate | K63 baseline→candidate | K6 baseline→candidate |
|---|---:|---:|---:|
| Norms | 5.379→5.424 | 15.255→15.126 | 2.152→2.124 |
| Product tiles | 100.272→27.269 | 693.088→138.319 | 12.641→7.713 |
| Reconstruction | 11.425→11.393 | 32.120→32.501 | 5.226→5.429 |
| Exclusive recurrence | 0.983→0.901 | 2.234→2.155 | 1.318→1.393 |
| Whole joint directions | 147.300→74.145 | 805.867→250.764 | 46.317→42.295 |
| Canonical rank | 12.963→12.848 | 37.111→39.167 | 12.806→13.137 |
| Retained reprojection, one call | 15.247→15.137 | 41.422→41.655 | 8.736→8.843 |
| Outer score, seven calls | 4.542→4.623 | 10.355→10.385 | 3.613→3.491 |
| Inference | 4.411→4.648 | 22.718→23.512 | 0.867→1.029 |
| Core publication wrapper | 3.603→3.791 | 8.646→8.926 | 4.833→4.762 |
| Native root | 231.263→158.313 | 1006.961→457.748 | 120.138→116.562 |

The 73-ms and 555-ms product reductions account for nearly all of the wide
native-root improvements. Narrow K6 saves only about 5 ms in products; its
remaining 7.7-ms product cost cannot close the whole-command deficit by itself.
Its joint caller has 22.670 ms exclusive of the instrumented solve, including
fresh original-input work that needs finer attribution before any checking
redesign. The current source has no expensive old serial kernel left to tune.

The fast categorical control N16320/K16/T4 is unchanged by this implementation,
but its source-current ledger identifies the next substantial cost:

| Phase | Calls | Baseline ms | Candidate ms |
|---|---:|---:|---:|
| Initial retained design | 1 | 18.900 | 18.708 |
| Canonical response/design reprojection | 2 | 45.291 | 47.662 |
| Canonical rank | 3 | 12.009 | 12.722 |
| Ordinary categorical LSMR solve | 5 | 21.430 | 20.327 |
| LSMR preconditioner | 5 | 2.049 | 1.553 |
| LSMR recurrence | 5 | 13.645 | 13.198 |
| LSMR final checks | 5 | 3.706 | 3.579 |
| Outer score | 8 | 2.187 | 2.212 |
| Inference | 1 | 1.622 | 1.618 |
| Native root | 1 | 125.646 | 127.885 |

These rows nest and must not be summed. Single intrusive clocks establish phase
location, not a headline timing regression. The two reprojections occupy 37% of
the candidate native root; all ordinary LSMR solves together occupy 16%. The
reprojection span includes 36.927 ms exclusive of the existing probes, so
attribute its internals before equating that entire amount with removable work.

## Next implementation milestone

**Generalize accepted-frame finalization to fast categorical PPML.** The current
strict/final path performs two canonical response/design projections and three
rank constructions including initialization. The already adopted joint route
uses a strict current-weight Newton witness plus one independent canonical final
rank/bread/Hessian construction while preserving the accepted state. Investigate
using the existing fast categorical LSMR owner as that strict witness, rather
than replacing its ordinary engine merely because it differs from the joint
route. Verify its fresh projection/finite-score/reconstruction authority and
bounded refusal policy before sharing the finalization mechanism.

Instrument the unassigned canonical projection work and compare identical frozen
frames. Reuse only valid current-sample, basis, raw-weight and generation state;
keep separate canonical rank and Hessian authority. Test forced witness refusal,
state movement, basis changes, stale weights/generations, optional-memory refusal
and rollback, with independent Poisson/Hessian/weighted oracles. Protect the
current low-mode and slope wins, explicit solver choices, generated terms,
small fits and OLS. Do not remove guards from the rejected all-coefficient
canonical route or relax strict tolerances.

Fast categorical's present 135-ms public median needs about a **59% reduction**
to reach 54.9 ms. Removing one reprojection alone cannot be promised to close
that gap. Treat coherent finalization ownership and its remaining projection
work as one bounded milestone; revisit ordinary solver choice only if its
measured residual cost warrants it. Then address narrow original-input passes,
one-FE/public-output costs and current threaded-wide losses. Current targets
are 31.5 ms for the repeated narrow case, 23.4 for one FE, 106.2 for fast slopes
and 199.8 for threaded K64. No parity promise or universal non-regression claim.

Historical large one-FE stock-mean discrepancies, the separate 1e6-shift failure,
weighted constrained-fit failures, insufficient-budget nonretaining-CG span
instability and static allocation source drift remain open. SCC/Linux T1/8/16
and physical-cap confirmation stay deferred. No stopped campaign, SCC auth
retry, push, release or qualification was performed.

## Immutable receipts

[Receipt archive](receipts-content-addressed.zip),
[container checksum](receipts-content-addressed.sha256) and
[verifying restorer](restore_receipts.py) preserve **9,117 logical files**, deduplicated
into 2,239 payloads (18,058,616 bytes). Container SHA-256:

`ce38b636ea97b0239bf94585d326080e920c265d2d03093718c95f73d3e5c230`.

The archive includes exact computational sources, artifact identities/build logs,
controllers/declarations, failed-generation receipts, numerical exports and
scientific audits, distributions and metadata, licensed-fixture outcomes and
isolated phase spans. Raw licensed logs, datasets and binaries are excluded
with hashes. Machine paths are sanitized. The restorer checks the container
and every logical payload against the sealed manifest. Claims attach to the
captured source/artifact identity, not to later repository revisions.
