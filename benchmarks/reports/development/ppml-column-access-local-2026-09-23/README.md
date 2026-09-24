# Source-oriented joint PPML access

Local macOS arm64 development milestone, September 23–24, 2026. Adopt `columnflow2`
under [ADR 0149](../../../../docs/adr/0149-source-oriented-joint-ppml-access.md).
Original finite columns no longer lose direct access because the complete model
exceeds a numeric-binding limit or contains generated terms. Keep fused row
scans where row bindings are already direct; use source-column scans where
rows otherwise evaluate recipes repeatedly.

Two fresh threaded cases above the old binding limit improve about **30%**.
All 66 native export pairs are exact. Wide serial improves only 2.7%, and several
controls retain small losses. None of the six fresh cases reaches the broader
target of whole public time **<=0.90 times comparable stock fixest**. The weak
connectivity control already clears that target. This is a substantial reduction
of a measured cost, not completion of the broad PPML objective.

## Source and comparable work

Baseline `jointtri1` matches `4c2532a4`, plugin SHA-256
`481755f5ee9d42fa417f810d8373095506e05f21dc80e27b8bc2d5e64f968d7c`.
Final `columnflow2` overlays:

* `crates/fereg-core/src/fe/slope_coefficients/joint.rs`
* `crates/fereg-core/src/ols/linear_session.rs`
* `crates/fereg-core/src/fe/map.rs` — independent oracle test coverage only

All **542 captured computational inputs** match the adopted source. Final plugin:

`f2113d96c61794a6d8b2c77cccbf687697acd73c75bf8d741343f49ef77419cb`.

The unmodified pinned stock fixest 0.14.2 archive has SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Use `glm.tol=1e-10`, `fixef.tol=1e-11`, matched requested threads and serial
BLAS. Public clocks include robust inference and fitted means; stock prediction
is included. Startup, data generation/import, warmup and export are outside
both clocks. Low native execution is not physical-memory-matched to stock.
Weighted and clustered science is checked by separate independent licensed
oracles; ordinary fixest weights are not a frequency-weight inference oracle.

Baseline/candidate/stock order rotates in fresh processes. No compilation,
profiling, archiving or competing fit accompanies headline clocks. Development
seed 202609347 is reused. Fresh seed **202609355** was predeclared after final
source freeze and is consumed. No source tuning uses that seed. Frozen 77/16/111
definitions and protected cycle seeds 211/212 remain unchanged.

## Actual binding boundary and chosen organization

Isolated `columnledger1`, plugin
`fa2cd055b98dfb191c2fa74356ae0989f159bcc771d6e374f67ae7fcaf98fb9f`,
logs complete-model binding inputs on the baseline source. At N16320, public
K24, K63 and K64 models contain 25, 64 and 65 columns respectively, including
one intercept. Each full fit logs 21 minimum-one and one minimum-eight binding
attempt. The captured 64-column admission rule refuses all K64 attempts. All
three diagnostic applications and public-state audits pass. Counter lines and
their summaries are archived; these intrusive timings are not headline evidence.

The candidate resolves a numeric slice or an ordinary factor's codes before a
column scan. Generated interactions retain fallible recipe evaluation. Stable
norms, residual reconstruction and inner residual moments follow columns when
complete direct row bindings are unavailable. Norm columns use disjoint scalar
destinations on the existing team. Directly bound numeric rows and retained
dense designs keep their fused row scans.

Each norm retains its stable row-order recurrence. Each observation retains
finite-coefficient FMA order during reconstruction; score reductions still
follow row order. Independent original-input score/mean checks freshly visit
their current sources and weights. No certificate, tolerance, rank, sample,
weight, DoF, separation or publication policy changes. The observed exact
exports support this arithmetic argument; independent oracles remain necessary.

The single-worker finite-product kernel still uses complete rows even when
they are recipe-backed. The rare observation-space curvature fallback also
retains its row visitor. Other consumers keep `NumericOlsColumns` admission.
Those limitations matter to the remaining serial and canonical costs below.

