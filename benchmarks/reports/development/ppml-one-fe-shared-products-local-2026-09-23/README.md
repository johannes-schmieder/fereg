# Shared one-FE PPML products: local development milestone

Accept `onefeshare2`, based on `19ab5cab`, with all **536 computational inputs**
matching the final source. Plugin SHA-256:
`26cef3ff3c30a727c34437c81025cf2d104c2807c391e4d1f637f51dce687316`.
Baseline is accepted `jointfinal2`, plugin
`96bea000c5ca47fa0176ae37302b793fec13470efbcbbf657b7446e7cf99506b`.
This improves one-FE PPML and reduces memory; it does **not** achieve the broader
whole-command target of at most 0.90 times comparable pinned stock fixest.

## Change and scientific boundary

[ADR 0132](../../../../docs/adr/0132-shared-one-fe-ppml-products.md) makes two
related ownership changes. Concentrated one-FE Newton steps borrow retained
FE-equivalent finite columns, removing their second N-by-K copy. Subtracting
group constants from those columns changes the optimized group intercepts but
preserves the model, centered Hessian and score. The borrow carries no authority
for current-weight orthogonality; that is established independently.

The strict canonical Newton WLS still rebuilds the within design, canonical
rank, finite solve, bread and model-space/reconstruction certificates. When it
confirms the unchanged accepted profiled point, those products can supply final
inference directly. Exact weight bits, every scale field, frame/design generation
and fresh predictor-decomposition agreement are required. Final consumption
again checks exact q*mu; independent outer scores, objective, inference and
publication checks remain. A declined identity/consistency check retains the
canonical predictor-response final solve. There is no tolerance or sample change.

Only the already eligible one-intercept-FE, fast, MAP-owned PPML route changes.
Slopes, categorical multi-FE, low mode, synthetic no-FE, other solver families,
mu separation and public OLS retain their computational paths. No extra heap
bank, public option or response-history solver layer is added.

## Fresh public timing distributions

Predeclared seed **202609332**, three alternating baseline/candidate/stock
rotations. Every timed application is uncontended by builds, profiling, archiving
or other fits; overlap and monitor checks pass. Times are milliseconds,
**median [minimum, maximum]**. Timers include the whole public command with
loading/preparation, robust inference, fitted means and publication. Generation,
import, process startup, warmup and exports are outside the timer.

Stock is unmodified pinned fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
using `glm.tol=1e-10`, `fixef.tol=1e-11`, matched application threads, serial BLAS
and the established comparable covariance normalization. No diagnostic build
contributes a headline time. All stock comparisons in this milestone pass.

| One-FE design | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| N76800 K10 T4 | 159 [157,160] | 139 [139,141] | 83 [80,83] | 1.675 |
| N61440 K10 T1 | 171 [169,175] | 150 [147,154] | 96 [94,97] | 1.562 |
| N38400 K24 T4 | 149 [148,151] | 134 [131,136] | 100 [98,106] | 1.340 |
| Unbalanced N76800 K6 T8 | 138 [136,139] | 122 [121,123] | 42 [41,45] | 2.905 |
| Small N3840 K3 T1 | 22 [17,22] | 22 [19,22] | 11 [9,11] | 2.000 |
| Larger N184320 K6 T8 | 311 [305,318] | 270 [269,273] | 126 [124,127] | 2.143 |

Median native gains are **12.6%, 12.3%, 10.1%, 11.6%, 0%, 13.2%**. Every paired
larger fit improves (candidate/baseline 0.858–0.919); the small cell includes a
29.4% paired loss. Its predeclared five-pair follow-up uses identical numerical
input hashes and gives **23 [17,23] / 19 [16,25] ms**. Preserve both blocks and
the individual observations. Small-command variability prevents a precise gain
claim; no follow-up stock ranking replaces the original fresh comparison.

