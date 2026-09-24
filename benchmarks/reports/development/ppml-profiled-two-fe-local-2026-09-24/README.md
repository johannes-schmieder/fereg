# Profiled two-FE PPML lifecycle: local evidence, 2026-09-24

## Decision

Adopt `profilecurve5` under [ADR 0174](../../../../docs/adr/0174-profiled-two-fe-poisson-trials.md).
Analytically optimize one categorical intercept partition at every trial along
the existing certified joint Newton direction. Use one canonical final Newton
checkpoint, preserve the independently confirmed accepted point, and reuse its
exact-current-weight products only after the existing independent final checks.
The implementation removes duplicate final work without adding another ordinary
direction engine or changing acceptance tolerances. One FE, slopes, three-plus
FE and mu separation retain their previous mechanisms.

Fresh eligible medians improve 3–16%. Seven eligible cases improve every native
pair; the wide disconnected case has a serious preserved latency tail. Four of
ten multi-FE medians meet the 10% stock lead, two beat stock with less margin,
and four lose. This selected remaining-gap screen is not a package-wide win rate.
The broad goal remains unfinished. No fresh cases were used to fit a guard.

## Exact sources, artifacts and measurement

Baseline source is **17a239ee05de5b23ba0611de1ebeedb7df694ff0**. Each candidate
captures that base plus its archived recipe. Final production changes are in
ppml/memory.rs, mod.rs, prepared_irls.rs, prepared_estimator.rs and the new
profiled_intercept_curve.rs. Its source inventory also captures the new component
truth fixture. All **550 captured inputs match** final source; the separately
added allocation fixture exactly matches its tested target copy and has its own
hash. The archive includes complete source inventories and captured inputs.

| Artifact | Role | Plugin SHA-256 |
|---|---|---|
| canonicaltiles2 | Baseline; all 548 inputs match 17a239ee | e7c6d4bd81433f0a7067107bf3c167e02146845122c9c53e057673beccc2f529 |
| profilecurve1 | Curve with previous joint strict witness | 51572105d122084df00eb3a7f417860e6f97dcfbafc7d6a565a67614f354840e |
| profilecurve2 | Full-product ordinary direction; refused in review before fits | 6345a88976598b917dbc46a80bb9813ccfcfb05e9bf82aab0bf9e5f18dbc845d |
| profilecurve3 | Corrected full-product ordinary direction; rejected | 76117dd5d5cfe1dabf1f0a94a359646bf4a5225b051d50c953b87543d7438d60 |
| profilecurve4 | Existing joint directions plus one canonical checkpoint; selected prototype | a69fda8f2c37cb6c7163711bb3b82e4bcb04c8f9b20eec4f1d18ff332274e183 |
| profilecurve5 | Final production candidate; 550 inputs match committed computational source | 569c601e8143e0744d9359f44d9fc966a09b244ee2a29a51ad0e1840a5b2bbd0 |

The comparator is pinned, unmodified fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with glm.tol=1e-10, fixef.tol=1e-11, robust inference and requested fitted means.
All ranked scientific comparisons pass. Default-stock accuracy failures remain
separate; ordinary fixest weights are never a Stata frequency-weight oracle.
Native threads match declarations; BLAS helpers use one thread. The existing
harness times the whole public estimation command after a separate <=7680-row
warmup, excluding common dataset import. These are warm, not cold, results.

No compilation, profiling, auditing, archiving or competing fits ran alongside
headline timings. Every completed fit has clear overlap/error lists under the
captured ancestry/GUI-activity monitor. Clean monitoring does not prove absence
of ambient OS, filesystem or scheduling load. The unrelated GUI was untouched.

## Fresh distributions, seed 202609391

Five rotating baseline/candidate/stock repetitions; low memory unless stated.
All request robust inference and saved means. Values are medians [min–max] in ms;
C/B is the full matched-pair ratio range. Case numbers in later tables follow
this order, starting at zero. The one-FE control is outside the multi-FE goal.