## First candidate: unconditional column scans

`columnflow1`, plugin
`0fc42f8f6f23612292994413c119545daae738c472859e86b763da07a5a6e82a`,
uses column scans even when direct numeric row bindings are available. It passes
58 focused Rust tests and the K65 licensed Poisson/Hessian/sandwich oracle.
All 54 public applications, 54 public-state checks, nine small independent
Poisson results, 18 native pairs and 18 direct stock comparisons pass. All 18
native exports are exact.

Development medians [min,max] in ms, three rotations:

| Low case | Baseline | First candidate | Stock | Native paired changes |
|---|---:|---:|---:|---|
| N16320/K24/T8 | 124 [123,124] | 120 [118,127] | 72 [66,72] | -4/+4/-6 |
| N30720/K6/T4 | 97 [94,97] | 96 [96,97] | 59 [54,60] | -1/+3/-1 |
| N16320/K64/T8 | 437 [432,438] | 314 [309,320] | 192 [188,193] | -118/-129/-117 |
| N16320/K24/T1 | 234 [232,240] | 242 [241,261] | 227 [224,236] | +27/+10/+1 |
| Slopes on both, N30720/K6/T4 | 127 [127,128] | 129 [128,130] | 120 [117,122] | +3/+1/+1 |

Do not adopt unconditional scans. Fused row reductions remain valuable when
source access is already direct. The second candidate selects organization by
that storage property, without new thread/width thresholds fitted to these
cells. The separate small pilot retains ties and +2-ms losses in its receipts.

## Final public timing evidence

Development seed347, three rotations, medians [min,max] in ms:

| Low case | Baseline | Final candidate | Stock | Native paired changes |
|---|---:|---:|---:|---|
| N16320/K24/T8 | 123 [121,123] | 123 [123,123] | 71 [67,72] | 0/+2/0 |
| N30720/K6/T4 | 97 [95,97] | 96 [94,98] | 59 [58,59] | -1/+1/-1 |
| N16320/K64/T8 | 431 [426,495] | 308 [306,310] | 192 [190,197] | -116/-123/-189 |
| N16320/K24/T1 | 233 [230,233] | 234 [232,239] | 227 [227,232] | +9/+1/-1 |
| Slopes on both, N30720/K6/T4 | 128 [126,129] | 128 [127,128] | 120 [114,122] | 0/+2/-2 |

The K64 median improves 28.5%; every pair improves. Preserve the baseline's
495-ms observation rather than hiding its distribution. Narrow medians change
by at most 1 ms, with individual losses. Final small pilots are 15→15, 14→15
and 14→16 ms against stock 14, 17 and 18 ms; one rotation is not confirmation.

Fresh seed355, five rotations, low mode with robust inference and fitted means:

| Case | Baseline | Candidate | Stock | Candidate/stock | 0.90× stock target |
|---|---:|---:|---:|---:|---:|
| Two FE N17280/K63/T8 | 281 [279,282] | 283 [280,288] | 204 [203,206] | 1.387 | 183.6 |
| Disconnected two FE N20160/K65/T8 | 511 [508,516] | 360 [356,362] | 251 [246,254] | 1.434 | 225.9 |
| Two FE N15360/K96/T8 | 714 [712,720] | 497 [494,501] | 306 [305,313] | 1.624 | 275.4 |
| Serial two FE N12000/K65/T1 | 1322 [1314,1328] | 1286 [1277,1302] | 601 [599,603] | 2.140 | 540.9 |
| Unbalanced two FE N15120/K24/T4 | 98 [96,100] | 97 [97,99] | 71 [68,76] | 1.366 | 63.9 |
| Unbalanced three FE, multiple slopes, N28800/K10/T4 | 192 [191,195] | 194 [193,200] | 212 [210,215] | 0.915 | 190.8 |

The multiple-slope design uses g1[z1,z2] and g2[z2]. Paired changes by row:

* -1/0/+1/+7/+7 ms
* -149/-160/-148/-156/-151 ms
* -218/-219/-216/-213/-223 ms
* -33/-42/-28/-20/-42 ms
* -3/0/+2/+1/-3 ms
* +2/-2/+8/+2/-1 ms

The two threaded cases above the binding limit improve 29.5% and 30.4%, with
all ten pairs improving. The serial wide case improves 2.7%, with all five
pairs improving. The below-limit K63 and multiple-slope medians lose 0.7% and
1.0%. No fresh case clears the stock target; further reductions of 35.1%, 37.2%,
44.6%, 57.9%, 34.1% and 1.6% respectively would be needed at these stock medians.
Those are target budgets, not promised attainable times.

Controls, three rotations:

| Case | Baseline | Candidate | Stock | Native paired changes ms |
|---|---:|---:|---:|---|
| Fast categorical two FE N16320/K16/T4 | 115 [114,115] | 117 [113,117] | 65 [57,65] | +2/+2/-1 |
| Low one FE N16320/K12/T4 | 40 [40,40] | 39 [37,40] | 26 [25,28] | 0/-1/-3 |
| Low weak barbell N7680/K4/T4 | 30 [30,31] | 29 [29,29] | 160 [155,160] | -1/-1/-2 |
| Fast slopes on both, N30720/K6/T4 | 123 [119,123] | 123 [123,125] | 122 [120,123] | 0/+2/+4 |
| OLS categorical two FE N16320/K16/T4 | 34 [33,35] | 35 [34,35] | — | +1/+1/0 |
| OLS slopes on both, N30720/K6/T4 | 66 [65,69] | 67 [66,68] | — | -3/+2/+2 |

OLS exports are exact but both control medians lose 1 ms. Fast categorical
loses 2 ms; fast slopes retain two losing pairs. No universal timing
non-regression claim. The small one-FE comparison does not resolve the
historical large one-FE stock-mean discrepancy.

## Scientific and memory checks

The final artifact passes **58 distinct Rust tests**, including the independent
weighted-contrast joint oracle in both scan organizations, changing frames,
rank-deficient/disconnected slopes, zero RHS, failure revocation, strict retry,
rollback and all 51 PPML memory tests. Production ABI/panic containment passes.

**Seven licensed fixtures pass**, with explicit Stata return codes and terminal
sentinels. N4800/K8 and N4800/K65 explicit Poisson/Hessian/sandwich fixtures each
cover unit/FW/PW at T1/T4 with robust and cluster inference. Further checks cover
N4800 ordinary factors and a numeric product with PW and FE shifts 0/1000, a K65
allocation audit, multipartition slopes, separation and checkpoint transaction failure.
The separate 1e6-shift failure is not rerun or repaired.

The final headline blocks contain **192 applications**, **180 public-state
checks**, **nine small independent Python oracle results**, **66 native pairs**,
**60 baseline/stock** and **60 direct candidate/stock pairs**, all passing.
All 66 native exports are exact. There are no scientific exclusions, application
failures, timeouts, detected overlaps or monitor errors. The rejected first
organization is separate: combined totals are 246 applications, 18 independent
oracle results, 84 exact native pairs and 78 direct stock pairs. Nine diagnostic
applications are outside these headline totals.

All 60 final PPML pairs retain outer/inner iterations, halvings, fallback/strict
rejections, rank/DoF, modeled peaks and scratch metadata. Tracked peaks match
in 59 pairs. One excluded fast categorical pair rises from 24,111,769 to
24,485,864 bytes, a 374,095-byte increase; preserve that observation. There is
no new heap bank or admission formula in the implementation. The K65 actual-
plugin audit records zero allocator events/growth during sixteen ordinary joint
steps, stable capacities and fresh/warm equivalence. That window directly
exercises the new column-oriented path, not every allocation lifetime.

An initial diagnostic controller stopped before fitting because sandboxed
process inventory was denied. Zero applications started. Its receipt is
preserved; the authorized retry retained the contention check and completed.
Do not relabel this preflight failure as a passing controller or a fit failure.