Development `onefeshare1`, seed331, separately gives 130/116/52 ms at N61440/K10/T4,
155/142/120 at N30720/K32/T4, and 128/115/51 for unbalanced N61440/K10/T8.
Its plugin is `13389014c24396a94c5cb883aee4d808764fb7b3a7ab036ba0c1552d9a52a609`.
The only subsequent source change corrects a stale comment; `onefeshare2` has a
separate identity, build, licensed checks and fresh timings. Do not transfer
prototype clocks to the final artifact.

## Attribution and memory

Native production phase receipts, fresh medians:

| Case | Strict/final baseline -> candidate (ms) | Candidate prep | Candidate line search | Candidate publication |
| --- | ---: | ---: | ---: | ---: |
| N76800 K10 T4 | 44.234 -> 27.018 | 20.180 | 19.649 | 12.673 |
| N61440 K10 T1 | 50.768 -> 30.928 | 16.496 | 16.704 | 9.800 |
| N38400 K24 T4 | 38.578 -> 25.031 | 16.946 | 21.288 | 6.904 |
| Unbalanced N76800 K6 T8 | 42.284 -> 25.999 | 16.359 | 16.084 | 12.720 |
| Larger N184320 K6 T8 | 100.828 -> 61.319 | 40.305 | 38.303 | 29.231 |

Outer counts remain unchanged: three in the main, serial, unbalanced and larger
cells; four in the wide and small cells. Inner diagnostics fall by exactly one,
reflecting the removed canonical transaction. Most measured whole-command gain
coincides with the strict/final reduction. These aggregate phases do not separate
all remaining kernels; nested measures must not be added as independent costs.
The earlier isolated diagnostic at the baseline remains in its original report.

| Case | Baseline tracked peak bytes | Candidate tracked peak bytes | Candidate modeled bytes |
| --- | ---: | ---: | ---: |
| N76800 K10 T4 | 49,579,774 | 43,435,774 | 47,790,296 |
| N61440 K10 T1 | 38,354,590 | 33,439,390 | 36,957,080 |
| N38400 K24 T4 | 44,000,026 | 36,627,226 | 38,820,624 |
| Unbalanced N76800 K6 T8 | 36,824,036 | 33,137,636 | 37,386,770 |
| Small N3840 K3 T1 | 1,553,437 | 1,461,277 | 8,828,368 |
| Larger N184320 K6 T8 | 92,934,672 | 84,087,312 | 94,575,762 |

Tracked reductions equal the removed 8*N*K payload, ranging 0.092–8.847 MB.
No physical memory-cap or cross-platform qualification is implied. Actual-plugin
T1/T4 warmed allocation audits retain zero events and growth. Their separately
admitted cold witness materializes original raw finite columns outside the
measurement window to compare against borrowed representatives.

## Science and excluded-path controls

Final-source **350 distinct Rust tests** pass: the PPML family plus focused
retained-session tests. Full-dummy weighted normal equations cover unequal
weights, large/small common scales, rank refusal and threads. New boundaries
cover group-constant shifts, noncontiguous active columns, changing weights,
canonical final fallback, one-ULP changed weights, changed scales, stale generation
labels and corrupted predictor agreement. The first session filter selected zero
tests; its log is retained, and the corrected filter ran 16 tests. It is not counted
as validation. Both actual-plugin ABI/panic checks pass.

Five final-artifact licensed fixtures pass explicit application rc, process rc
and terminal sentinels: one-FE dummy Poisson/Hessian inference, wider weighted
one-FE passes, real allocation/saved-output checks, separation and OLS memory
modes. They cover unit/frequency/probability weights, offsets, robust/clustered
inference, T1/T4, saved effects and a zero-control group oracle. Ordinary fixest
weights are not used as a frequency-weight inference oracle.

Across prototype and final blocks, **175 applications**, **129 public PPML
states**, **49 native PPML pairs**, **31 stock pairs** and **12 small independent
Poisson/Hessian checks** pass. Of these, final-artifact blocks contribute 142
applications, 96 public states, 38 native pairs, 20 stock pairs and six independent
checks. No application failure, timeout, scientific discrepancy or benchmark
contention occurred. This does not adjudicate historical comparator failures.