| Design | Baseline | Candidate | Stock | C/stock | Paired C/B |
|---|---:|---:|---:|---:|---:|
| Connected narrow N22560/K2/T4 | 48 [47–50] | 43 [42–44] | 31 [28–31] | 1.387 | 0.8600–0.9149 |
| Disconnected N44160/K10/T8 | 131 [129–131] | 121 [121–123] | 79 [78–79] | 1.532 | 0.9237–0.9389 |
| Connected fast N22560/K10/T4 | 67 [65–68] | 62 [61–63] | 57 [54–58] | 1.088 | 0.9118–0.9403 |
| Unbalanced N24480/K28/T4 | 113 [112–120] | 95 [94–96] | 116 [113–118] | 0.819 | 0.7833–0.8496 |
| Connected serial N17280/K10/T1 | 70 [67–72] | 59 [59–62] | 102 [101–104] | 0.578 | 0.8429–0.8806 |
| Weak graph N15360/K8/T1 | 161 [160–165] | 138 [137–141] | 953 [951–958] | 0.145 | 0.8364–0.8812 |
| Connected wide N28800/K104/T4 | 572 [569–578] | 541 [537–542] | 891 [876–896] | 0.607 | 0.9343–0.9491 |
| Disconnected wide N55200/K102/T8 | 1109 [1095–1131] | 1071 [1067–4733] | 968 [946–1005] | 1.106 | 0.9469–4.3224 |
| Two slope partitions N22560/K10/T4 (control) | 104 [104–105] | 105 [103–106] | 112 [107–112] | 0.938 | 0.9904–1.0192 |
| Three FE unbalanced N24480/K10/T4 (control) | 92 [91–94] | 92 [92–99] | 95 [91–96] | 0.968 | 1.0000–1.0532 |
| One FE fast N12480/K6/T1 (control) | 39 [38–41] | 39 [36–40] | 21 [18–22] | 1.857 | 0.9474–1.0000 |

The narrow, disconnected K10, fast K10 and disconnected K102 medians still lose
to stock. Slope and three-FE controls beat stock without a full 10% margin. Their
prior source-bound wins are not transferred to this artifact. The slope control
adds 1 ms; three-FE and one-FE medians are unchanged. OLS controls below include
small paired losses as well.

**Preserved wide-case tail:** c7/r3/candidate took 4733 ms, with unchanged
four outer iterations, 46 inner work units, tracked memory and scientific result.
Its FE projection phase was 1259.57 ms (other runs 284.20–298.92), and publication
2304.61 ms (others 36.29–39.82). Strict solving remained 322.39 ms. These locate
most of the delay but do not prove its cause. The original candidate arithmetic
mean is 1803.4 ms versus baseline1112.2 ms; the median improvement is not a tail-
latency guarantee. No observation is deleted or reclassified as invalid.

A separately predeclared five-rotation confirmation reused the exact frozen
case and unchanged binaries. Baseline: **1115 [1106–1133]**, candidate:
**1076 [1072–1264]**, stock: **950 [938–957]** ms. The 4733 ms stall did not recur,
but the 1264 ms candidate loses its pair (baseline1106). All other confirmation
pairs improve. Both distributions remain; tail latency is unresolved. Stop this
bounded investigation here rather than fitting an exclusion to c7.

## Measured attribution and memory

The candidate retains the existing joint absorption and finite solves. It saves
one outer iteration in narrow, unbalanced and serial cases. In the others, the
largest consistent reduction is final verification. Native phase medians in ms:

| Case | Outer B/C | FE projection B/C | Finite B/C | Strict final B/C | Objective/trial B/C |
|---|---:|---:|---:|---:|---:|
| 0 | 4 / 3 | 7.86 / 6.38 | 3.86 / 3.20 | 8.01 / 4.28 | 2.96 / 2.53 |
| 1 | 4 / 4 | 30.45 / 31.22 | 10.34 / 10.21 | 35.07 / 22.99 | 6.32 / 7.08 |
| 2 | 4 / 4 | 14.28 / 12.96 | 4.40 / 4.31 | 13.87 / 9.26 | 3.24 / 3.64 |
| 3 | 4 / 3 | 28.33 / 22.38 | 9.81 / 7.64 | 24.37 / 15.23 | 4.18 / 3.47 |
| 4 | 4 / 3 | 14.78 / 12.08 | 7.42 / 5.73 | 13.88 / 7.67 | 3.15 / 2.94 |
| 5 | 5 / 5 | 57.44 / 58.40 | 9.02 / 8.99 | 64.60 / 39.61 | 3.14 / 3.93 |
| 6 | 4 / 4 | 150.25 / 149.02 | 50.50 / 50.19 | 124.10 / 92.08 | 47.10 / 46.84 |
| 7 | 4 / 4 | 291.15 / 288.25 | 115.69 / 114.29 | 350.78 / 312.17 | 88.18 / 88.07 |

Strict phase includes nested work; do not add it to nested phase totals. Inner
reported units fall 39→20,55→42,35→24,30→14,36→17,398→324,41→30,60→46.
These units combine different algorithms and are not comparable operation counts
against fixest. We do not attribute all public gains to fewer iterations.

The immutable owner adds log prior weights, group outcome totals and trial shifts,
plus identified slots. Complete admission charges sizeof(owner)+2*sizeof(usize)*K
+8*(N+2G), including temporary slot overlap and retention through strict checking.
No new N-by-K design or dense system is retained. Eligible median whole-action
tracked peaks and candidate modeled bytes are:

| Case | Baseline tracked | Candidate tracked | Change | Candidate model |
|---|---:|---:|---:|---:|
| 0 | 11,766,784 | 12,007,440 | +240,656 | 16,036,154 |
| 1 | 39,237,307 | 39,708,427 | +471,120 | 46,188,995 |
| 2 | 22,314,722 | 22,555,442 | +240,720 | 76,491,090 |
| 3 | 24,919,918 | 25,119,998 | +200,080 | 30,822,345 |
| 4 | 13,288,649 | 13,473,049 | +184,400 | 17,367,790 |
| 5 | 14,532,395 | 14,737,259 | +204,864 | 17,165,284 |
| 6 | 73,029,885 | 73,337,917 | +308,032 | 89,324,456 |
| 7 | 136,446,701 | 137,036,317 | +589,616 | 170,589,234 |

Excluded controls have identical tracked/model values. These are allocator/model
receipts, not RSS or physical constrained-memory qualification. Optional admission
can refuse the curve; existing solver paths remain available. Real sixteen-
iteration audits at T1/T4 report zero allocation events and zero current growth,
stable capacities and fresh/warm equivalence. The static inventory drift remains
an open failure and is not contradicted by these scoped audits.

## Rejected candidates and frozen-frame evidence

The previous source-bound profile at b6088330 put narrow public time at46 ms,
native core at34.62 ms, four complete joint calls at9.16 ms, but their recurrence
at only1.21 ms. Two canonical preparations totaled8.77 ms. That motivated changing
the complete Poisson lifecycle, not just the inner recurrence. Those numbers
belong to the earlier artifact and are not current-source timers.

An isolated centered solver formed a full reduced finite/remaining-FE matrix,
with explicit component anchors, direct centered Gram terms, conservative scaled
Gaussian solving, an unfactored backward check, and fresh original-input FE and
finite score certificates. Independent known WLS truth covered three disconnected
components, nonzero residuals, changing weights, T1/T4 and duplicate-control
refusal. It passed 36 frozen frame/thread combinations: four exact frames each
from three previous captures, T1/4/8, six alternating pairs per combination.

Ranges below are medians across the four frames, milliseconds; diagnostic score
translation was outside the candidate timer. They are not whole-public claims.