## Isolated phase attribution and next work

Baseline diagnostic `jointtriledger1`, plugin
`128a7ac697827756969b940e8f17aa69e10eb575d633f243913a49b85634c124`,
has the baseline implementation plus captured probes. Candidate diagnostic
`columnflowledger2`, plugin
`ea8e63b6a7d10dd5519c44b573b4e613b13575057e537ebf71f5a04db3463031`,
adds the same spans to the final candidate. Six applications and public-state
audits pass after all headline work, using development seed347.

| Inclusive phase, ms | K24/T8 baseline→candidate | K64/T8 baseline→candidate | K65/T1 baseline→candidate |
|---|---:|---:|---:|
| Norms, six calls | 5.513→5.398 | 28.016→3.812 | 19.230→16.785 |
| Product tiles, six calls | 4.886→5.365 | 92.810→23.056 | 1021.704→1003.853 |
| Reconstruction, six calls | 11.245→11.763 | 54.656→25.826 | 37.790→18.309 |
| Exclusive recurrence | 0.950→0.955 | 2.162→2.128 | 2.130→2.107 |
| Whole joint directions | 33.254→34.535 | 193.961→70.058 | 1131.163→1087.239 |
| Canonical rank, two calls | 16.364→17.106 | 42.219→38.588 | 34.987→35.934 |
| Retained reprojection, one call | 13.679→14.838 | 37.630→37.986 | 32.815→31.640 |
| Inference | 4.646→4.733 | 23.373→22.746 | 19.114→19.422 |
| Core publication wrapper | 3.559→3.611 | 11.447→12.519 | 8.690→11.578 |
| Native command root | 114.496→117.911 | 423.757→292.943 | 1311.743→1267.387 |

The first two cases have N16320; the serial case has N12000. Rows nest and must
not be summed. These are single intrusive clocks, not headline ratios. Stable
six-call counts, exact exports and unchanged iteration metadata support reduced
preparation/reconstruction cost rather than less stringent solves.

The strongest next opportunity is now explicit: **recipe-backed serial finite
products still use the old complete-row kernel**, consuming about 1,004 ms,
79% of the serial diagnostic root. The adopted scan policy did not reach this
kernel. Evaluate source-oriented/bounded product formation there using the
available-storage distinction, with fresh evidence and the existing scientific
checks. Do not tune the fixed accumulator width or add benchmark-specific
thread/width thresholds. Seed355 remains frozen validation, not tuning input.

After that serial work, return to current-frame canonical/product ownership.
At threaded K64, canonical rank costs about 39 ms, retained reprojection 38 ms,
outer score certification 28 ms and inference 23 ms. Their independent scientific
purposes remain; investigate duplicated data access and valid shared state.
Fast categorical and one-FE losses also remain substantial. This report supplies
measured outstanding costs rather than claiming the 0.90 target is unattainable.

## Immutable receipts and limits

The archive contains **10,485 logical receipts**, 2,207 unique payloads and
16,400,650 bytes. SHA-256:

`19f12fd636ae693db5ab79549da23bdd8321332cabd1fde2baaebed9d032f8bf`.

`restore_receipts.py NEW_DIRECTORY` verifies the container, index and every
payload. Restoration was checked before handover. The `columns/` prefix contains
both candidate artifacts, source hashes, declarations, test/fixture receipts,
timing distributions, all audits, binding counter lines and isolated phases.
`summary.json` covers the final candidate; `prototype1-summary.json` covers the
rejected organization. Raw licensed logs, generated datasets/means and binaries
are excluded with hashes where applicable; local paths are sanitized.

Physical-memory-cap and cross-platform confirmation remain deferred. Static
allocation inventory drift, weighted constrained-fit failures, nonretaining-CG
instability, historical stock-mean discrepancies and the separate large-FE-shift
failure retain their scope. No SCC retry, stopped-campaign restart, push,
release or qualification was performed.