Excluded PPML preserves **13 exact scientific-export pairs**, including follow-up.
Initial two-pair medians, baseline/candidate:

| Excluded case | Median ms | Range baseline / candidate |
| --- | ---: | --- |
| Low one-FE PPML | 114 / 104.5 | [100,128] / [100,109] |
| Dense three categorical FE | 203 / 206.5 | [201,205] / [204,209] |
| Degree-six categorical | 111 / 109.5 | [110,112] / [109,110] |
| Two slope partitions | 136.5 / 136.5 | [136,137] / [136,137] |

Five identical-input dense-categorical follow-up pairs give **201 [199,206] /
203 [200,205] ms**. A 2-ms median loss remains with overlapping distributions;
there is no excluded-path speed claim.

OLS preserves **23 exact scientific-export pairs** across slope, low-memory
three-FE, one-FE full outputs and generated-factor controls. Initial medians are
66.5/66.5, 64.5/65.5, 17/18 and 66/66 ms respectively. Five-pair follow-up is
64/68 for low three-FE and 18/17 for one-FE. Candidate low-mode times decline
70,69,68,63,63 ms within that block; one final fixed-input block is 66/65.
Across **all 12 low-mode pairs**, medians/ranges are **65.5 [63,69] / 65 [63,70]**.
Keep the earlier loss visible; these bounded results do not establish universal
non-regression. Every follow-up input hash matches its original cell.

## Remaining work and decision

Retain this small ownership change: the larger eligible fits reliably improve
10–13%, memory decreases, and independent acceptance remains intact. Stop tuning
this final reuse predicate; its remaining scan is necessary to establish the
specific handoff and is not the dominant cost.

The fresh N76800/K10 fit still needs **64.3 ms** to reach 0.90*83 = 74.7 ms.
The larger fit needs **156.6 ms** to reach 113.4 ms. Even eliminating its entire
61.3-ms strict/final phase could not close that gap. Preparation and publication
alone consume about 69.5 ms there, and ordinary line-search/profile work another
38.3 ms. Stronger certificates explain only part of the residual difference.

Next investigate **initial rank/fit construction and shared accepted-state
assembly across the whole PPML path**. Measure a rank-only initialization
candidate instead of constructing unused initial inference products, while
preserving the exact initial canonical rank policy and a fresh final inverse.
Separately account for repeated original-column scans and duplicate accepted
predictor/mean/moment construction before choosing the next implementation.
A current-state owner may share computation, but its independent original-input
and final-Hessian witnesses must remain. Scope this to a bounded milestone with
one-FE, categorical multi-FE, slope and low-mode controls; do not reopen isolated
serial weighted-slope tuning or pursue the small-fit timing noise.

Seed332 is now consumed; seed333 is unused and reserved for a future predeclared
confirmation. Static allocation-inventory drift, constrained-fit/nonretaining-CG
failures, historical stock discrepancies and SCC/Linux/physical-cap deferrals
remain open. Frozen 77/16/111 definitions are unchanged. No release, push,
platform qualification or stopped campaign restart occurred.

## Immutable receipts

The content-addressed archive contains **7,255 logical receipts**, **1,860 unique
payloads**, 25,005,652 bytes. SHA-256:
`486a9aa90c3f38a16f18e1a94eb1533f098d647472c9565a1f8fd60ae3ce8d9f`.
It preserves source snapshots, identities, compiler/test evidence, declarations,
controllers, full timing distributions, scientific exports/audits and follow-up
input identities. Raw licensed logs, plugins and binary datasets are excluded;
omitted numerical input files retain hashes. Machine paths are sanitized with
original and archived digests retained. `restore_receipts.py` verifies both the
container and every logical file before restoring to a new directory.