| Frozen family | Threads | Baseline | Dense candidate |
|---|---:|---:|---:|
| Connected K2,89 coordinates |1|2.59–2.97|2.02–2.37|
| Connected K2 |4|2.30–2.50|1.63–1.89|
| Connected K2 |8|2.29–2.31|1.67–1.71|
| Disconnected K10,710 coordinates |1|10.21–10.34|35.03–35.48|
| Disconnected K10 |4|8.31–8.50|33.06–33.38|
| Disconnected K10 |8|8.16–8.24|34.26–34.47|
| Unbalanced K26,121 coordinates |1|11.90–11.95|12.15–12.25|
| Unbalanced K26 |4|9.66–9.89|6.75–6.92|
| Unbalanced K26 |8|9.43–9.48|4.84–4.88|

The disconnected T8 owner requires40,561,360 bytes versus684,136 for narrow.
Reject the global dense representation. A stock frozen-inner comparison was
not run; the stock-demean recipe is preparatory evidence only. We do not claim
complete operation attribution against fixest. Diagnostic test binary SHA-256:
`0bb73476291372067fc5b267ec2b0abb47341f60622e683743327ea6b9cfb1bb`.

Three public development candidates used seed202609347, three rotations on
connected N21120/K2/T4 low, disconnected N42240/K10/T8 low, connected N21120/K10/T4
fast, and unbalanced N23040/K26/T4 low. Medians B/C/stock, ms:

| Candidate | Narrow | Disconnected | Fast | Unbalanced |
|---|---:|---:|---:|---:|
| Curve alone, profilecurve1 |46/44/31|119/123/77|62/62/54|103/95/103|
| Full ordinary products, profilecurve3 |46/41/30|118/118/77|63/56/52|102/91/103|
| Existing joint + one checkpoint, profilecurve4 |46/40/31|118/110/77|62/57/55|103/85/103|

Curve1 retains expensive final duplication and regresses disconnected work.
Curve3's extra ordinary full products offset checkpoint gains. Curve4 is selected;
production curve5 adds conservative memory/capacity accounting and focused tests.
Only curve5 supplies fresh adoption timings. Every earlier distribution is archived.
No rejected direction engine is left in production.

Profilecurve2 was built but never fitted: review found an advanced canonical
response anchor without a matching solve. Curve3 leaves that frame untouched
until an actual fallback/checkpoint, correcting ownership before its first fit.
The failed initial builder assertion, centered-probe build omissions/argument
correction, and initial fresh N12480 weak-graph generation refusal are retained.
The latter happened before all fits; valid N15360 uses40 branches and the warm
N7680 uses20. No frozen reference design was silently changed. The first archive recipe then
refused mixed absolute/relative paths before creating an archive; the corrected
recipe and original failure are preserved.

## Scientific and regression checks

Final artifact: **33 distinct focused Rust tests**, five licensed science fixtures,
and both T1/T4 allocation fixtures pass. Plugin ABI/panic containment passes.
No broad Rust suite or qualification profile is claimed. Source-bound logs include
expected caught-panic injections; they are passing rollback tests, not hidden
application failures.

The analytic optimum test covers invariant group shifts, threads and weight
scales1,1e60,1e-60, with zero-outcome-group refusal. The new explicit truth fixture
uses paired equal-design observations whose opposite errors cancel: true finite
coefficient .2, known predictors/means, two disconnected components, offsets,
PW disparity1 and1e13, fast/low,T1/T4. It has nonzero residuals independently of
any reference. The expanded final-product test compares canonical fallback with
normal reuse, wrong-generation refusal and corrupted-predictor refusal for one
and two FE, weighted/unweighted, cluster/saved effects, fast/low,pools1/2,T1/T4.
Failed transform/overflow tests preserve accepted eta, means, coefficients and
capacities. Existing strict rank/rebuild/rollback tests pass without weakened
numerical envelopes; fallback-only tests explicitly disable the optional curve.

Licensed controls also include canonical PPML products, generated terms with
FW multiplicity and PW clustered scores, multipartition slopes, and three-FE OLS.
Fresh seed202609392 supplies three additional small explicit-design Poisson
problems (connected, disconnected, unbalanced), comparing coefficients, means,
objective, full Hessian and robust sandwich for both native artifacts and stock.
**All nine fits pass these independent oracles.**

Across development, fresh, oracle and tail confirmation: **297 public states,
198 baseline pairs and99 direct candidate/stock pairs pass**; none are unranked.
The final fresh/oracle/confirmation subset comprises189 public states. No timeout
or completed application/scientific failure occurred. All losses remain visible.

Focused OLS screen, seed202609392, N16320, three rotations: one-FE robust/all-output
15→16 ms, two-FE28→28, low two-slope97→97, low three-FE59→60. All24 applications
and12 science/output pairs pass. These small losses are not attributed to the
PPML-only algorithm. Full per-call distributions remain in final-review.json.
The 77 primary,16 holdout and111 native-control definitions are untouched.
Seeds202609391/392 are now consumed/frozen; earlier frozen seeds remain so.

## Outstanding work and next bounded milestones

1. Narrow public43 ms versusstock31 needs another15.1 ms to reach0.90×. Current
   strict work is4.28 ms; deleting even all of it could not close that gap. The
   earlier recurrence ceiling also remains too small. Target initial canonical
   preparation and repeated loading/binding/normalization/output passes together,
   and measure the whole command against stock. Any reuse must separate immutable
   topology from current weights and retain independent rank/model-space authority.
2. Disconnected K10 remains121 versus79; ordinary FE+finite phases total about
   41.4 ms, strict23.0 ms. Wide disconnected remains1071 versus968; projection
   288.2 ms, finite114.3 ms and strict312.2 ms are material. Replace worker-private
   full group/RHS reduction duplication with partitioned ownership or bounded
   tiles, enabling component-wise canonical products under the existing budget.
   Do not resurrect the measured global dense710-coordinate solve or fit a guard
   to the fresh disconnected topology. Retain and investigate the latency tails
   separately from arithmetic gains.
3. Fast K10 needs62→51.3 ms. Evaluate direct current-weight canonical products
   writing into the existing retained dense representation, sharing the accepted
   low-mode mathematics and certificates rather than changing solver family.
   Test generation/ownership and all-output controls before public timing.
4. Slope105/112 and three-FE92/95 need roughly4.2/6.5 ms for a10% lead. These
   controls did not inherit this two-categorical-FE change. Generalize successful
   lifecycle or final-state mechanisms only when their rank/model-space checks
   apply, with independent changing-weight slope and Poisson/Hessian evidence.

Known weighted constrained-fit/nonretaining-CG span failures, the canonical LSMR
reporting assertion, static allocation inventory drift, seed378 comparator issue
and deferred SCC/Linux/thread/physical-memory qualification remain open. No remote
mutation, SCC retry, stopped campaign or release action occurred.

## Immutable receipts

`receipts-content-addressed.zip` contains sanitized, deduplicated logical receipts.
`receipt-index.json` maps logical names to blobs; `archive-hashes.json` retains
original and sanitized hashes. The adjacent SHA-256 identifies the archive.
All **12,512 logical receipts / 2,671 payloads / 24,969,269 bytes** were verified
and restored. Archive SHA-256:
`3cbb1b7bfda867da25bf507b00f2c16034e34731f1e1850f982f766dc19613e1`.
`restore_receipts.py` verifies and restores every logical receipt. Raw licensed
logs, binaries and data are excluded; hashes, recipes and protocol sentinels are
preserved. Frozen-frame input hashes link back to the preceding tiled-output
report's capture provenance. Local work roots are target/ppml-profile-curve-20260924
and target/ppml-concentrated-two-20260924. STATUS.md is the sole mutable handover.
