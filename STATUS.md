# fereg 0.5.0 beta status

**Last updated:** 2026-09-23
**Branch:** main
**Mode:** prototype/development
**Release state:** public OLS and PPML beta surfaces; no qualified release,
tag, or Windows scientific qualification.

This is the sole mutable progress/handover record. Historical receipts remain
unchanged under benchmarks/reports/. This compact handover supersedes the
completed optimization/calibration investigation chronology formerly here.

## Retained canonical tiles accepted; grouped product ownership next — 2026-09-23

The active objective remains whole public PPML **<=0.90 times comparable pinned
stock fixest wherever achievable**. This is a supporting improvement, not
completion of that objective. This opening supersedes older next-step orders
and unused-seed statements below.

Accept `canontile1`, based on production `68e3ba75`, under
[ADR 0138](docs/adr/0138-retained-scratch-standardization-and-norms.md) and the
[source-bound report](benchmarks/reports/development/ppml-canonical-tiles-local-2026-09-23/README.md).
All **539 captured computational inputs** match plugin
`5ede5b83f42a4a291e56dab2b27a94c3711197ac3a32d3ef6d64870461a1b205`.
The archive was restored and **7,126 logical receipts** verified, SHA-256
`f8fbd79ab2fce16bf7b605b2ea038cceb7eac489a882bb41fadea6b7d9ec842b`.
Baseline is `onelow1`; all earlier categorical, slope, loading, inference and
OLS improvements remain. No push or release.

### Adopted behavior and checks

Retained scratch moments compact and standardize selected values once per row
and Gram-row partition in the already admitted private tile. Strictly increasing
source positions make forward compaction safe. Scratch bytes and every
coordinate's observation/FMA order remain unchanged. This generalizes dense
moment reuse to low PPML across FE families without new storage or thresholds.

Retained control projection also borrows freshly certified column norms only
for one categorical intercept FE at exactly matching weight normalization.
Other projectors, multiple effects and mismatched scales retain explicit scans.
No norm survives its absorption call. Raw means/norms, canonical rank and QR
fallback, all projection/finite/score/objective/Hessian/reconstruction gates,
separation and publication semantics remain unchanged.

**376 distinct Rust tests**, production-plugin ABI/panic checks and **ten
licensed runs of eight fixtures** pass. Independent scalar moments cover widths
through K257, T1/T4/T16, weights, scaling and noncontiguous columns; scratch is
reread to prove immutability. Full-dummy Poisson/Hessian/sandwich fixtures cover
unit/FW/PW, offset, robust/clustered inference and generated terms. Fixed-process
pool1/T1, pool1/T4 and pool2/T4 allocation audits have zero events/growth,
stable capacities and warm/cold equivalence.

Public receipts: **146 passing applications**, **130 PPML states**, **46 native
PPML pairs**, **38 baseline/stock pairs**, **38 direct candidate/stock checks**,
nine small independent oracle checks and **eight exact OLS pairs**. Eight
excluded multi-FE/slope PPML pairs also have exact scientific exports. Isolated
diagnostics add sixteen passing applications/states and six pairs. No timeout,
application failure, scientific discrepancy, oracle skip or detected contention.

### Gains, losses and memory

Robust inference plus fitted means; stock glm.tol=1e-10/fixef.tol=1e-11,
matched threads. Median ms, five decision rotations and three fresh rotations:

| Low one FE | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| N30720 K6 T4 | 59 | 59 | 25 | 2.360 |
| N61440 K10 T4 | 122 | 119 | 53 | 2.245 |
| N30720 K32 T4 | 167 | 150 | 121 | 1.240 |
| Fresh N46080 K8 T8 | 86 | 85 | 39 | 2.179 |
| Fresh N61440 K10 T1 | 165 | 160 | 95 | 1.684 |
| Fresh unbalanced N61440 K16 T4 | 158 | 149 | 113 | 1.319 |
| Fresh N12288 K65 T4 | 234 | 208 | 127 | 1.638 |
| Fresh small N3840 K3 T1 | 23 | 22 | 13 | 1.692 |

K32/K65 gains are **10.2%/11.1%**, all eight pairs improving. Narrow gains are
small or absent. Fresh pairs have twelve gains, two ties and one loss. Fast
one-FE medians are 104/104 versus stock54; fast three-FE control loses
182/184.5 ms over two pairs. Preserve these losses and the full distributions.
**Every measured one-FE case still loses stock.** Small-fit rounding prevents a
precise margin claim. No general timing non-regression claim follows.

All measured modeled/tracked peaks, scratch bytes and outer/inner counts are
unchanged. Low N61440/K10/T4 tracked peak delta remains 34,603,565 bytes,
modeled 43,061,399, observed scratch 4,915,200. No new design/tile/cache bank.
Low against ordinary stock is not physically memory-matched.

### Next bounded milestone: change canonical product ownership

The refined paired diagnostic low N61440/K10/T4 plugin median is
109.126 -> 102.217 ms. Two finite-moment calls fall 10.176 -> 7.245 ms; two
duplicate within-norm scans disappear. All six canonical projections remain.
Initial and strict WLS still cost **22.325 + 20.128 ms inclusive** in the
candidate. Fast costs **16.849 + 14.160 ms inclusive**. Six objective evaluations
remain about **11 ms**, ordinary profile directions only 5.656 ms low/3.420 fast,
and low core publication 9.248 ms inclusive. Nested clocks are not additive;
the report preserves exclusive costs and diagnostic variability.

1. **Build one coherent grouped canonical product owner.** Initial rank and
   accepted final Hessian should consume current-weight centered group products
   directly from original prepared columns. Retain stable centered arithmetic,
   deterministic FE-collinearity/rank policy and independent original-input WLS
   witnesses. An implicit centered row view should serve reconstruction and
   inference; materialize a design only where retained QR/fallback actually needs
   it. Investigate ownership/lifetimes through session products first. Do not
   accumulate another competing N-by-K cache. This design was investigated,
   not implemented by the present tile change.
2. **Set a whole-command budget before choosing the representation.** At this
   stock median, low needs **119 -> <=47.7 ms**, fast **104 -> <=48.6 ms**. At most
   two coherent candidates, measured first on identical frozen current-weight
   frames, then the complete public command. Require independent WLS/Poisson/
   Hessian checks, rank-deficient/extreme-scale/generated controls, and declared
   fresh cases. Small factorization tuning cannot close this gap: guarded
   Cholesky and retained QR already exist, and materialization/scans dominate.
3. **Share accepted-state work as part of that budget.** Current means, working
   state, objective and publication repeat compatible scans. Establish a frame
   owner with explicit sample/prior/current-weight generations. Immutable
   outcome terms may be cached, but keep an independent final objective replay.
   Examine the Stata control-plane portion of the whole-command gap as well;
   keep the one-call Rust boundary and transactional publication. A modest
   objective cache alone is not completion.
4. **Then refresh multi-FE/slope whole-command attribution and broaden coverage.**
   Existing coefficient Schur/PCG, weighted geometry, coupling, joint directions,
   low tiles and categorical joint steps are already adopted. Use measured
   setup/canonical costs to select warm starts or shared geometry work. Protect
   OLS and categorical/slopes with focused controls and include weights,
   generated terms, output costs, memory limits and small/no-FE fits adaptively.
   Do not reopen closed serial weighted-slope or rejected residual probes without
   a materially different organization and cost hypothesis.

Each milestone must end in a coherent measured commit; stop expensive low-return
threshold tuning and move to the next cost center. The whole-command 10% goal
remains active. Seeds **202609335 through 202609338 are consumed**; choose and
predeclare fresh cases before the next candidate results. All current builds,
tests, fits, profiles and archives are terminal. Preserve static inventory drift,
mixed-thread allocation retirement, constrained-weight/nonretaining-CG issues,
earlier comparator discrepancies and SCC/platform/physical-cap deferrals.
Frozen 77 primary, 16 holdout and 111 native-control definitions remain unchanged.

## Low-memory concentrated one-FE PPML accepted; canonical construction next — 2026-09-23

The active objective remains whole public PPML **<=0.90 times comparable pinned
stock fixest wherever achievable**. This checkpoint improves low one-FE PPML;
it does not complete that objective. This opening supersedes older next-step
orders and unused-seed statements below.

Accept `onelow1`, based on `e6f63a1b`, under
[ADR 0137](docs/adr/0137-low-memory-one-fe-poisson.md) and the
[source-bound report](benchmarks/reports/development/ppml-one-fe-low-local-2026-09-23/README.md).
All **539 captured computational inputs** match plugin
`c69cc27f61955232ee73f3183ec60f947203c0d5d9e91164ef97f8d58aac78c0`.
The archive was restored and **8,304 logical receipts** verified, SHA-256
`cedb9532dd73a72fa91a1b8914e92884068557ed6cd862592924d14951633ba6`.
Baseline is `catjoint2`, production `e6f63a1b`; previous categorical, slope,
loading, inference and OLS improvements remain.

### Adopted behavior and validation

Actual one-intercept-FE MAP-owned publication PPML now admits concentrated
ordinary Newton steps in low mode. A short view borrows original prepared numeric
columns or generated-column recipes; canonical sample/topology and frozen basis
identities are checked. Fast mode retains its dense representatives. No new
N-by-K matrix or random group access into scratch. Current means and centered
moments are reconstructed at each step. Independent original-input scores,
objective, two-pass convergence and strict canonical WLS remain acceptance gates.
The existing strict/final product reuse now reaches low sessions only at the
exact accepted q*mu frame with valid generations, rank/bread and reconstruction.
No changed tolerance, sample, weight, DoF, separation or transactional policy.

**348 distinct Rust tests**, production-plugin ABI/panic checks and **ten licensed
runs of eight fixtures** pass. Independent full-dummy Poisson/Hessian/sandwich
checks cover unit/FW/PW, offset, robust/clustered inference and generated factors.
Production allocation audits at pool1/T1, pool1/T4 and pool2/T4 in separate
processes have zero events/growth, stable capacities and warm/cold equivalence.

Public evidence: **180 passing applications**, **164 PPML states**, **51 native
PPML pairs**, **50 baseline/stock pairs**, **38 direct candidate/stock pairs**,
six small independent oracle checks and **eight exact OLS pairs**. Eight excluded
multi-FE/slope PPML pairs also have exact scientific exports. Separate diagnostic
builds add eight passing applications/states and two pairs. No application failure,
timeout, scientific discrepancy or detected contention in these blocks.

The first pilot oracle request exceeded its existing 1,200-FE-column bound by
29 columns. Its failure is preserved; the corrected bounded audit explicitly
skips that cell's independent dense oracle and keeps its public-state/comparator
checks. No bound changed. Independent licensed larger/weighted/generated oracles
pass. A build CLI error, instrument-name error and empty test-filter attempt are
preserved and separately identified; final test counts include executed tests only.

### Measured gains, losses and memory

Robust inference plus fitted means, matched threads, stock glm.tol=1e-10 and
fixef.tol=1e-11. Median ms; development seed335 has five decision rotations,
fresh seed337 has three. Full ranges and every pair are in the report.

| Low one FE | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| N30720 K6 T4 | 80 | 59 | 27 | 2.185 |
| N61440 K10 T4 | 180 | 124 | 54 | 2.296 |
| Fresh N46080 K8 T8 | 141 | 91 | 37 | 2.459 |
| Fresh N61440 K10 T1 | 223 | 167 | 98 | 1.704 |
| Fresh unbalanced N30720 K16 T4 | 151 | 98 | 46 | 2.130 |
| Fresh disconnected generator N15360 K3 T1 | 47 | 38 | 15 | 2.533 |
| Fresh small N3840 K3 T1 | 20 | 19 | 11 | 1.727 |

Decision reductions are **26.3–31.1%**, fresh **5.0–35.5%**. All ten low decision
pairs and 14/15 fresh pairs improve. Every one-FE case still loses to stock.
The fast excluded control initially loses 103/105 ms; five identical-input pairs
then give 104/103. The small fresh case's 22/21 ms loss triggers five fixed-input
rotations, all native gains, medians 26/19/12 baseline/candidate/stock. Preserve
both distributions; no universal non-regression or small-fit percentage claim.

Fresh tracked peak increases **0.108–1.891 MB**, within admitted modeled storage;
no new finite-design bank. N61440/K10/T4 peak is 32,712,369 -> 34,603,553 bytes,
model 43,061,399; scratch remains 4,915,200. The profile owner is charged through
strict solving and released before inference. Optional-state refusal retains the
canonical route. Low versus ordinary stock is not physical-memory-matched.

### Next bounded milestone

The refined isolated whole-plugin ledger uses non-overlapping exclusive spans
and keeps stock/production timings separate. At N61440/K10/T4 low it shows:
retained-control reprojects **4 -> 1**, ordinary frozen solves **2 -> 0**, full
rank constructions **3 -> 2**, strict solves **2 -> 1**. Diagnostic plugin time
falls **166.058 -> 110.242 ms**. The accepted route still spends **24.192 ms** in
initial canonical WLS and **24.046 ms** in strict canonical WLS, inclusive. Ordinary
profiled directions total only **5.599 ms**. Six objective evaluations cost about
**10.972 ms**; original-input outer scores remain five fresh checks at **3.106 ms**.
SPI loading is **4.932 ms**, core preparation **4.397**, inference **1.915**, core
publication **10.680 inclusive**, plugin publication **1.791**. See the report for
exclusive costs and unassigned core/IRLS/control-plane work; do not sum nested
phase clocks or use instrument timings as headline rankings.

1. **Redesign one-intercept canonical construction.** Inspect initial/final rank,
   scratch/retained-design construction and reconstruction passes. Build a bounded
   current-weight group-centered finite-moment owner where it can preserve the
   established rank policy, with independent original-input WLS/projection and
   final-Hessian witnesses. Avoid promoting speculative profile state into authority.
   Compare at most two coherent designs on the current source, then fresh cases
   and excluded controls. The N61440/T4 target is 124 -> <=48.6 ms low and about
   105 -> <=48.6 fast: tiny inner-kernel tuning cannot close that gap.
2. **Share accepted-state scans with explicit generations.** Investigate compatible
   means/predictors/working-state construction and repeated objective work. An
   immutable outcome/prior-weight contribution cache is justified by measurement,
   but its entire objective budget is only ~11 ms. Preserve independent final
   objective replay and invalidate on sample/prior-weight changes. Treat this as
   part of the larger cost budget, not completion after another small cache change.
3. **Return to multi-FE/slope setup and canonical boundaries after that bounded
   milestone.** Coefficient Schur/PCG, weighted factors, symbolic coupling, joint
   finite directions, low-memory tiles and categorical joint steps already exist.
   Reprofile full commands before selecting warm-start/recycled-preconditioner or
   shared current-weight geometry work. No reopening of closed serial weighted-
   slope tuning or rejected residual-extrapolation/local-stop probes without a
   materially different computational design.
4. **Broaden coverage adaptively.** Weighted/generated/output-heavy, no-FE/small,
   wide/rank-deficient and memory-constrained paths need proportionate public timing
   and oracle controls. Match actual work before ranking stock; retain losses and
   unresolved comparisons. Freeze candidate source before fresh confirmation.

Keep each milestone bounded, with a measured cost budget, independent oracles,
alternating stock rotations, a coherent commit and explicit next subsystem. Smaller
broad improvements are useful but do not finish the 10% goal. Do not call remaining
checking costs intrinsically unavoidable without examining their implementation.

Seeds **202609335, 202609336 and 202609337 are consumed**. **202609338** is unused
and not yet a declared matrix. All builds, tests, fits, diagnostics and archives
are terminal. Preserve mixed-thread allocation retirement, static inventory drift,
weighted constrained-fit/nonretaining-CG failures, earlier comparator disagreements,
and SCC/platform/physical-cap deferrals. Frozen 77 primary, 16 holdout and 111
native-control definitions remain unchanged. No push, release, qualification,
SCC authentication retry or stopped-campaign restart is authorized.

## Joint categorical low PPML accepted; whole-command one FE next — 2026-09-23

The active objective is whole public PPML **at least 10% faster than comparable
pinned stock fixest per case wherever achievable**. This checkpoint completes
production integration of the categorical joint engine; the objective remains
unfinished. This opening supersedes older implementation orders, proposed
mechanisms already adopted, and unused-seed entries below.

Accept `catjoint2`, based on `e196efa3`, under
[ADR 0136](docs/adr/0136-low-memory-categorical-joint-ppml.md) and the
[source-bound report](benchmarks/reports/development/ppml-categorical-joint-production-local-2026-09-23/README.md).
All **538 captured computational inputs** match plugin
`2697e671ce99185f173ad5be28ee0ca32b98e6eb5744103c386046139a8d8736`.
The archive was restored and **6,983 logical receipts** verified, SHA-256
`85b9506cfc58c59c40526d4f8f9251e06d2cdeaafa0823e9dbef298652217ed6`.
Baseline is `infscan1`, production `b9f2acbf`; its accepted inference scans and
all earlier slope improvements remain. The completed
[frozen experiment](benchmarks/reports/development/ppml-categorical-joint-probe-local-2026-09-23/README.md)
is source-bound preparatory evidence, not current public timing evidence.

### Adopted organization and science

Low categorical MAP with multiple effects and finite controls now uses the
existing joint finite/FE Schur engine for ordinary directions. Bounded scratch
tiles reuse current-weight geometry and FE-equivalent controls. Two/three FE
retain the established borrowed categorical projector for canonical rank/bread
work; four-plus FE use the generic coefficient projector there. One owned FE
design, no extra N-by-K bank. Fast categorical LSMR, one FE, mu separation,
other solver families and OLS retain their routes; slope routing is unchanged.

Fresh FE projection, original-input finite scores, reconstruction, objective,
rank and final-Hessian acceptance remain mandatory. The strict current-weight
joint Newton witness plus one canonical final construction now serves admitted
low categorical sessions too. Failed comparisons retain canonical retry and
rollback. No changed tolerance, sample, weight, DoF, separation or failure policy.

Initial `catjoint1` used coefficient solves at every categorical phase and failed
the existing dynamic-range test: predictor error **7.438e-10** against **1e-10**.
Supplemental local stopping and scheduling probes did not converge. Those source
snapshots and failures are preserved; neither numerical modification remains.
The accepted organization passes the original precision test. Do not resume
those probes without a materially different numerical design.

**347 distinct Rust tests**, actual-plugin ABI/panic checks and **nine licensed
runs of seven fixtures** pass. Public evidence: **175 passing applications**,
**139 passing PPML states**, **24 small independent Poisson/Hessian checks**,
**49 native PPML pairs**, **36 stock pairs**, **five stock mean discrepancies**,
**18 exact OLS pairs**, and **eight exact excluded PPML pairs**. Direct candidate/
stock comparisons independently give the same 36/41 result. A later diagnostic
trace adds one passing application/state. No application failure, timeout or
detected timing contention in these blocks.

The sparse development stock discrepancy is localized with neutral-start
explicit-design Poisson Newton using both LSMR and LSQR. Both converge at the
predeclared strict score gate, agree under the extra stability envelope, and
match native coefficients, covariance, means and objective. Stock misses five
means by up to **1.260e-6**, about **4.895** times the allowed error. Keep this
stock comparison **unranked**. The initial oracle score-cap and cross-tolerance
stability failures remain archived. No full FE rank/DoF claim follows from this
large diagnostic, and no existing oracle bound or tolerance changed.

### Current public timing and memory

Robust inference and fitted means; alternating native/stock rotations. Times
are median ms. Development seed335 has five rotations; fresh seed336 has three.
All ranges, pairs and losses are in the report. Low versus ordinary stock is
not physically memory-matched.

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Dense three FE N30720 K6 T4 | 559 | 350 | 245 | 1.429 |
| Sparse three FE N38400 K8 T4 | 5375 | 2049 | 15782 | Unranked |
| Strong two FE N30720 K6 T4 | 165 | 131 | 54 | 2.426 |
| Fresh dense three FE N46080 K8 T8 | 3108 | 1750 | 10633 | 0.165 |
| Fresh sparse three FE N30720 K4 T1 | 7945 | 2557 | 19393 | 0.132 |
| Fresh unbalanced three FE N61440 K16 T8 | 762 | 583 | 252 | 2.313 |
| Fresh disconnected two FE N23040 K8 T4 | 140 | 118 | 61 | 1.934 |
| Fresh weak barbell N7680 K4 T4 | 60 | 42 | 212 | 0.198 |
| Fresh four FE N15360 K6 T4 | 3670 | 83 | 169 | 0.491 |

Fresh native median reductions are **15.7–97.7%**, with all 18 pairs improving;
four fresh cases clear 0.90 times stock. The comparable remaining target gaps
are dense **350 -> <=220.5 ms**, strong two FE **131 -> <=48.6**, wide unbalanced
**583 -> <=226.8**, and disconnected **118 -> <=54.9**. Dense projection falls
**322.665 -> 111.772 ms**, but strict/final remains about **170 ms**. A separate
trace accepts both strict witness and final transaction, so this remaining cost
is not repeated rejection. Do not label its current implementation unavoidable
certification overhead or restart a long small-gap tuning cycle.

Two excluded OLS median losses, **67/69.5** and **64/68 ms**, become **71/70**
and **71/71** on five identical-input pairs. Scientific exports remain exact;
individual timing losses remain visible. The small two-FE PPML pilot's initial
**17/21/15 ms** becomes **17/17/15** for baseline/candidate/stock on five fixed-
input rotations. Preserve both distributions; no universal non-regression claim.

Fresh tracked storage increases **1.111–16.022 MB**, within the selected models.
Optional geometry/joint state and canonical-owner coexistence are charged over
the session lifetime and released before inference. Pool1/T1, pool1/T4 and
pool2/T4 production-plugin audits in separate processes have exactly zero
allocation events/growth, stable capacities and warm/cold equivalence. The
older mixed-thread Crossbeam reclamation failure remains open, as do static
allocation drift and physical-cap qualification. No extra N-by-K design.

### Remaining implementation plan

1. **Carry one-FE concentration through the whole command and into low mode.**
   This is the next subsystem. Reestablish a small current-source public/stock
   baseline first; earlier one-FE losses belong to the
   [inference-scan artifact](benchmarks/reports/development/ppml-inference-scan-local-2026-09-23/README.md).
   Analytic profiling, borrowed design, strict/final product sharing and parallel
   original-input scores already exist. Build a non-overlapping ledger of loading,
   initial rank, ordinary profiled steps, accepted-state checks, final Hessian
   and publication. Use one accepted-state owner for compatible means, predictors,
   group totals and moments, with explicit sample/weight generations. Implement a
   bounded streamed profiled route for low mode without a hidden N-by-K copy.
   Redesign repeated initial/final construction while retaining independent
   original-input score, rank, reconstruction and Hessian witnesses. Measure the
   repeated log_gamma(y+1) objective work before caching any immutable outcome/
   prior-weight contribution; preserve independent final objective replay and
   invalidate on sample/prior-weight changes. Seek whole-command gains large
   enough for the measured budget, not another isolated inverse optimization.

2. **Finish slope setup, warm starts and canonical boundaries.**
   Coefficient Schur/PCG, weighted factors, symbolic coupling, joint finite
   directions and low-memory tiles are already adopted. Reprofile full public
   fast/low commands and identical frozen frames; count geometry refreshes,
   scans, Krylov actions, reconstruction and independent checks. Compare coherent
   coefficient warm starts/recycled preconditioning with shared current-weight
   FE/finite preparation. A prior solution is only a guess. Retain fresh checks
   and a canonical rank/bread/Hessian boundary. Assess categorical checkpoint
   carryover from a new numerical design, not the rejected local-stop probes.
   Protect multipartition OLS; do not reopen closed serial weighted-slope tuning.

3. **Close coverage and choose routes from measured work.**
   Small adaptive screens should cover no-FE/small fits, wider/generated terms,
   multiple slopes, imbalance/connectivity, changing weights, rank deficiency,
   threads, memory budgets, robust/clustered inference and requested outputs.
   Weighted/generated/output-heavy PPML has partial science coverage but no broad
   current-source timing conclusion. Match sample, separation, objective, inference,
   outputs and threads before ranking stock; ordinary fixest weights are not a
   Stata FW inference oracle. Preserve all 77 primary, 16 holdout and 111 native-
   control definitions. Do not transfer historical OLS wins to this artifact.

4. **Establish a defensible per-case 10% margin.**
   Use fresh baseline/candidate/stock rotations and excluded controls; at least
   five decision pairs when spread/timer resolution could change the conclusion.
   Retain all losses and discrepancies. A family closes when the target survives
   confirmation or concrete measured costs explain the remaining limit. Stock
   stays unmodified; diagnostics remain separate. Do not use aggregate wins to
   hide a loss or call unexplained deficits certification overhead.

Each bounded milestone admits at most two materially different initial designs,
one decision screen and predeclared fresh confirmation, then a coherent commit
or documented rejection. Keep a cost budget; move mechanisms after two failed
candidates rather than extending threshold tuning. Smaller broadly useful gains
are acceptable but do not complete the objective. Protect OLS, weights, inference,
saved outputs, allocation failure and transactional publication proportionately.

Seeds **202609335 and 202609336 are consumed**; **202609337** is unused, not yet
a predeclared matrix. No build, fit, profiler or benchmark remains running at
this checkpoint. Preserve the mixed-thread allocation failure, static source
drift, weighted constrained-fit/nonretaining-CG failures, older comparator issues
and SCC/platform/physical-cap deferrals. No push, release, qualification, SCC
authentication retry or stopped-campaign restart is authorized.

## Bounded low-memory joint slopes accepted; scratch inference next — 2026-09-23

The active objective remains whole public PPML **<=0.90 times scientifically
comparable pinned stock fixest per case wherever achievable**. This checkpoint
completes a useful implementation milestone; it does not complete that objective.
It supersedes older implementation-order and unused-seed entries below.

Accept `lowjoint4`, based on `7d665c37`, under
[ADR 0134](docs/adr/0134-bounded-low-memory-joint-ppml-slopes.md) and its
[source-bound report](benchmarks/reports/development/ppml-low-joint-local-2026-09-23/README.md).
All **537 computational inputs** match plugin
`853bd995ff3783f7223e5e29c18b343aa5bd4d05da42ca9bc0452629809837f2`.
The immutable archive was restored and all **11,852 receipts** verified, SHA-256
`9ecddafb244db5f6835c10ff0d8a2f836c9883daad904274364ae168832873f1`.

Low PPML now uses the shared joint finite/FE Schur solver with bounded scratch-row
tiles and existing worker observation buffers. Finite width is independent of
RHS pool width; no new N-by-K bank. Current-weight norms/products are rebuilt,
and recurrence diagnostics never substitute for fresh FE, original-input finite
score or reconstruction acceptance. The admitted low owner also supplies the
existing strict Newton witness followed by one independent canonical final
rank/bread/Hessian construction. Tolerances, sample/weight/DoF/separation policies,
geometry authority and failure atomicity remain unchanged.

Final-source **353 distinct Rust tests**, actual-plugin ABI/panic checks and
**eight licensed runs of five fixtures** pass. Final public evidence includes
**153 passing applications**, **127 PPML states**, **46 native and 35 stock
pairs**, **15 independent Poisson/Hessian checks**, **13 exact OLS pairs** and
**11 exact excluded PPML pairs**. No public application failure, timeout,
scientific discrepancy or detected timing contention in those blocks.

### Current timings and remaining budget

Robust inference plus fitted means, three alternating rotations; times are ms.
Development seed333; fresh seed334. Low versus ordinary stock is not physically
memory-matched. Full ranges, every pair and intermediate losses are in the report.

| Case | Baseline | Candidate | Stock | Candidate/stock | Further reduction to 0.90*stock |
| --- | ---: | ---: | ---: | ---: | ---: |
| Low two slope partitions N30720 K6 T4 | 195 | 177 | 116 | 1.526 | 72.6 ms |
| Low unbalanced three-FE slopes N30720 K6 T4 | 188 | 180 | 121 | 1.488 | 71.1 ms |
| Low larger three-FE slopes N61440 K10 T8 | 474 | 415 | 226 | 1.836 | 211.6 ms |
| Fresh low two slope partitions N38400 K8 T1 | 355 | 340 | 473 | 0.719 | Target met here |
| Fresh low wider three-FE slopes N46080 K16 T8 | 565 | 475 | 1290 | 0.368 | Target met here |
| Fresh low multiple slopes N23040 K6 T4 | 173 | 157 | 132 | 1.189 | 38.2 ms |
| Fresh low weak barbell N7680 K4 T4 | 41 | 38 | 266 | 0.143 | Target met here |
| Fresh low disconnected slopes N15360 K6 T4 | 126 | 109 | 80 | 1.363 | 37.0 ms |

Fresh gains are **4.2–15.9%**, every native pair improves. The development middle
case retains one **4.8% paired loss**. Fast slope controls retain **129/131** and
**301/302 ms** native losses, versus stock120/227. Excluded low one-FE PPML's
initial **101/118** ties at **107/107** on five identical-input follow-up pairs;
its inference timing varies widely. Low categorical OLS's initial **63/64.5**
becomes **64/63** on five identical-input pairs. Other OLS slopes retain **67/68**.
Do not claim universal timing non-regression or transfer historical wins to this
source. Preserve every initial loss and follow-up distribution.

Tracked/modelled peak increments agree: **0.272/0.308/0.611 MB** on development
low cases and **0.065–0.513 MB** fresh. The larger low peak is
**57,443,213 tracked / 69,232,515 modeled bytes**. K16 exceeds default pool 10;
weighted oracle and allocation fixtures cover pool 1/2 and T1/T4. Physical-cap
confirmation remains deferred.

### Retained failures and attribution

The isolated frozen kernels were promising, but the first public per-column
adapter regressed the larger low fit **488 -> 511 ms**. Bounded row tiles alone
had only 1–4% median gains. Completing the shared strict/final path produced the
accepted result. Preserve both rejected intermediate organizations; do not resume
tile-width or benchmark-specific threshold tuning.

Each of two frozen prototypes has 480 passing runtime kernel attempts. Strict
independent WLS exports pass; ordinary 1e-7 exports fail final residual envelopes
and some coefficient envelopes, so they are not ranked as final fits against
strict stock. All public strict/final oracles pass. Constructor-clock overlap,
missing-fixture compile failure, partial-formatting test break and corrected
source identities are recorded explicitly. Do not use the partial-diff formatter
again; format complete changed functions or files and inspect the result.

Fixed-configuration production allocation audits remain exactly zero-event and
zero-growth. **The mixed-thread audit still fails** after retiring T4 workers:
zero events, **-4,376 bytes** growth, stable capacities and equivalent solves.
An isolated intrusive build attributes 2,072+2,304-byte frees to Crossbeam epoch
reclamation of Rayon queues. Baseline passed the same sequence. The exact-zero
criterion is unchanged; the new fixture selects one configuration per fresh
process. This runtime-accounting limitation is open, not repaired or hidden by
the successful fixed-process checks. Intrusive/traced allocation probes are not
headline timings or valid allocation witnesses.

### Next bounded milestones

1. **Scratch-backed inference and remaining low-mode transport.** The larger low
   fit still needs 211.6 ms; projection is 168.8 ms, strict/final 94.3 and
   inference 28.1, with some clocks nested. The current final score-design adapter
   issues positional reads per row, whereas retained finite solving already has
   an immutable scan cursor. Measure actual reads, consumer passes and ownership;
   extend bounded read traversal where coherent, preserving arithmetic, parallel
   ownership and independent Hessian/score checks. This should help low PPML
   broadly. Treat the mixed-thread audit limitation separately; do not mask it
   with sleeps or a relaxed growth criterion.
2. **Joint ordinary directions for sparse categorical PPML.** Compare a direct
   finite block with existing joint LSMR/categorical FE operators on identical
   frozen weights. Reuse immutable topology, rebuild weight-dependent numerical
   state, protect dense/weak/disconnected cases and retain canonical boundaries.
3. **Whole-command one-FE concentration.** Inventory loading, preparation, accepted
   means/predictors/group totals, strict/final products and publication. Remove
   duplicated construction with clear ownership; do not spend a cycle on the
   previously measured 3.9-ms inverse phase against much larger public deficits.
4. **Remaining fast slopes and uncovered breadth.** Measure setup, current-weight
   coupling/products and warm-start effectiveness. Extend mechanisms to wide,
   generated, weighted, small/no-FE and requested-output cases using fresh
   predeclarations. Previous one-FE/sparse stock budgets belong to their captured
   artifact and need matched current-source remeasurement.

Keep each milestone bounded: one computational comparison, a small public
baseline/candidate/stock screen, independent science, fresh confirmation and
excluded controls, then a coherent commit or documented rejection. Preserve
stock/source identities, uncontended measurements and distributions. Do not
accept a large unexplained deficit as an inherent certification cost.

Seed **202609334 is consumed**; **202609335** is unused/reserved. Frozen 77/16/111
inputs, static allocation drift, weighted constrained-fit/nonretaining-CG issues,
historical comparator discrepancies and SCC/platform deferrals remain unchanged.
No push, release, qualification, SCC retry or stopped-campaign restart.

## Current-frame scans accepted; ambitious per-case PPML plan — 2026-09-23

The active objective remains whole public PPML **<=0.90 times scientifically
comparable pinned stock fixest in each case wherever achievable**. This update
finishes the pending experiment and revises the implementation order from its
evidence. It does not declare that objective complete. Historical source-bound
claims and unresolved comparator discrepancies retain their original scope.

Accept `framemom1`, based on `07d41bef`, under
[ADR 0133](docs/adr/0133-current-frame-moments-and-ppml-scores.md) and its
[source-bound report](benchmarks/reports/development/ppml-current-frame-moments-local-2026-09-23/README.md).
All **536 computational inputs** match plugin
`12fd878cf359681240c23c43f46cdc3f541ff41c81bd980106510502db5c7509`.
The restored immutable archive verifies **7,367 receipts**, SHA-256
`c7964155884721759a83d75f5b11663254e73f5d0b78792d37b63feaf9e97bc2`.

Canonical retained assembly borrows already fresh raw means under checked
sample/topology/weight generations. Original-input outer numeric scores run
independent columns on the admitted team, preserving row arithmetic and ordered
reduction. Every required score check still runs; FE scores remain fresh.
No new observation bank, rank/tolerance/sample/weight/DoF change or weakened
certificate. Small/generated/wider score models retain the generic evaluator;
ordinary one-shot OLS retains its computation. Tracked/modelled peaks are unchanged.

Final-source **352 distinct Rust tests**, actual-plugin ABI/panic checks and
**six licensed fixtures** pass. Headline/pilot/control evidence has **163 passing
applications**, **117 PPML states**, **39 native and 39 stock pairs**, **nine
independent Poisson/Hessian checks**, and **23 exact OLS export pairs**. No
application failure, timeout, current scientific discrepancy or timing contention.
The initial failed compile is preserved. Real T1/T4 warmed allocation audits
remain zero-event/zero-growth. Small OLS one-FE retains a **17/18 ms** follow-up
loss; slope and generated-term controls tie on follow-up. No universal timing
non-regression claim. Full distributions and all initial losses remain in the report.

### Current budgets, not forecasts

Robust inference plus fitted means, three alternating rotations. The first two
rows use development seed332; the remaining rows use predeclared fresh seed333.
No median transfers to another revision or another design. Times are milliseconds.

| Case | fereg | Stock | 0.90*stock | Further native reduction |
| --- | ---: | ---: | ---: | ---: |
| Larger three-FE slopes N61440 K10 T8 | 300 | 232 | 208.8 | 30.4% |
| Dense categorical three FE N30720 K6 T4 | 193 | 232 | 208.8 | Already below target |
| One FE N61440 K10 T1 | 142 | 97 | 87.3 | 38.5% |
| One FE N76800 K8 T4 | 112 | 55 | 49.5 | 55.8% |
| Unbalanced one FE N122880 K6 T8 | 162 | 96 | 86.4 | 46.7% |
| Two slope partitions N23040 K6 T4 | 99 | 86 | 77.4 | 21.8% |
| Wider unbalanced three-FE slopes N46080 K10 T8 | 269 | 930 | 837.0 | Already below target |
| Sparse categorical N38400 K8 T4 | 133 | 84 | 75.6 | 43.2% |
| Low three-FE slopes N30720 K6 T4 | 194 | 117 | 105.3 | 45.7% |

Low versus ordinary stock is not physical-memory-matched. Development low one-FE
is still **98/24 ms**, an important diagnostic loss, not a constrained-memory
comparison. Fresh native gains are 5.9–12.9% in six main cells and a variable 1%
in low slopes, including a 5.1% paired loss. Preserve both existing wins above.

The isolated baseline diagnostic found only ~3.9 ms in each initial/strict
moments-factor-inverse phase on larger one FE; independent outer scores totalled
29.1 ms and each raw mean scan ~5.8 ms. Removing an initial inverse alone is not
the next redesign. Current larger one-FE phase medians include preparation 35.9,
publication 29.1, line search/profile 24.9 and strict/final 51.1 ms, versus 236 ms
public and a 121.5 ms target. Larger slope projection remains 109.0 ms and
strict/final 70.4 ms versus a 91.2 ms public deficit. These phases are partly
nested, not additive estimates of removable cost.

### Implementation order

1. **Make the successful joint solver work within low-memory ownership.**
   Start with matched fast/low frozen frames and count scratch reads, generated
   column evaluations, projections, normalized copies and finite products.
   The current ordinary joint path is fast-only because it owns a normalized
   N-by-RHS bank. Implement bounded tiles/streamed FE-finite cross-products and
   a matrix-free joint action over existing scratch; do not simply remove the
   guard or hide an N-by-K resident copy. Compare this organization with the
   existing affordable route before production integration. Keep current-weight
   geometry, rank, score/reconstruction and final-Hessian acceptance. Aim to
   remove a substantial part of the 45.7% deficit, with measured I/O and peak
   memory alongside time. This is the next bounded implementation milestone.
2. **Generalize joint ordinary directions to sparse/unbalanced categorical PPML.**
   Compare a direct finite block with the existing joint LSMR and categorical
   FE operators/preconditioners on identical frozen weights. Reuse immutable
   topology; rebuild only numerical weight-dependent state. Share projected
   control products and compatible response work. Choose one coherent ordinary
   direction engine with explicit admission and canonical initial/final duties.
   The 43% deficit calls for eliminating repeated work, not routing thresholds.
   Protect dense categorical wins and weak/disconnected graphs. Remove superseded
   machinery once a replacement is established.
3. **Complete one-FE concentration across the public command.** Analytic profiling,
   borrowed representatives and shared strict/final products are already accepted.
   Inventory every full-data pass from loading to publication; separate mandatory
   independent witnesses from repeated construction. Design a single accepted-
   state owner for means/predictors/group totals and final moment consumers,
   retaining independent original-input and Hessian certificates. Redesign initial
   and final interfaces where duplicated work dominates, including setup and
   output copying, instead of optimizing a 3.9-ms phase against a 62.5-ms deficit.
   Evaluate a bounded streamed one-FE route for low mode as part of this design.
4. **Close remaining ordinary slope losses.** In the existing coefficient Schur
   and joint owner, measure warm-start effectiveness, current-weight projected-
   control setup, coupling construction, Krylov work and canonical reconstruction.
   Compare coefficient warm starts/recycled preconditioning and shared streamed
   products as a small number of complete candidates. Prior solutions are guesses,
   never certificates for new weights. Target the 21.8–30.4% deficits on the
   comparable two-partition and larger three-FE cases. Do not restart the rejected
   residual-extrapolation probe or closed serial weighted-OLS tuning.
5. **Finish coverage and route selection.** Remeasure current unbalanced categorical,
   small/no-FE, wider and generated-term PPML before assigning them priorities.
   Extend successful mechanisms around imbalance, weak/disconnected topology,
   multiple slopes, rank deficiency, changing prior/IRLS weights, N/K, T1/T4/T8,
   robust/cluster inference and requested outputs. Use structure and admitted work
   to select routes, never case names or tuning seeds. Resolve scientific failures
   with independent evidence before ranking them. Preserve frozen 77/16/111 inputs.

Each milestone gets a frozen-frame computational comparison, one public decision
screen and predeclared fresh confirmation. Limit initial exploration to one or
two materially different designs; if both fail, record the cost evidence and move
on instead of tuning thresholds indefinitely. Accept smaller broad/cheap gains,
but do not call a loss closed while a large unexplained budget remains. For a
10% win claim, require the margin to survive fresh paired distributions and
relevant controls, extending repetitions when timer precision/spread matters.

Use unmodified pinned fixest, matched samples/work/threads/outputs and strict
reference tolerances. Diagnostic instrumentation stays separate. No competing
builds/profiles/fits during headline timings. Validate with independent explicit-
design WLS and Poisson/Hessian oracles; preserve rank, DoF, weight semantics,
separation, every final certificate and failure atomicity. Protect categorical
PPML, weighted/slope OLS, generated terms and saved outputs. Every adoption gets
its own reviewed decision, immutable evidence and coherent local commit. Remaining
losses require measured unavoidable/avoidable costs and a concrete next mechanism;
near-parity is not promised before evidence.

Seed **202609333 is consumed**; **202609334** is unused and reserved for future
predeclared confirmation. Static allocation drift, weighted constrained-fit and
nonretaining-CG failures, historical comparator discrepancies and platform/
physical-cap deferrals remain open. No push, release, qualification, SCC retry
or stopped-campaign restart. This section supersedes older next-step entries.

## Shared one-FE products accepted; whole-path construction next — 2026-09-23

The active goal remains whole public PPML **<= 0.90 times comparable pinned
stock fixest** wherever achievable. This is a completed bounded implementation
milestone, not completion of that goal. It supersedes the next-step and unused-seed
entries below. Historical artifact timings and discrepancies retain their scope.

Accept `onefeshare2`, based on `19ab5cab`, under
[ADR 0132](docs/adr/0132-shared-one-fe-ppml-products.md) and its
[source-bound report](benchmarks/reports/development/ppml-one-fe-shared-products-local-2026-09-23/README.md).
All **536 computational inputs** match plugin
`26cef3ff3c30a727c34437c81025cf2d104c2807c391e4d1f637f51dce687316`.
The restored immutable archive verifies **7,255 receipts**, SHA-256
`486a9aa90c3f38a16f18e1a94eb1533f098d647472c9565a1f8fd60ae3ce8d9f`.

Concentrated one-FE PPML borrows the retained FE-equivalent finite columns,
removing its selected N-by-K copy. Original-input scores and fresh canonical
checks remain independent. Once strict canonical WLS confirms the unchanged
accepted point, its current-weight within design, rank and inverse supply final
inference directly. Every weight bit/scale, current generation, q*mu identity
and predictor decomposition must agree; otherwise canonical final solving remains.
No tolerance, sample, inference, separation or transactional policy changes.
No extra state bank. Only the existing fast one-intercept-FE MAP-owned route changes.

Fresh seed332 public medians, baseline/candidate/strict stock (ms), robust plus mu:

| One-FE case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| N76800 K10 T4 | 159 | 139 | 83 | 1.675 |
| N61440 K10 T1 | 171 | 150 | 96 | 1.562 |
| N38400 K24 T4 | 149 | 134 | 100 | 1.340 |
| Unbalanced N76800 K6 T8 | 138 | 122 | 42 | 2.905 |
| Small N3840 K3 T1 | 22 | 22 | 11 | 2.000 |
| Larger N184320 K6 T8 | 311 | 270 | 126 | 2.143 |

Larger native gains are **10.1–13.2%**; all their individual pairs improve.
The small fit ties and has one 29.4% paired loss. Five identical-input follow-up
pairs give 23/19 ms with wide overlapping ranges; retain both blocks. Tracked
peak reductions equal 8*N*K, **0.092–8.847 MB** across fresh cases. Outer counts
stay unchanged; one fewer reported inner checkpoint reflects the removed solve.

**350 distinct final-source Rust tests**, actual-plugin ABI/panic checks and
**five licensed fixtures** pass. Across prototype and final blocks all **175
applications**, **129 PPML public states**, **49 native pairs**, **31 stock pairs**
and **12 small independent Poisson/Hessian checks** pass. Final artifact alone
has 142 applications, 96 states, 38 native pairs, 20 stock pairs and six independent
checks. All headline isolation checks pass. Real T1/T4 warmed allocation audits
remain zero-event/zero-growth; their raw-input cold witness is separately admitted.

Excluded PPML has **13 exact scientific-export pairs**; OLS has **23**. Dense
categorical follow-up retains a 2-ms loss (201/203) with overlapping ranges.
One-FE OLS's initial 17/18 becomes 18/17 on five follow-up pairs. Low three-FE OLS
has an initial 64.5/65.5, then 64/68 with within-block candidate decline, then
66/65. Across all **12 identical-input pairs**, medians are **65.5/65** with ranges
[63,69]/[63,70]. Preserve every block; no universal non-regression claim.

Fresh N76800/K10 strict/final time falls **44.234 -> 27.018 ms**. The larger fit
falls **100.828 -> 61.319 ms**, with preparation **40.305**, publication **29.231**
and line search/profile work **38.303 ms**. It still needs **156.6 ms** to reach
0.90*126 = 113.4 ms. Eliminating final checks entirely would be insufficient.
Do not continue polishing the accepted reuse predicate or small-fit noise.

**Next bounded implementation:** initial rank/fit and shared accepted-state
construction across the whole PPML path. Investigate avoiding unused initial
inference products while preserving the exact canonical initial rank decision;
measure repeated original-column scans and predictor/mean/moment construction.
Retain fresh original-input, current-weight and final-Hessian witnesses. Confirm
with one-FE, categorical multi-FE, slope and low-memory controls. Historical
single-frame slope gains remain accepted; do not reopen closed serial weighted-
slope tuning. No generic claim that stronger certificates explain the remaining gap.

Seed **202609332 is consumed**; **202609333** is reserved and unused, with fresh
case declarations still required. Static allocation drift, constrained-fit/CG
failures, previous comparator discrepancies and platform/physical-cap deferrals
remain open. Frozen 77/16/111 definitions are unchanged. No push, release, SCC
retry, qualification or stopped-campaign restart.

## Single-frame slope finalization accepted; one-FE whole-path redesign next — 2026-09-23

The active objective remains whole public PPML time <= **0.90 times comparable
pinned stock fixest** wherever achievable. This is a bounded implementation
milestone, not completion. It supersedes the pending candidate and unused-seed
entries below; the broader per-case plan remains active.

Accept `jointfinal2`, based on `00e9b30c`, under
[ADR 0131](docs/adr/0131-single-frame-ppml-slope-finalization.md) and its
[source-bound report](benchmarks/reports/development/ppml-single-frame-finalization-local-2026-09-23/README.md).
All **536 computational inputs** match plugin
`96bea000c5ca47fa0176ae37302b793fec13470efbcbbf657b7446e7cf99506b`.
The restored immutable container verifies **9,799 receipts**, SHA-256
`5fa293ccf608f3ecedab1c0cbb7410c66fbee617d681342df96957c5e7527dce`.

Eligible fast slope finalization now uses a strict joint Newton witness followed
by one canonical rank/within-design/bread/Hessian construction at the unchanged
accepted weights. Fresh projection, finite score, model-space, objective,
reconstruction and final-Hessian conditions remain mandatory. Exact owned weight
bits/scales and sample/topology generations govern geometry relabeling; revoked
authority cannot revive. A declined witness or comparison retains canonical retry.
There is no additional heap bank. Ordinary joint solving is unchanged; the
shortcut excludes low, categorical, one-FE and other solver families.

Development seed330 baseline/candidate/strict-stock medians (ms): two slope
partitions N30720/K6/T4 **149/139/116**; unbalanced three FE N30720/K6/T4
**155/150/116**; larger N61440/K10/T8 **337/321/230**. Fresh seed331:

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Two slope partitions N23040 K8 T1 | 159 | 149 | 294 | Unranked |
| Unbalanced three FE N38400 K6 T4 | 195 | 184 | 154 | 1.195 |
| Wider unbalanced three FE N46080 K12 T8 | 283 | 273 | 960 | 0.284 |
| Multiple slopes N7680 K6 T4 | 53 | 51 | 50 | 1.020 |
| Weak barbell N15360 K6 T4 | 69 | 64 | 1146 | 0.056 |
| Disconnected N15360 K6 T4 | 92 | 88 | 70 | 1.257 |

Every main paired native change improves; fresh median gains are **3.5–7.2%**.
The first fresh cell's three stock pairs fail one fitted-mean coordinate:
maximum mean error 6.437e-7, maximum tolerance ratio 1.027866. Native pairs and
all independent public states pass, but this does not identify which large
returned mean is responsible. Keep stock unranked. Small passing pilots and
previous seeds do not resolve this discrepancy or historical reference failures.

**53 distinct Rust checks**, actual-plugin ABI/panic checks, **five licensed
fixtures**, and all **15 small independent Poisson/Hessian oracle checks** pass.
All **195 public applications**, **151 PPML public states** and **52 native PPML
pairs** pass; 44 of 47 stock pairs pass, with the three failures above preserved.
No headline overlap/monitor error is recorded. One existing environment-dependent
Rust harness test remains ignored. Failed build/controller/diagnostic setup
attempts are retained separately. The actual T1/T4 warmed allocation audit stays
zero-event/zero-growth. Median tracked and modeled peaks are unchanged; larger
development peak **58,342,238 bytes**, modeled **65,262,268 bytes**.

Excluded controls have **20 PPML and 22 OLS exact scientific-export pairs**.
Initial small timing losses received five identical-input pairs: dense categorical
PPML **191/191 ms**, slope OLS **69/66**, low categorical OLS **64/64**, with
overlapping ranges. Preserve the original losses too; no excluded speed claim.
Current candidate/stock excluded medians remain low slopes **221/113**, sparse
categorical **234/136**, unbalanced categorical **181/113**, dense categorical
**193/232**, and one FE **128/52**. Low versus ordinary stock is not memory-matched.

The large native strict/final phase falls **86.945 -> 72.783 ms**; outer iterations
remain six. Isolated diagnostic same-input calls replace **43.483 + 34.953 ms**
canonical transactions with a **20.704 ms** strict joint witness and **44.486 ms**
canonical final transaction. Final reprojection costs **23.369 ms**, norm refresh
**3.857**, and finite/rank assembly **16.704**, including **10.459 ms** reprojecting
the nearly zero residual. Ordinary joint calls take 18.2–20.8 ms; fresh single-RHS
FE certificates take about 0.8–1.1 ms. These diagnostic builds are not headline
comparators. The 321 ms large fit still needs **114 ms** to reach the 207 ms target.

**Next bounded implementation:** one-FE whole-path concentration and final product
ownership. Existing analytic concentration already converges in three outer
iterations in the captured one-FE diagnostic. Initial retained construction is
18.4 ms, strict transactions 15.6 + 11.3 ms, and the complete strict/final phase
37.3 ms. The current public 128/52 ms comparison needs an **81.2 ms** reduction to
46.8 ms; ordinary-step polishing or a small copy alone cannot deliver it. Design
shared accepted-weight centered-design/moment and canonical final products with
independent rank/bread/reconstruction checks; remove the second selected-design
copy through explicit ownership. Include initial and public/outer passes in the
budget. Refresh weight-dependent moments on the exact accepted frame. Do not
continue threshold tuning this accepted slope shortcut or revive rejected probes.

Seeds **202609330/331 are consumed**; **202609332** is reserved and unused for
the next fresh confirmation, with case declarations still required. Known static
allocation drift, constrained-fit/CG failures, comparator discrepancies and
deferred platform/physical-cap qualification remain open. Frozen 77/16/111
definitions are unchanged. No push, release, SCC retry or stopped campaign restart.

## Updated PPML plan: close per-case runtime budgets — 2026-09-23

The target is whole public `feppml` time **<= 0.90 times scientifically comparable
pinned stock fixest in each case wherever achievable**. It is not an aggregate
win-rate target. This planning update preserves the pending implementation below;
it introduces no new accepted runtime change or speed claim. Accepted production
remains `4527694e` / `jointprod2` and its source-bound report below.

Current accepted-artifact budgets, robust inference plus fitted means, in ms:

| Case | fereg | Stock | Target | Further fereg reduction |
| --- | ---: | ---: | ---: | ---: |
| Two slope partitions N30720 K6 T4, development seed329 | 147 | 118 | 106.2 | 27.8% |
| Unbalanced three-FE slopes N30720 K6 T4, development seed329 | 153 | 118 | 106.2 | 30.6% |
| Larger three-FE slopes N61440 K10 T8, development seed329 | 335 | 227 | 204.3 | 39.0% |
| One categorical FE N61440 K10 T4, seed330 | 128 | 51 | 45.9 | 64.1% |
| Sparse categorical two FE N61440 K10 T4, seed330 | 245 | 136 | 122.4 | 50.0% |
| Unbalanced categorical three FE N30720 K10 T4, seed330 | 178 | 115 | 103.5 | 41.9% |

Low slopes N30720/K6/T4 are 222 versus 118 ms, implying a 106.2 ms aspirational
target and 52.2% reduction; native low versus ordinary stock is not memory-matched.
Several fresh slope cases and dense categorical already clear 0.90. Preserve
their exact-source evidence and protect those wins. The multiple-slope 0.900
rounded median has no comfortable timing margin. These budgets are arithmetic,
not forecasts or claims covering an entire design family.

1. **Complete the strict/final redesign, then account for the remaining slope
   gap.** Test the pending tight joint Newton witness followed by one canonical
   rank/bread/Hessian construction on the unchanged accepted weight frame.
   Only bit-identical weights and scales may transfer weighted geometry authority;
   a changed basis, failed witness or movement retains canonical retry. Independently
   verify all final conditions. The isolated accepted-source diagnostic found
   about 43 ms strict plus 35 ms final work on the larger case; that is an envelope,
   not all removable time. Even eliminating the accepted report's entire 87 ms
   strict/final phase would not cover its 131 ms public deficit. After this decision,
   examine initial construction and the remaining joint projected-control setup,
   normalization, current-weight products and reconstruction. Pursue shared row
   passes and certified coefficient warm starts within the existing Schur owner;
   retain a new organization only if complete public work improves. Do not restart
   the rejected response-history or residual-extrapolation probes.
2. **Finish exact one-FE concentration as an end-to-end engine.** Analytic group
   elimination already exists; adding it again is not the task. Remove its second
   selected finite-design copy through borrowing or bounded row access. Reuse
   exact-state centered moments and final products only where they can discharge
   the canonical rank, inverse and reconstruction duties independently. Count
   initial, ordinary and final passes explicitly. If redundant canonical work
   dominates, redesign its interfaces instead of polishing ordinary Newton steps.
   Preserve initial rank/status policy and retain numerical fallback. The 64%
   required reduction makes a whole-path change necessary.
3. **Close sparse and unbalanced categorical losses.** Compare the existing joint
   LSMR route with a direct finite block coupled to reusable categorical FE
   operators/preconditioners. Reuse topology across IRLS, refresh numerical
   weights, and share compatible RHS/product work. Compare on identical frozen
   frames first; do not assume slope routing guards can simply be removed. Choose
   one successful organization using structural work, dimensions and admitted
   memory. Protect dense categorical wins and weak/disconnected graph behavior;
   remove superseded ordinary machinery once replacement is established.
4. **Make low memory an efficient solver design.** Adapt successful joint or
   concentrated computation to bounded row tiles and streamed finite products,
   avoiding repeated scratch decoding and unnecessary full-design rewrites. Count
   I/O bytes and passes as well as arithmetic. No hidden resident N-by-K copy.
   Evaluate fast/low native equivalence and report actual/modeled peaks separately
   from ordinary-stock timing. Refusal must retain an affordable certified route
   or fail atomically. Physical-cap qualification remains deferred.
5. **Remove remaining public-command overhead where measurements justify it.**
   Once large solve costs fall, measure loading, generated/factor terms, worker
   setup, inference and saved-output reconstruction, including small fits and
   wider controls. Share compatible final row scans and owned data; preserve fresh
   scores and main-thread SPI. Inference is only 1.3 ms in the larger accepted
   slope case, so it cannot be the first answer to that case's 131 ms deficit.
6. **Confirm breadth and close each loss explicitly.** Use predeclared fresh
   cases around any successful mechanism: imbalance, weak/disconnected topology,
   multiple slopes, rank deficiency, changing prior/IRLS weights, N/K, T1/T4/T8,
   robust/clustered VCE, requested outputs and memory limits. Keep no-FE, generated
   terms and higher-dimensional cases in the adaptive coverage inventory rather
   than claiming untested families. Protect one/two/three-FE and slope OLS,
   categorical PPML, separation and failure atomicity with focused controls.

Each candidate has three decision stages: frozen computational comparison,
alternating public baseline/candidate/stock screen, then fresh confirmation.
Extend repetitions when spread or timer precision prevents a 10% conclusion.
Compare matched work against unmodified pinned fixest with `glm.tol=1e-10` and
`fixef.tol=1e-11`; instrumented reference builds remain diagnostic. No compilation,
profiling, archiving or competing fits accompany headline timings. Independent
explicit-design WLS and Poisson/Hessian oracles precede ranking; ordinary fixest
weights never serve as an FW inference oracle. Report distributions, paired
changes, failures, scientific discrepancies and memory per case. Preserve all
samples, tolerances, rank/DoF/weight/separation policies and final certificates.

Accept smaller broad, cheap gains when reliable, but do not call a subsystem done
while a large budget remains. Reject inconclusive complex candidates, record why,
and choose a materially different design. Each adopted milestone receives its
own ADR where required, immutable report and coherent local commit. Any remaining
loss must end with measured required versus avoidable costs and a concrete next
mechanism. No claim of universal parity, release qualification or Linux results.
Frozen 77/16/111 definitions and known deferred failures remain unchanged.

**Pending experiment preserved:** three modified Rust files implement the
strict/final candidate; no production adoption or public candidate timing yet.
Current focused checks pass: 37 prepared-IRLS and 10 FE-MAP tests; one existing
FE-MAP harness test remains ignored. The analytic test includes T1/T4, disabled
candidate, forced predictor movement and forced basis-change retry. Local
`target/ppml-final-construction-20260923/pending-plan-checkpoint.json` binds source,
patch and test-log hashes; its adjacent patch preserves this exact working state.
Diagnostic `finalgeomdiag2` is accepted-source instrumentation, not the candidate.
Seed **202609331** remains predeclared and unused for candidate fresh confirmation.
Resume by reviewing/formatting this candidate, building an immutable artifact,
running focused licensed/oracle boundaries, then the public decision screen.

## Joint finite/FE PPML directions accepted; exact-state final work next — 2026-09-23

The active objective remains whole public PPML time <= **0.90 times comparable
pinned stock fixest** wherever reasonably achievable. This checkpoint completes
a production solver milestone, not that broader objective. It supersedes the
pending integration and unused-seed entries below.

Accept `jointprod2` under [ADR 0130](docs/adr/0130-joint-ppml-slope-directions.md)
and its [source-bound report](benchmarks/reports/development/ppml-joint-slope-production-local-2026-09-23/README.md).
All **536 computational inputs** match plugin
`5ca6e1a81a548aef3c733146f04929b68da849493c35574a0361973a94aa8730`,
based on `b2b59f62`. The restored immutable container verifies **6,723 receipts**,
SHA-256 `9bc10851c2710c2e2cbe152d33c584ac74c8fd1925f398d6d930f926b36a6ddf`.

Fast ordinary multipartition slope PPML now solves one joint finite/FE direction
using the existing rank-aware QR, FE Schur operator and optional coupling. It
borrows retained FE-equivalent controls and reuses the normalized RHS bank;
there is no additional N-by-K design. The distinct ordinary result cannot supply
bread or within-inverse authority. Current-weight FE/raw finite-score and
reconstruction checks remain fresh; canonical initial rank, strict transactions,
model-space/objective and final-Hessian construction remain independent. The
ADR records the replacement of unused ordinary inverse construction. Low,
categorical, one-FE, explicit non-MAP, separation and public OLS routes are unchanged.

Development seed329 baseline/candidate/strict-stock medians (ms): two slope
partitions N30720/K6/T4 **163/147/118**; unbalanced three FE N30720/K6/T4
**166/153/118**; wider N61440/K10/T8 **372/335/227**. All still lose to stock.
Fresh seed330, three rotations, robust inference and fitted means:

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Two slope partitions N23040 K8 T1 | 176 | 163 | 290 | 0.562 |
| Unbalanced three FE N38400 K6 T4 | 211 | 198 | 154 | 1.286 |
| Wider unbalanced three FE N46080 K12 T8 | 354 | 306 | 958 | 0.319 |
| Multiple slopes N7680 K6 T4 | 60 | 54 | 60 | 0.900 |
| Weak barbell N15360 K6 T4 | 76 | 68 | 1263 | 0.054 |
| Disconnected N15360 K6 T4 | 99 | 91 | 76 | 1.197 |

Fresh gains are **6.2–13.6%**, all paired native changes improve. The report
includes ranges and every pair; the 0.900 multiple-slope margin is limited by
millisecond timer precision. Passing current cases do not resolve historical
reference failures at other inputs. Default-reference failures stay separate.

**52 distinct Rust checks**, actual-plugin ABI/panic checks and **five final
licensed fixtures** pass, including the independent unit/FW/PW robust/cluster
Poisson/Hessian oracle and a T1/T4 actual-allocation audit. The warmed 16-iteration
window has zero events/growth and passes fresh/canonical equivalence. Five small
explicit Poisson/Hessian pilots pass all 15 native/stock oracle comparisons.
All **175 public applications**, **141 PPML public-state checks** and **64 native
scientific pairs** pass, with no detected overlap or monitor error. The initial
foreign-Stata preflight refusal and repaired test-development failures are retained.
One unrelated frozen-frame Rust test was ignored by its existing harness.

Excluded controls remain healthy: 15 PPML and 17 OLS native scientific-export
pairs are exact. The initial OLS slope 68/70 ms loss was checked with five more
identical-input pairs: **73/73 ms**, overlapping ranges. Do not claim an OLS speed
improvement. Current excluded candidate/stock medians remain **222/118 ms** for
low slopes, **245/136** sparse categorical, **178/115** unbalanced categorical,
**203/242** dense categorical, and **128/51** one FE. These are important remaining
losses; low native versus ordinary stock is not matched on memory.

The joint owner adds **0.069–0.611 MB** to tracked local peaks across the measured
main cases. The larger development peak is 58,337,590 bytes versus 57,726,518.
Optional refusal preserves the old solver; all joint state is released before
inference. Static allocation inventory drift and physical-cap gates remain open.

**Next bounded implementation:** shared exact-state strict/final construction.
On the larger development case, projection falls **153.760 -> 109.043 ms**,
while strict/final rises **74.261 -> 87.053 ms**; outer iterations stay six,
fallbacks zero. Controls retained after a joint direction are valid representatives,
not newly converged current-weight within columns. Their later canonical refresh
is real work. A 227 ms stock time implies a 204.3 ms target versus 335 ms now,
so another **130.7 ms** is needed; 1.3 ms inference cannot close that gap.
Inspect exact accepted-frame ownership and duplicate projection/rank/factor/
Hessian construction, then implement coherent reuse/transfer only where identities
and independent acceptance justify it. Preserve current-weight final certificates;
do not restart ADR 0128's rejected response-history probes or tune joint thresholds.
Then address deliberate low-memory solving, one-FE concentration and categorical
coverage under the broader plan below. OLS needs a canonical within design/bread;
this direction-only win does not itself justify a one-shot OLS adapter.

Seeds **202609329 and 202609330 are consumed**. Seed **202609331** is predeclared
and unused for the next materially new candidate. Work remains local on main;
no push/release, SCC retry, stopped campaign or qualification run. Constrained-fit/
generic CG failures, historical comparator discrepancies and deferred platform
qualification remain unresolved.

## Direct joint slope candidate selected; production integration next — 2026-09-23

The active objective remains public PPML runtime <= **0.90 times comparable
pinned stock fixest** wherever reasonably achievable. This milestone establishes
a computational candidate, not production adoption or completion. It supersedes
the pending-experiment and unused-seed entries below; the implementation plan
otherwise remains active.

The [frozen comparison report](benchmarks/reports/development/ppml-joint-slope-frozen-local-2026-09-23/README.md)
preserves two implemented isolated candidates and actual changing-weight fused
response/control frames. Reject generic global-partition augmentation: its global
QR/recovery and loss of RHS parallelism make the wider ordinary T8 proxy
54.568 versus 21.954 ms. Select the direct finite block coupled to the existing
FE Schur operator for a production ordinary-iteration experiment. Its paired
proxy medians improve 9.269 to 6.689 ms on unbalanced K6/T4 and 23.018 to
15.967 ms on wider K10/T8; wider T1 improves 71.178 to 27.929 ms. Small and
tight two-partition threaded losses remain. No threshold has been fitted to them.

All strict candidate, baseline and stock outputs pass independent explicit
weighted-design oracles on four frozen frames. Direct maximum coefficient and
residual discrepancies are 3.194e-14 and 1.464e-11. Loose ordinary solves do not
meet final-publication envelopes, reinforcing the required strict acceptance.
Five accepted-baseline licensed captures and eight frozen test invocations pass.
These are not candidate public Poisson/Hessian or OLS integration tests.

The accepted native inner solver already beats stock on these warm frozen
problems. That limited evidence does not rank whole IRLS and does not remove
the final-state/inference work from the plan. Preserve the corrected small-case
generator, independent SVD nullspace and standalone stock-thread-controller
attempts; the existing public benchmark harness's thread settings are unaffected.

The immutable archive restores **2,323 receipts**, SHA-256
`9a29de4aded72296438cc7ad8213504b07577717db5449912db950d9652634c3`.
All **534 production computational inputs** still match accepted `scrread1` at
`7bf3c402`. Both prototypes remain isolated under
`target/ppml-joint-slopes-20260923`, with exact source also sealed in the report.
No new production runtime or memory claim is established.

**Immediate implementation:** integrate a direct finite-block ordinary PPML
direction with the existing slope geometry/Schur owner. Reuse its normalized
column bank and borrow retained controls where valid, rather than adding another
N-by-K design or FE-owner copy. The prototype's dense Z bank is not low-memory
admission evidence. Add bounded fallible storage, original-input finite/FE score
checks, reconstruction, frame/basis invalidation, fallback and allocation-audit
coverage. Preserve initial canonical rank and strict/final construction. Measure
complete public baseline/candidate/stock rotations and excluded OLS/categorical
controls before adoption; the narrow threaded margin may disappear in public
costs. Broader exact-state final construction and inference remain next.

Seed **202609329 is consumed**; fresh **202609330** is predeclared but ungenerated.
Controllers ran sequentially without assistant-launched builds, profiles, archives
or competing fits; these diagnostic kernels did not attach a whole-machine
contention monitor. Static allocation drift, constrained-fit/CG failures,
reference discrepancies and deferred platform gates remain open. No push,
release, SCC retry or stopped campaign restart.

## Bounded low-memory reads accepted; joint finite-plus-FE solving is next — 2026-09-23

The active objective remains public PPML at most **0.90 times scientifically
comparable pinned stock fixest** wherever reasonably achievable, protecting OLS
and all scientific/memory/transaction contracts. This milestone does not complete
that objective. This opening supersedes historical next-step/unused-seed entries.

Accept `scrread1`, based on `73c73dfc`, under [ADR 0129](docs/adr/0129-retained-scratch-read-cursors.md)
and the [source-bound report](benchmarks/reports/development/ppml-scratch-reader-local-2026-09-23/README.md).
All **534 computational inputs** match plugin
`e6d3bd7035556bb77d501b4aa5de2a030002af10068c32b97ce0913e1ba46c4f`.
The restored immutable container verifies **6,564 receipts**, SHA-256
`00e2f4779eebc0723c2c10e1f8e74262794cb3305d587779d4aff2efdb561b75`.

Retained finite QR/span and fresh/frozen residual reconstruction reuse one
immutable 8 KiB encoded page across scratch rows. Existing moment tiles, numeric
operations, rank, certificates and policies are unchanged. The borrow prevents
mutation while bytes are live; failed page loads revoke authority. No extra heap
or resident design bank. Stack lifetime is longer under existing headroom, with
no claim of physical-cap qualification.

Fresh seed328 low-memory baseline/candidate/strict-stock medians (ms):

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Unbalanced slopes N23040 K6 T4 | 251 | 162 | 94 | 1.72 |
| Wider slopes N46080 K12 T8 | 794 | 593 | 942 | 0.63 |
| One FE N61440 K16 T4 | 467 | 325 | 110 | 2.95 |
| Sparse categorical two FE N30720 K10 T1 | 477 | 359 | 193 | 1.86 |
| Unbalanced categorical three FE N23040 K8 T4 | 330 | 242 | 90 | 2.69 |
| Multiple slopes N7680 K6 T4 | 100 | 64 | 54 | 1.19* |

Native gains span **24.7–36.0%** with identical tracked peaks. The wider cell
clears the target, most still lose. Low native versus ordinary stock is not a
matched-memory comparison. *A smaller independent multiple-slope stock pilot
fails two covariance coordinates (maximum tolerance ratio 1.60028); disconnected
stock fails convergence. Preserve both unranked; passing large pairs do not
resolve those families or older discrepancies.

**202 distinct Rust checks**, actual-plugin ABI/panic checks and **four licensed
fixtures** pass, including a captured independent low-memory unit/FW/PW robust/
cluster Poisson/Hessian oracle. All **69 native pairs** retain exact scientific
exports. There are 184 public applications: 183 pass, one stock nonconvergence;
no overlap/monitor errors. The dense categorical excluded loss 188/192 ms gets
five identical-input pairs, 194/195 ms with overlapping ranges. Preserve that
small residual loss. OLS controls all pass. An early formatting-cleanup compile
failure was repaired before artifact creation and is preserved in the receipts.

### Ambitious PPML implementation plan

The target is **whole public command time <= 0.90 times comparable stock fixest
per case**, not a 10% improvement over the previous fereg build or an aggregate
win rate. The current fresh low-memory losses need approximately 48% (unbalanced
slopes), 70% (one FE), 52% (sparse two FE), and 67% (unbalanced categorical three
FE) further runtime reductions to reach that target. These are arithmetic budgets
from the table above, not predictions. The wider slope cell already clears it.
Low native versus ordinary stock remains explicitly unmatched on memory.

1. **Bind the remaining work to current source and cost budgets.** Keep the
   existing current-source screen; remeasure older fast-path losses on the next
   candidate/baseline/stock rotation rather than transferring older claims.
   Start with two slope partitions, unbalanced three FE slopes, wider slopes,
   one FE, sparse categorical two FE and unbalanced categorical three FE. Keep
   dense categorical three FE as a control. Measure setup/loading, weighted
   geometry, ordinary solving, strict/final construction, inference and output
   separately, with counts of RHSs, passes, reconstruction and fresh checks.
   Diagnostic instrumentation stays outside stock headline timings. Explicitly
   rank comparable fast work separately from low-memory execution and preserve
   scientific failures as unranked.
2. **Replace K+1 ordinary slope projections with one joint Newton solve where
   profitable.** Projection still costs 312.32 ms in the fresh wider low case.
   Compare one augmented finite/FE coefficient Schur solve with a shared-pass
   batched Schur alternative on identical frozen current-weight problems.
   Reuse accepted local rank-aware geometry and symbolic topology; do not merely
   lift intercept-only categorical guards. Include geometry construction,
   finite coefficient recovery, certificates and all workspace in the comparison.
   If the global finite QR or recovery erases savings, use a direct finite block
   sharing the FE operator instead of tuning thresholds. Retain one successful
   organization. Add current-weight-certified warm starts and a bounded-memory
   adapter deliberately; no hidden resident N-by-K bank. Evaluate an OLS adapter
   only after the PPML kernel succeeds and its one-shot costs are measured.
3. **Make strict/final construction a shared exact-state operation.** Build
   each necessary weighted geometry, within design, finite factor and Hessian
   once per valid state, with explicit sample/design/weight/basis identities and
   revocation on mutation or failure. Investigate whether a single canonical
   construction can serve final rank, finite solve, bread and inference without
   rebuilding it at each boundary. Cheap diagnostics may schedule expensive
   checks; independently fresh residual, model-space, score, objective,
   reconstruction and final-Hessian acceptance remain mandatory. Document any
   revised checking procedure in a successor ADR with adversarial stale-state
   tests. Do not resume ADR 0128's rejected response-history tuning.
4. **Finish one-FE concentration and categorical solver coverage.** One-FE low
   strict-final and inference alone still cost 85.67 and 54.74 ms in their native
   phase medians. Remove the concentrated engine's extra finite-design copy
   through deliberate ownership/streaming, then extend its useful computation
   to low memory. Reuse centered group moments for Newton/Hessian construction
   where valid. For sparse/unbalanced categorical cases, compare the admitted
   joint operator, preconditioning and topology reuse against existing projection
   using measured structural work. Protect dense categorical wins. Routing must
   follow actual topology, dimensions, workspace and convergence evidence, never
   case names or seeds. Do not accumulate overlapping speculative engines.
5. **Reduce inference, saved-output and public-command passes.** Carry bounded
   scratch readers through covariance scans; share compatible row decoding,
   weighted score preparation and final products without reusing stale scores
   or weakening independent acceptance. Include robust and clustered VCE and
   requested fitted/saved effects in end-to-end comparisons. After the large
   solve costs fall, measure loading, factor/generated-term preparation, worker
   setup and publication on small fits; remove repeated ownership/copying work
   within the Stata/Rust/main-thread SPI and transactional boundaries.
6. **Close every measured loss with evidence.** Each milestone gets focused
   independent weighted explicit-design/Poisson/Hessian oracles, baseline/candidate/
   stock rotations, then fresh predeclared confirmation. Adapt coverage around
   imbalance, weak/disconnected topology, multiple slopes, rank deficiency,
   changing weights, regressor width, threads, memory, inference and outputs.
   Keep OLS one/two/three-FE, slopes, generated terms and fast/low controls, plus
   categorical PPML, separation and failure-atomicity checks proportionate to
   the changed boundary. Report medians, ranges, paired changes, failures and
   measured memory. Confirm noisy or small regressions before accepting them.
   Commit coherent successful replacements and retire superseded machinery.

**Bounded decision rule:** each coherent candidate gets a frozen-kernel decision,
a public development screen and, if promising, fresh confirmation. Small reliable
gains may be retained when cheap and broad, but they do not complete a subsystem
whose remaining loss needs a multiple. Reject inconclusive high-complexity probes
and move to a materially different organization. If a target remains unmet,
provide its measured residual phase budget and the precise outstanding mechanism;
do not replace evidence with a generic request for more profiling. This plan is
development work, not authorization for full qualification or a claim of parity.

**Pending isolated experiment:** `target/ppml-joint-slopes-20260923` contains the
frozen augmented-Schur prototype and declarations. Its current-source diagnostic
capture plugin `jointfreeze1` built successfully, SHA-256
`4799fde7d15f0391c57796a549d75f1e6e17add662a641ee793bf0b3a6ec7edf`;
the build receipt is `build-jointfreeze1.log` in that directory. The prototype
has not yet been compiled or measured, and captures have not run. The later
capture declaration correctly requires an actual fused response/control frame;
older control-only frames must not stand in for a Newton response. No production
source changes or new performance claims are associated with this preparation.

Seeds **202609327/328 are consumed**. The pending experiment declares
**202609329** for development and **202609330** for fresh confirmation; neither
has been generated yet. Static allocation drift, constrained-fit/
CG failures, obsolete receipt assertion, reference discrepancies and deferred
Linux/SCC/physical caps remain open. Frozen 77/16/111 definitions are unchanged.
No push, release, SCC authentication retry or stopped campaign restart occurred.

## Final-state probes rejected; proceed to substantial low-memory and joint solving — 2026-09-23

The active objective remains public PPML at most **0.90 times scientifically
comparable pinned stock fixest** wherever reasonably achievable, while protecting
OLS and all scientific/memory/transaction contracts. This opening supersedes
historical next-step and unused-seed entries below.

Reject `finalstate1` and `finalstate2` under [ADR 0128](docs/adr/0128-final-state-response-probes.md).
The [source-bound report](benchmarks/reports/development/ppml-final-state-probes-local-2026-09-23/README.md)
preserves all **168 passing public applications**, both timing distributions,
132/134 Rust checks, four passing licensed fixtures per candidate and exact source
identities. Restored container verifies **6,173 receipts**, SHA-256
`7294400c7cc4ea7eca78d524a5909cd9ef63c2480193496595e934c2a12fd467`.
Production is restored to `4ad7ade5`: all **534 computational inputs** match
accepted `oneprof6`. The broader point-retention/response-history state machine
did not justify its complexity: second-screen gains are small/mixed, wide slopes
lose slightly, and low memory is unchanged. Do not tune this probe further.

Next implement bounded retained-scratch readers for QR and reconstruction,
then inference where measured. **Finite moments already batch complete rows
under ADR 0070**; do not repeat that work. Measure syscall counts, finite/strict
phases, public time and admitted/actual memory. No hidden resident N-by-K bank.
The principal algorithmic target remains joint finite-plus-slope Newton solving
to avoid K+1 ordinary projections. Compare that coherent candidate against
shared-pass batched Schur, then retain one successful organization. Subsequent
shared exact-state factor construction, one-FE ownership and categorical/shared
passes must address their full remaining cost budgets, with fresh independent
acceptance and OLS/PPML controls at each accepted milestone.

Seed **202609325 is consumed**; 326 was declared but not generated. Use
**202609327** for the next development design and predeclare fresh confirmation
separately. No broad objective, unresolved science/memory issue or deferred
platform gate is closed by this rejection checkpoint.

## Concentrated one-FE PPML accepted; broad target remains active — 2026-09-23

The active objective remains public PPML runtime at most **0.90 times scientifically
comparable pinned stock fixest** wherever reasonably achievable, while protecting
OLS and scientific/transaction/memory contracts. This checkpoint completes the
one-FE experiment; it does not complete that broader objective. This opening and
the next sequence below supersede historical next-step and unused-seed entries.

Accept `oneprof6`, based on `386b2170`, under
[ADR 0127](docs/adr/0127-profiled-one-fe-poisson.md) and the
[source-bound report](benchmarks/reports/development/ppml-one-fe-profile-local-2026-09-23/README.md).
All **534 computational inputs** match plugin
`c1aa681c6c160d60381d8483b783a1bbed057c63d4f0d81c89f9e02485ac67b3`.
The restored immutable container verifies **11,494 logical files**, SHA-256
`c9b6d302f392429be442c67b5eb115884d510469343f1e6020a9c2d62a85b691`.

After the canonical initial rank solve, analytically optimize categorical group
intercepts for every finite-beta trial and use a centered finite Newton system.
Keep independent outer scores, two convergence passes, strict canonical WLS and
fresh final predictor-response/Hessian acceptance. Retaining a strict-confirmed
accepted point preserves bitwise current weights and enables existing same-weight
control reuse. No tolerances, sample, rank, weights, inference, DoF, separation
or failure atomicity were weakened. Scope is optional fast, actual one-intercept-FE,
MAP-owned publication; other paths retain their existing execution.

Fresh seed324 medians, baseline/candidate/strict-stock ms:

| One-FE case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| N61440 K10 T4 | 147 | 129 | 53 | 2.43 |
| N61440 K10 T1 | 194 | 172 | 98 | 1.76 |
| N30720 K32 T4 | 186 | 151 | 116 | 1.30 |
| Unbalanced N61440 K10 T8 | 140 | 129 | 49 | 2.63 |
| N3840 K3 T1 | 25 | 20 | 11 | 1.82 |
| N245760 K6 T8 | 474 | 406 | 158 | 2.57 |

Native gains span **7.9–20.0%**, with **0.20–18.89 MB** extra tracked peak. Every
cell still loses to stock. The additional N-by-K finite-design copy is a material
memory cost and is excluded from low memory. Both fresh small independent
Poisson/Hessian pilots pass for baseline, candidate and stock; all large public
state and pair checks pass. Distributions remain in the report and receipts.

**134 distinct Rust tests**, actual-plugin ABI/panic checks and **four licensed
fixtures** pass. The licensed coverage includes weighted explicit-dummy inference,
separation, saved outputs and real sixteen-iteration allocator audits at T1/T4.
The audit caught allocate/free churn despite stable capacities; grouping dependent
worker phases reduced seven external submissions to two and the unchanged audit
now passes. All **206 public applications** across development and final screens
pass, with no overlap/monitor errors. All **22 excluded PPML/OLS pairs** retain
exact scientific exports. Initial small low-memory and degree-six losses receive
five further identical-input pairs: 187/188 and 113/113 ms. Preserve both blocks.

Failed broad no-FE routing, invalidated-session audit access, new-fixture syntax
and offset-identity errors, diagnostic setup errors and sealing false positives
are preserved. The obsolete hidden allocation fixture fails the same wire-minor
assertion on accepted baseline and prototype; this milestone does not repair it.
The successful new allocator fixture is separate evidence.

### Next bounded implementation sequence

1. **Share exact final-state construction.** Final-source K10/T4 strict-final
   work is 35.83 ms; N245760/K6 is 131.34 ms. A separate actual-FE diagnostic
   locates two full finite rank solves of about 8 ms each around final acceptance,
   despite identical final weights and no final control reprojection. Separate
   response-dependent work from current-weight design/rank/factor products;
   bind reuse to complete owned generations and invalidate on mutation/errors.
   Preserve fresh residual, score, model-space, reconstruction and Hessian checks.
   Also investigate ownership of the new finite-design copy. Establish a full
   cost budget: K10/T4 still needs 81.3 ms (63%) reduction to reach 0.90, so this
   duplicate alone cannot close the public gap. Use an ADR for checking changes.
2. **Reduce ordinary slope work fundamentally.** Existing slope Schur already
   has weighted geometry reuse, fusion and individual stopping. Compare a joint
   finite-plus-slope Newton/Krylov update, avoiding K+1 ordinary absorptions,
   against shared-load batched Schur products/reconstruction. Use identical
   frozen frames and complete setup/certificate costs; retain one coherent
   successful organization. The prior threaded gaps still need 34–45% less
   public time. Do not repeat the rejected residual extrapolation or tune the
   prepared-operator selector to existing seeds.
3. **Low-memory finite and strict solving.** Prior source-bound diagnostics show
   114.8 ms finite and 70.6 ms strict-final work. Develop bounded block/streamed
   stable finite solving and valid same-frame reuse without a hidden resident
   N-by-K bank. Count scratch passes, standardization, rank and reconstruction;
   measure runtime and admitted/actual peaks together.
4. **Categorical multi-FE and shared passes.** Extend the existing joint and
   projection mechanisms using structural dimensions/connectivity/RHS/memory
   evidence, protecting already competitive dense-three-FE cases. Then address
   generated terms, objective/score passes, inference and saved outputs where
   measured. A shared OLS adapter needs separate OLS oracles and controls.
5. **Fresh broad confirmation.** Extend bounded cases around measured weaknesses
   in imbalance/connectivity, multiple slopes, widths, weights, rank, threads,
   budgets and outputs. For each milestone, freeze baseline/science, implement,
   check independent oracles/boundaries, run uncontended alternating rotations,
   confirm fresh cases and controls, then commit or reject with immutable evidence.
   Keep per-cell distributions/losses and unranked scientific discrepancies visible.

Seeds **202609321 and 202609324 are consumed**. Seeds322/323 were declared but
never generated; **202609325 is next unused for new work**. Static allocation
inventory drift, weighted constrained-fit failures, generic nonretaining-CG
instability, older comparator/factor-provenance issues and Linux/SCC/physical-cap
deferrals remain open. Frozen 77/16/111 definitions are unchanged. No push,
release, SCC authentication retry or stopped campaign restart occurred.

## Symbolic PPML coupling accepted; broader target remains active — 2026-09-23

The active objective remains public PPML runtime at most **0.90 times scientifically
comparable pinned stock fixest** wherever reasonably achievable, while protecting
OLS and scientific/transaction/memory contracts. This checkpoint completes the
prepared-operator successor, not that broader objective. The current implementation
sequence below supersedes older historical next-step entries.

The [source-bound report](benchmarks/reports/development/ppml-symbolic-coupling-local-2026-09-23/README.md)
and [ADR 0126](docs/adr/0126-ppml-symbolic-coupling.md) accept `slsymbol2`, based on
`a2147baf`. All **532 computational inputs** match plugin
`f86fd3e0b9864874df5a5700233e7e5a1b9bce347181d4772e07982db6159740`.
The restored immutable container verifies **7,794 logical files**, SHA-256
`2a008189fbe88c83557c0a072933e339eb13cf9725f04874fc313ad4121e8705`.

Prepare symbolic group-pair topology and observation mappings once; refill only
current-weight numeric blocks. Compact execution avoids the rejected dense D
bank; small systems may form explicit Schur under the unchanged predeclared
structural work margin. Four CG vectors now store only remaining coordinates,
with one eliminated-group scratch bank per worker. Original-RHS reconstruction,
all-partition moments and full fresh acceptance remain authoritative. Existing
sample/topology/weight generations govern reuse; errors revoke it. The operator
is optional after numerical-solver/certificate admission. No public OLS or
categorical PPML routing, rank/tolerance/weight/sample/DoF/separation policy changes.

Fresh seed319 baseline/candidate/strict-stock medians (ms):

| PPML case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Two slope partitions N30720 K6 T4 | 189 | 161 | 118 | 1.36 |
| Unbalanced three FE N30720 K6 T4 | 191 | 178 | 113 | 1.58 |
| Larger unbalanced N61440 K10 T8 | 376 | 367 | 226 | 1.62 |
| Two slope partitions N30720 K6 T1 | 292 | 208 | 319 | 0.65 |
| Unbalanced native low memory | 393 | 358 | 119 | 3.01 |
| Multiple slopes N7680 K6 T4 | 61 | 55 | 55 | 1.00 |
| Four FE N7680 K6 T4 | 99 | 67 | 207 | 0.32 |

All large-cell public-state/pair checks pass. Native gains span **2.4–32.3%**,
with **0.19–4.30 MB** extra tracked peak. Serial and four-FE cell medians clear
the target; the others do not. Native low versus ordinary stock is not budget
matched. A fresh smaller two-partition stock pilot fails four independent
covariance coordinates (maximum tolerance ratio 1.33073) and stays unranked;
this limits family-level claims despite the passing large pairs. Earlier stock
multiple-slope discrepancies are not resolved by another seed's success.

**36 distinct Rust tests**, actual-plugin ABI/panic checks, six frozen T1/T4
independent WLS/span checks, all six final native Poisson/Hessian pilots and three
licensed fixtures pass. Across the two symbolic candidates, all **193 public
applications** pass with no timeout/overlap/monitor error; the stock scientific
failure remains visible. All **29 excluded native OLS/categorical pairs** retain
exact exports. Small categorical median losses remain recorded. A 51/54-ms factor
OLS loss did not reproduce in five further pairs, 53/51 ms. Original and corrected
controller/sealing failures are preserved. The report gives distributions and
memory/validation limits; no full qualification was run.

On the identical wide diagnostic input, numeric preparation falls from the
rejected operator's **48.62 to 11.28 ms**, plus **2.01 ms** once for topology.
All 540 products use the prepared operator. Absorption still costs about 150 ms,
including 19.7 QR, 19.3 fresh certificates and 11.3 prepared products setup.
A final fresh low-memory call spends **114.8 ms finite solving** and **70.6 ms
strict-final work**. These measured costs, rather than another cache threshold,
should drive the next changes.

### Next bounded implementation sequence

1. **Dedicated one-categorical-FE profiled Poisson.** Current fresh control is
   149/56 ms candidate/stock; the 0.90 target is about 50 ms, requiring roughly
   66% less native public time. Verify the group-intercept elimination route in
   exact current source, then prototype analytic group normalization for each
   finite-beta iterate, centered gradient/Hessian accumulation and a safeguarded
   finite-dimensional Newton solve. Retain the canonical sample/separation/rank
   boundary and independent current-state strict/final-Hessian acceptance.
   Compare complete preparation, iteration, verification and output costs. This
   is the next subsystem; do not resume tuning the prepared work selector.
2. **Low-memory finite and strict solving.** Profile scratch reads, QR/moments,
   standardization and repeated scans on the new baseline. Implement a bounded
   streamed/block finite solve and reuse valid same-frame work without a hidden
   resident N-by-K copy. The present 358/119-ms gap needs about 70% reduction to
   clear 0.90; reducing projection alone cannot deliver it.
3. **Outer/strict/final work sharing and slope RHS execution.** Separate required
   independent certificates from duplicate construction or loading. Preserve
   fresh final state/Hessian authority while sharing generation-valid products.
   For remaining slope costs, test RHS block application with individual stopping
   and cheaper reconstruction organization. The current T4/T8 losses still need
   roughly 34–45% reduction for 0.90, so demand a whole-command cost budget before
   another implementation. Do not recycle rejected residual extrapolation.
4. **Categorical multi-FE routing.** Degree-six two FE is 233/138 ms; unbalanced
   three FE is 183/115. Compare coefficient/Schur and current LSMR organization on
   frozen weights; retain the already competitive dense-three-FE control (192/249).
   Use topology/RHS/memory evidence, not design labels, for any route change.
5. **Broaden practical coverage and protect shared work.** Extend bounded cases
   adaptively to larger N, wider/generated controls, weights, cluster inference,
   saved effects, difficult connectivity/rank and tight budgets. Decompose
   fit-only/inference/output/loading costs and remove remaining repeated passes.
   A shared multipartition OLS adapter is allowed only after independent OLS
   oracles and current controls demonstrate benefit; preserve closed serial
   weighted-slope tuning and the frozen 77/16/111 definitions.

Each milestone must freeze current baseline science/timing, set an operation and
memory hypothesis, implement one coherent change, pass independent/boundary tests,
measure alternating baseline/candidate/stock rotations, confirm on predeclared
fresh inputs, then commit or explicitly reject. Publish every loss/discrepancy and
move subsystems when remaining gains are small or inconsistent. Target 0.90 per
comparable cell, with distributions; no pooled win rate substitutes for it.

Seeds **202609318–202609320 are consumed**; **202609321 is next unused**.
Static allocation inventory drift, weighted constrained-fit failures, generic
nonretaining-CG instability, earlier comparator discrepancies, hidden receipt/
factor provenance issues and Linux/SCC/physical-cap deferrals remain open.
No push, release, SCC authentication retry or stopped campaign restart occurred.

## Prepared PPML coupling integration rejected — 2026-09-23

The full implementation goal remains active: public PPML at most **0.90 times
scientifically comparable pinned stock fixest** wherever reasonably achievable,
with OLS and the existing scientific/transaction/memory boundaries protected.
Production remains computationally identical to `9955bd36` / `slframe3`;
all **531 inputs** have been verified after removing the rejected candidate.

The [source-bound report](benchmarks/reports/development/ppml-prepared-coupling-probe-local-2026-09-23/README.md)
and ADR 0125 preserve a complete first prepared-operator experiment. Frozen
native absorption favored explicit Schur for two FE (12.46 to 8.78 ms at T4)
and compact products for three FE (14.19 to 10.64 ms); both lost on the
935-row disconnected frame. All 54 native calls and 18 independent WLS/span
audits pass. These are inner diagnostics, not public speed claims.

The integrated `slcouple1` adds generation-authorized reuse, complete conservative
optional admission, independent certification and structural work selection.
Three public rotations, baseline/candidate/strict pinned stock (ms), give
**186/180/114** for two FE, **181/176/115** for three FE,
**387/399/230** for the wider T8 case, and **361/339/118** in low memory.
Gains of 2.8–6.1% do not justify the wide 3.1% loss or additional tracked peaks
of **3.98–20.84 MB**. All 42 public screen/pilot applications, their public-state
and pairwise checks, both independent small Poisson/Hessian pilots, 35 distinct
Rust tests, actual-plugin ABI checks and three licensed fixtures pass. No timeout,
fit/build overlap or monitor error. Candidate science passed; performance and
memory organization caused rejection. It is not enabled in production.

A separate instrumented build on identical screen data locates **48.62 ms** of
wide coupling preparation across eight weight frames: C construction 27.22 ms,
D construction 17.02 ms. All 540 prepared applications avoid the cancellation
fallback. The current prototype repeatedly builds sparse patterns and uses a
dense D construction bank; faster products alone do not repay that preparation.
The receipt container restores and verifies **3,071 logical files**, SHA-256
`9f87180b9d85d2c954192e5cb489706adb4983160d5f48c95f16fee57dff43a9`.
Native/prototype and production-shaped source snapshots, all clocks/audits,
failed build recipes and omitted-data hashes are preserved. Raw licensed logs
and binaries remain outside Git. No additional holdout or excluded-path timing
block was run after rejection; accepted production code was restored instead.

**Next bounded implementation:** retain symbolic cross-partition sparsity and
observation-to-block mappings with the immutable topology, refill only numeric
weighted blocks, and remove the dense D construction bank from compact execution.
Measure full preparation plus applications; then consider RHS tiling and compact
remaining-coordinate vectors if products still dominate. Do not tune the rejected
20% work selector to these inputs. Prior response/control residuals already provide
a mathematical warm start; another coefficient cache must add a real benefit.

The remaining plan also includes outer/checkpoint work sharing, dedicated
one-categorical-FE profiled Poisson, low-memory finite solving, categorical
2+/3+ FE losses, broader PPML coverage and a selectively admitted shared OLS
adapter. None is completed by this rejected experiment. Seed **202609318** is
consumed; **202609319 remains unused** (previously reserved for fresh acceptance).
Frozen 77/16/111 definitions and all previously recorded allocation, constrained-fit,
CG, comparator and platform deferrals remain open. No push, release, qualification,
SCC retry or stopped campaign restart occurred.

## PPML frame geometry and fresh certificates accepted — 2026-09-23

The broader implementation objective is active: aim for public PPML runtime
at most **0.90 times scientifically comparable pinned stock fixest**, wherever
reasonably achievable. This checkpoint completes the first implementation
milestone of the new plan; it does not complete that objective.

The [source-bound report](benchmarks/reports/development/ppml-frame-certificates-local-2026-09-23/README.md)
and ADR 0124 accept `slframe3`. All **531 computational inputs** match plugin
`c5e41cef298abced05af47874afc957eb4e6ffa0e5ad13174e58d3e7624c1b5a`.
The restored immutable receipt container verifies 7,518 logical files, SHA-256
`940650d9cb08ef812efd07bbea8491582d10f97319ad7d2e159421e33413c97c`.

Fresh certificate groups now share row loads in bounded RHS tiles, use the
existing native team, and merge in the original scalar order. Local weighted
QR can persist across compatible calls only under explicit sample/topology/
weight generations supplied by the owning linear session. Every call still
gets fresh acceptance; changed generations, errors and unframed calls revoke
reuse. The extra result bank is independently optional: refusing it preserves
the coefficient solver with serial certification. No numerical or scientific
policy, convergence schedule, sample, output or public routing changes.

Fresh baseline/candidate/strict-stock medians (ms) are two-partition T4
**219/187/118**, unbalanced three FE **214/180/120**, larger K10/T8
**491/385/232**, serial **318/308/315**, low memory **407/357/118**, and
multiple slopes **73/64/53**. Native gains range 3.1–21.6%; most stock gaps
remain. The multiple-slope large comparison passes pairwise/public checks,
but a smaller fresh stock pilot fails five covariance coordinates against the
independent Hessian oracle (maximum tolerance ratio 1.57822); keep that failure
unranked and do not make an unqualified family-level comparison.

On identical diagnostic inputs, wide fresh certification drops **133 to 20 ms**;
geometry builds drop 11 to 8. Low-memory geometry builds drop 19 to 8. Added
tracked peak is **0.0406–0.3779 MB**, matching modeled increments. This is not
physical-cap qualification or a whole-action allocation proof.

34 distinct Rust tests, actual-plugin ABI/panic checks, and three licensed
fixtures pass. Independent native Poisson/Hessian pilots and all final native
public states pass; 88 native pairs preserve exact scientific exports/means.
Of 254 benchmark attempts, 242 pass and 12 stock setup attempts fail before
fitting due to an omitted existing R dependency path; corrected rotations and
all failures are retained. No timeout, competing fit/build or monitor error.
An oversized independent pilot was replaced with smaller fresh cases within
the unchanged oracle bound, preserving its refusal and original applications.

Current categorical PPML candidate/stock medians are dense three FE **196/239**,
one FE **145/54**, degree-six two FE **229/139**, and unbalanced three FE
**181/110** ms. Fresh weak/disconnected slope cases pass same-cell independent
oracles and give **14/23** and **42/756** ms. These bounded cells clear the
0.90 median target; they do not establish broad topology or qualification claims.
Excluded controls retain exact science. Five-pair confirmation leaves a small
dense categorical PPML loss **190/193** ms with overlapping ranges; one-FE
saved-output OLS is **17/17** ms. The initial categorical OLS 43/44-ms loss is
also recorded, not silently dismissed.

**Next bounded implementation target:** compare prepared cross-partition
coupling operators and current-weight coefficient warm starts against the
remaining observation-based Schur operator on identical frozen frames. The
wide diagnostic still spends about 119 ms in absorption outside geometry and
certification. Count setup, coupling fill, applications and memory before
choosing compact versus observation-based execution; no benchmark-name guards.
Low-memory finite solving remains a separate large cost (104 ms ordinary,
81 ms strict-final in a representative fresh call). One-FE profiled Poisson
optimization is also now supported by a current measured gap. Outstanding plan
items include outer/checkpoint work sharing, low-memory redesign, categorical
specialization, broader PPML coverage, and a selectively admitted shared OLS
adapter; none is completed by this certificate milestone.

Seeds 202609311–202609317 are consumed; 202609318 is next unused. Frozen
77/16/111 definitions, static allocation drift, constrained-fit/CG failures,
hidden PPML receipt assertion, factor-base provenance and Linux/SCC/physical-cap
deferrals remain open. No push, release, qualification, SCC retry or stopped
campaign restart occurred. Historical closeouts below remain source-bound.

## Absorbed-slope solver replacement accepted — 2026-09-23

The [source-bound replacement report](benchmarks/reports/development/ppml-coefficient-schur-local-2026-09-23/README.md)
and ADR 0123 close the implementation milestone. All **531 computational inputs**
match accepted `slschur3`, plugin SHA-256
`ed5e7855efbea24863d6ab9d6c0dcecf260459339be75563d19feaa0bd689467`.
The 6,905 logical receipt files verify in the deduplicated immutable container,
SHA-256 `7d41ec832b8c5961acf037a3416f008fb12a5359d81ed37cdabeeb903ba1b899`.
Earlier geometry, diagnosis and rejected-probe evidence remains unchanged.

Adopt current-weight rank-aware coefficient Schur/CG for admitted publication
PPML with multipartition slopes. Prepare local QR once per absorption, eliminate
one partition exactly, stop columns independently, balance RHS work, and fuse
dense response/control warm starts. Reconstruct from the original RHS and run
the full fresh acceptance certificate; remove only the redundant full input
certificate used for early return. Every call rebuilds weighted geometry.
Insufficient optional workspace retains bounded generic MAP. The public MAP
owner includes this kernel; reported Krylov counts are not symmetric sweep counts.
Categorical, one-effect, mu separation, non-MAP families and public OLS routing
remain unchanged. No tolerances, rank/weight/DoF policy or final-Hessian gates
were relaxed; no unrelated generic nonretaining-CG repair is claimed.

Established robust/mean-output medians against the preserved current baseline
and strict pinned stock (ms) are two-FE N30720/K6/T4 **727 → 213 / 120**, and
unbalanced three-FE **524 → 193 / 117**. Fresh seed310 results are two-FE T4
**768 → 216 / 120**, wide N61440/K10/T8 **1661 → 447 / 231**, serial two-FE
**1347 → 312 / 325**, low memory **689 → 355 / 117**, and multiple slopes
**633 → 74 / 52**. Fresh reductions range 48.5–88.3%; stock losses remain,
particularly low memory. The report contains distributions, not pooled win claims.

The old captured 935-row disconnected warmup that exhausted 16000 MAP sweeps
now passes strict projection and independent WLS/model-space checks in 257
Schur steps. Fresh small weak/disconnected public pilots pass independent
Poisson/Hessian checks for both implementations; single observations are
15/39 ms native versus 23/356 ms stock. The earlier larger stock covariance
failure remains unresolved and is not reclassified by these different cases.

54 distinct focused Rust tests and three licensed fixtures pass on the final
source, including independent multipartition unit/FW/PW Poisson/Hessian and
robust/cluster inference, saved slopes and separation. All 225 completed
application attempts pass without timeout/overlap/monitor error; final scientific
comparisons and explicit oracles pass separately. OLS and categorical PPML
controls preserve scientific exports exactly. Small initial 2-ms OLS losses
were checked once more: multipartition OLS 68/69 ms and generated-factor OLS
51/50 ms over five pairs, with overlapping ranges. Do not claim universal
absence of performance regressions from these bounded controls.

Added tracked peak is 1.41–11.29 MB across fresh cases, explicitly admitted
through the linear-session lifetime. Static allocation inventory drift remains
an unresolved failure; this is not a whole-action proof or constrained-memory
qualification. Prototype normal-CG and coefficient-IT evidence, discarded
Python near-zero-RHS failure, and excluded accidental T1 reference clocks are
preserved. Only unmodified pinned stock supplies headline timings.

**Next bounded target:** fresh certificate scans (128 ms in the wide diagnostic),
then low-memory finite solving and repeated response-only projections. Use the
existing tiling/group-parallel mechanisms as candidates while preserving fresh
acceptance. Same-frame geometry reuse needs explicit generation authority.
Frozen WLS replays also support adapting the same kernel to public OLS through
a borrowed owner and full-fit admission, without a second solver implementation.
No such OLS speed claim or automatic routing change is made at this checkpoint.

Seeds 202609306–202609310 are consumed; 202609311 is next unused. Frozen
77/16/111 definitions, Linux/SCC/physical-memory deferrals, allocation drift,
hidden PPML receipt assertion, factor-base provenance, constrained-fit and CG
issues remain open. No push, release, qualification, SCC authentication retry
or stopped campaign was initiated. The requested production solver milestone
is complete; further subsystem work requires a new bounded milestone.

## Disconnected PPML diagnosis complete — 2026-09-23

The [reduced diagnosis](benchmarks/reports/development/ppml-disconnected-map-local-2026-09-23/README.md)
and ADR 0122 close that experiment at the user's intervening pause request.
At closeout no new solver experiment, larger timing block or comparator probe
had started. Production remains `190946a1`; all 529 computational inputs still
match accepted `ppmlgeom1` and plugin SHA-256
`895b2ae2ce57094a1290255d6e6f02ae15f7e374441e340caf5052d490003875`.
The prior geometry gains and tests remain bound to their original evidence.

Two predeclared disconnected three-FE/two-slope cases (N960/K3 and N1920/K4,
T4, robust, saved means) reproduce native `r(459)` during warmup. Both small
warmup datasets equal the full data. There are no native target clocks to
rank. Strict stock succeeds in single diagnostic calls of 1.195/.554 seconds;
the four completed attempts have no timeout, overlap or monitor error.

An isolated diagnostic captures the first failing MAP invocation: 935 rows,
five RHSs, all weights exactly one, tolerance `1e-6`, limit 16000. The final
fresh infinity certificate is `8.61836e-5` and still contracting. Independent
dense SVD and pivoted QR agree on rank 645 and residuals to `8.31e-14`; an
independent local QR certificate reproduces the native failure. Remaining FE
components reach 1.73% of an original RHS norm; leakage outside the FE span
is below `9.88e-15`. The failure is slow initial projection convergence before
IRLS advances, not stale changing-weight cache geometry. No tolerance, rank,
sample, recurrence or iteration limit was changed.

Both reduced cases have independent explicit Poisson/sample oracles. Stock
passes coefficients, means, objectives and fresh public-state checks in both.
The smaller case also passes covariance. The larger fails three covariance
coordinates: maximum error `5.48511e-10`, tolerance ratio 2.572. Dense SVD/QR
current-Hessian inference confirms the discrepancy, including at stock's own
means; its exact internal cause remains unresolved. Keep that comparison
scientifically unranked. No additional stock fit or instrumentation was run.

The 72-entry archive verifies SHA-256
`4468cedff139071d3d9fc8356399630a5d9b3a35df4016d571d9d7481febcce2`.
It preserves the bounded synthetic failed frame, independent audit scripts,
all outcomes, diagnostic source and protocols without raw licensed logs.
The diagnostic actual-plugin ABI/panic checks and one licensed expected-failure
capture fixture pass. This is not a passing native estimation, production
memory claim, new Rust-suite run or qualification campaign.

**Recommendation if work resumes:** predeclare a certified convergence remedy
on the bounded frame, then validate fresh topology/weight and excluded-path
controls. Do not repeat long failed fits, resume the stopped timing block or
silently substitute unresolved nonretaining CG. ADR 0120's rejected
extrapolation remains closed absent a separate new experiment. The stock
covariance discrepancy is a separate comparator-adjudication task.

Seed 202609305 is consumed; 202609306 onward are unused by this experiment.
Frozen 77/16/111 definitions, static allocation drift, hidden PPML receipt
assertion, factor-base provenance, constrained-fit/CG issues and SCC/Linux
deferrals remain unchanged. No push, release, qualification or stopped
historical campaign was initiated. The broad optimization objective remains
open; the redesign request above now governs further work.

## Current-call PPML certificate geometry accepted — 2026-09-23

The [geometry milestone](benchmarks/reports/development/ppml-certificate-geometry-local-2026-09-23/README.md)
and ADR 0121 retain identical weighted QR bases only within one reusable MAP
absorption call. Validity starts false on the call stack every time. Capacity
is optionally admitted for the complete linear-session lifetime, after the
existing projection bank; refusal keeps uncached certification. All residual
norms, moments, span checks, rank rules, iterations, final Hessian and public
certificates remain fresh and unchanged. No weight-dependent authority crosses
an invocation, including response-only and low-memory block calls.

All 529 computational inputs match `ppmlgeom1`: `5cb0434a` plus the archived
five core overlays, plugin SHA-256
`895b2ae2ce57094a1290255d6e6f02ae15f7e374441e340caf5052d490003875`.
Five successful slope rows gain 11.3–20.4% by paired medians: two FE
905→723 ms, unbalanced three FE 594→513, fresh wider T8 1280→1137,
serial 1478→1289 and low memory 859→762. Added tracked heap is 1.0–2.5 MB.
Strict unmodified stock still wins all five. The categorical PPML control
is 188 versus stock 234 ms on this artifact; its small baseline difference
is not attributed to the excluded cache. The OLS categorical control loses
2.3% at the paired median while both medians remain 43 ms. Distributions and
all losses remain in the report; do not tune new guards to these rows.

All 26 successful native pairs have exact scientific exports and non-clock
solver fields; all 20 native mean pairs and 44 successful repeat checks are
exact. Both independent Poisson/current-Hessian/rank pilots and all 18
comparable headline stock comparisons pass. Sixty-four distinct Rust tests,
ten harness tests, actual-plugin ABI/panic checks and three licensed fixtures
pass. Changing weights/RHSs, numerical-rank changes, budget boundaries,
strict rollback, saved slopes and independent FE/Poisson science are covered.

**New remaining gap:** the disconnected three-FE/two-slope control exhausts
MAP's iteration budget on both baseline and candidate (`r(459)`). Baseline
takes 52.244 seconds; two candidate failures take 43.583/43.898 seconds.
These are not ranked as successful-fit timings. Stock succeeds in 85.987
seconds and passes public score, model-space and objective replay checks.
A separate noisy diagnostic confirms the native error and no partial public
mean or `e(cmd)`. This convergence failure is not repaired by the cache.

The fresh timing block was stopped after the first failed native pair; a
second candidate had completed and a second stock attempt was interrupted.
Four declared arms remain unattempted. Overall screen receipts retain 76
completed calls (73 successes, three application failures), the interrupted
attempt, original plans and explicit missing comparisons. No completed call
has a timeout, overlap or monitor error. Do not resume this failed timing block.

Separate profiles keep 1320/1120 projections and 130/72 fresh certificates,
while certificate time falls .516→.310 and .241→.169 seconds. Normalization
plus QR falls .219→.034 and .090→.020 seconds. Eight diagnostic calls pass;
they include a first fine-only block and a separate explicitly traced block.
These clocks have overhead and are not headline evidence.

**Next bounded investigation:** reduce and diagnose disconnected slope-MAP
stagnation with a small independent explicit-design oracle, then investigate
a certified remedy. Avoid repeating long failed fits or substituting unresolved
nonretaining CG. Fresh raw-moment batching is a smaller subsequent opportunity.
Close this geometry tuning. ADR 0120's extrapolation experiment stays closed;
any later accelerator requires a separately declared experiment justified by
the lower certificate cost. ADR 0122 is available.

The 2,581-entry archive verifies SHA-256
`741ea9e35ebd3dea38e461001aceda830a11b93d96cd4f85c3f38d932b1a3d87`.
Seeds 202609299–304 are used; 202609305 onward remain unused here. Frozen
77/16/111 definitions, static allocation drift, hidden PPML receipt assertion,
factor-base provenance, constrained-fit/CG issues and SCC/Linux deferrals
remain unchanged. No stopped historical campaign, qualification, push or
release is resumed. The broad goal remains open; this opening supersedes
older next steps.

## PPML slope extrapolation closed; certificate cost localized — 2026-09-23

The [bounded probe](benchmarks/reports/development/ppml-slope-extrapolation-local-2026-09-23/README.md)
and ADR 0120 reject direct per-RHS Irons–Tuck extrapolation for reusable slope
MAP. Production remains `1de0b1bb`; all 529 computational inputs still match
`ppmlproj1`. Only useful isolated certificate instrumentation is retained.
The experiment is archived, not a selectable native route.

At N30720/K6/T4, robust inference and timed saved means, paired medians show
a 0.7% loss for two slope partitions (901→908 ms) and a 2.8% gain for
unbalanced three FE (604→586 ms). Strict stock remains 118/117 ms. Tracked
peak grows about 2.46 MB. Small single-run pilot gains do not justify a fitted
size guard. Do not tune this probe further. All 28 application calls pass,
including two independent Poisson/current-Hessian/rank pilots, fresh public
score/model-space checks and all 12 exact within-variant repeat comparisons.
Cross-recurrence mean differences reach 1.11e-10; bitwise parity is not claimed.
No new Rust or licensed-fixture campaign is claimed for unchanged production.

Isolated profiles show projection work falling, but extra fresh certificates
consume nearly all the savings. Baseline weighted basis normalization and QR
cost about .224/.091 seconds; raw moments cost .132/.092 seconds. Fine clocks
add overhead and are diagnostic only. The next bounded implementation is
**weighted certificate geometry valid within one absorption invocation**:
admit its capacity for the owner lifetime, rebuild/invalidate on every new
call, retain an affordable uncached fallback, and keep residual norms, moments,
span projections and all final certificates fresh. Establish changing-weight/
rank tests before adoption. Do not reuse OLS geometry across IRLS generations
or substitute generic CG with its unresolved nonretaining span boundary.

The 909-entry archive verifies SHA-256
`676aa97c426306a204a928e7b667090c7a40845fba856ca5b29c0d8667cd0daa`.
Seeds 202609297/298 are used; 202609299 onward remain unused here. ADR 0121
is available. Frozen 77/16/111 definitions, static allocation drift, stale
hidden-PPML receipt assertion, factor-base provenance, constrained-fit/CG
issues and SCC/Linux deferrals remain unchanged. No stopped campaign, push,
release or qualification is resumed. The broad goal remains open; this opening
supersedes older next steps.

## Reusable PPML slope projection accepted — 2026-09-23

The [PPML projection milestone](benchmarks/reports/development/reusable-ppml-projection-local-2026-09-23/README.md)
and ADR 0119 extend the accepted parallel group-coefficient/row kernel to the
reusable generic MAP owner. Only topology and scratch storage persist. Each
projection rebuilds coefficients and weighted Grams from its current frame;
all arithmetic, rank, certificates, iterations, final-Hessian requirements
and publication boundaries remain unchanged. This follows the coverage commit
`fa20dfd5`; all 529 computational inputs match candidate `ppmlproj1`, plugin
`f42ed127513640759dd4b0135c665d39e49061b40627390574bbb81fe48b17c2`.

Four eligible threaded PPML rows gain 27.8–37.8% by paired medians:
two slope partitions 1463→910 ms, unbalanced three FE slopes 918→596 ms,
fresh wider T8 2201→1406 ms and low memory 1158→832 ms. Added tracked heap
is 35–656 KB. The optional bank is admitted for the entire linear-session
lifetime, has stable reported capacities, and falls back to serial if it
cannot fit or allocate. Explicit non-MAP engines, Mu exploration, categorical
specializations and serial/small fits retain their execution. Mu's separate
parent separation allowance is not enlarged.

Comparisons use the pinned unmodified stock library with the previously
adjudicated public tolerances `glm.tol=1e-10, fixef.tol=1e-11`, robust inference
and means inside both command timers. Stock still wins all slope rows. Serial
PPML has a 0.4% paired median loss (1437→1443 ms); excluded OLS slope medians
are 67→68 ms. Categorical controls are effectively unchanged. These losses
and all distributions remain in the report. Earlier default-stock scientific
discrepancies are neither erased nor ranked as comparable timings.

All 66 headline calls pass without timeout, overlap or monitor errors. All 26
native pairs including pilots have exact scientific exports and non-clock
solver fields; all 20 mean pairs and 44 repeat comparisons are exact. All 18
headline stock comparisons and fresh public states pass. Two independent
Poisson/Hessian/rank pilots pass for baseline, candidate and strict stock.
Sixty-five distinct Rust tests, actual-plugin ABI/panic checks and four
licensed saved-slope/weight-inference/OLS transaction fixtures pass.

**Visible fixture failure:** `estimate_ppml_hidden_transaction.do` fails on
both baseline and candidate at its unchanged receipt minor-version assertion
(2 versus current source 3). Prior injected transaction stages run, but the
whole fixture is not counted as passing. Its repair is separate from this
performance change. No tolerances, samples or assertion are weakened.

Isolated profiles retain 1476/1240 projections while projection time falls
1.086→.472 and .644→.325 seconds. Fresh certificates stay about .470/.249
seconds. Close this scheduling milestone; do not tune thresholds or chunks.
**Next bounded investigation:** reduce repeated certified slope MAP work in
PPML, first examining an accelerator scoped to the current weight/response
frame and fresh certificate costs. Retain current-generation ownership,
rank and final-Hessian checks. Weighted/clustered, weak/disconnected and
separation-heavy slope throughput remains unmeasured by this screen.

The 2,355-entry archive verifies SHA-256
`bb65391b56f47e49679a1455b10731c86c9267a2256f28de9de8dedbb7dc751d`.
Seeds 202609292–296 are used; 202609297 onward remain unused here. Frozen
77/16/111 definitions, static allocation drift, factor-base provenance,
constrained-fit/nonretaining-CG issues and SCC/Linux deferrals remain unchanged.
No stopped campaign, qualification, push or release is resumed. The broad
performance goal remains open; this opening supersedes older next steps.

## PPML slope opportunity verified; matched mean harness added — 2026-09-23

The [PPML breadth milestone](benchmarks/reports/development/ppml-slope-opportunity-local-2026-09-23/README.md)
adds `--ppml-surface`, positive-outcome numeric slope designs, timed public means,
independent explicit-design Poisson/Hessian checks, and fresh public score/
fixed-beta model-space diagnostics. This checkpoint changes benchmark/audit
code only. All 529 computational inputs still match `parproj1` / `31a7f201`,
plugin `2ed364308f8c7e239e242414621e23cf6b59d1d9a50d6ab0abd09775e419f80a`.
No new native speedup, relaxed tolerance or memory policy is claimed.

At N30720/T4 with saved means, default native medians are 206 ms for dense
categorical three FE, 169 ms unbalanced categorical, 1488 ms two slope
partitions, 1017 ms unbalanced three FE with slopes and 1196 ms in low memory.
Default stock medians are 120/71/71/71/71 ms, but only six of 15 full-size
comparisons pass completely: three families miss saved means, two also miss
stock stationarity. All native public states pass. The three N2400 native
pilots pass independent science; all default stock pilots miss means and one
also misses covariance. Those default comparisons are not a blanket ranking.

A separate stock diagnostic uses the same pinned library with public
`glm.tol=1e-10, fixef.tol=1e-11`. All three small independent pilots and all
five full-size comparisons/public-state checks pass. Single-run native/strict
stock times are 196/244, 172/118, 1475/118, 1014/117 and 1194/118 ms. This
isolates a large slope opportunity; dense categorical three FE is competitive
in that specific diagnostic. It does not replace the failed default receipts.
The first stricter request used unsupported fixef.tol=1e-12, producing three
stock warmup failures; those errors are retained, not scientific failures.

Isolated native profiles identify reusable generic MAP projection as the
next cost: 1,380/1,260/1,320 projection calls take .999/.655/.695 seconds across
the three slope cases. Fresh certificates take .432/.256/.258 seconds.
`ReusableMapAbsorber` retains topology and storage but uses serial projection;
it does not enter ADR 0118's one-shot automatic OLS plan. Surrounding retained
finite design and fresh reference-statistic improvements already apply.
Saved-mean publication itself is only about 5–6 ms here. Individual saved FE
channels, weights/clusters and separation-heavy slope performance are still
outside this new coverage.

Final review caught the initial worker omitting explicit memory(fast): every
such captured run actually requested auto and selected fast. The report keeps
that label and those original receipts. The corrected worker emits explicit
fast/low requests; repeated small pilots have exact native exports/means and
pass independent/reference/state checks with the intended request codes.
There are 67 application attempts: 64 successes and three stock API refusals,
with no timeout, overlap or monitor error. All 20 repeat comparisons, means
and scientific exports are exact. Ten harness tests and diagnostic-plugin ABI/
panic containment pass. No new Rust or licensed-fixture campaign is claimed.

**Next bounded implementation:** admit the existing parallel group-coefficient/
row projection in the reusable PPML slope path. Recompute coefficients every
projection; reuse storage/topology only. Respect PPML fit-lifetime admission,
stable capacities, weight/sample generations, fresh certificates and affordable
serial fallback. Inspect `prepared_irls.rs` solver admission, retained-session
construction/capacity accounting and the projection plan together; do not simply
remove OLS guards or keep weighted geometry across IRLS iterations. Begin with
the verified two-slope and unbalanced-three-FE cases, then categorical, serial,
low-memory and changing-weight controls. Seeds 202609289–291 are used;
202609292 onward remain unused here. ADR 0119 remains available.

Primary and memory-correction archives verify 1,548 and 158 entries with SHA-256
`4ed1b2a5f452a6265bde632680a8e7f6b3d574b79c51cbffba1ecc93bf3f6433`
and `6a39a61e30993ee1916fcd5fc714faa7c135023730b72e868c24bbcedba2f354`.
Frozen 77/16/111 definitions, static allocation drift, weighted constrained-fit/
nonretaining CG issues, factor-base provenance and SCC/Linux deferrals remain
unchanged. No stopped campaign, push, release or qualification is resumed.
This is verified progress toward the broad goal; the native PPML gap is open.
This opening supersedes older next steps.

## Parallel owned-FE projection groups accepted — 2026-09-23

The [projection milestone](benchmarks/reports/development/parallel-projection-local-2026-09-23/README.md)
and ADR 0118 split independent group coefficient solves from disjoint row
subtraction on the installed team. Automatic owned slopes require at least
16,384 rows, nonempty RHSs and more than one worker. Original raw Gram/pivot/
rank rules, observation/channel arithmetic and per-entry subtraction remain
unchanged. No tolerance, certificate, recurrence, contraction, routing or
publication rule changes. Direct/serial/small paths, categorical borrowed
solvers and the separate changing-weight reusable MAP keep their execution.

A fallible optional bank adds at most `8*(L*C*R + T*C*C)` numeric bytes plus
T worker descriptors, beside the existing serial workspace. Admission follows
geometry/certificate priority and cannot displace an affordable route. MAP
releases its bank before LSMR/CG; CG admits a new bank after selecting its
context. Allocation/size refusal uses serial execution. Earlier attempts stay
in the peak. Coefficients are refreshed every projection; nothing survives
an invocation or crosses PPML weight/sample generations.

`parproj1` is `9a1d8db2` plus the archived projection/MAP/accelerator/automatic
overlays; all 529 computational inputs match. Plugin SHA-256:
`2ed364308f8c7e239e242414621e23cf6b59d1d9a50d6ab0abd09775e419f80a`.
Seven eligible threaded slope rows gain 15.3–36.0% by paired medians. Original
commands improve 213→174 and 276→210 ms, fresh unbalanced three FE
1648→1055 ms, low memory 254→209 ms, AW 196→161 and PW 197→159 ms.
Added tracked heap is 0.116–1.966 MB. Stock still wins all five comparable
rows. Wide serial loses 1.7% by paired median (+17/+25 ms pairs); small
categorical/PPML/explicit-solver losses remain visible. No further guard,
chunk or tile tuning follows this bounded candidate.

All 93 headline calls pass without timeout, overlap or monitor error. All 39
native pairs have exact scientific exports and non-clock solver fields; all
52 repeat checks and 15 direct stock comparisons pass. Three N2400 independent
explicit-design pilots pass for both natives and stock on this seed. Earlier
weak-stock covariance failures remain unresolved; weak timings stay native-only.
Fifty distinct Rust tests, five harness tests, actual-plugin ABI/panic checks
and four licensed slope/saved-PPML/transaction fixtures pass. Exact-budget,
capacity, fallback, independent weighted-formula and cross-thread bit checks
cover the new kernel. No optimized Rust tests or qualification are claimed.

Separate diagnostics retain work counts and confirm all 99 full-data projection
calls use four workers. Initial candidate projections take 33.5/47.3 ms;
fresh certificates remain about 39/50 ms. Baseline fine-grained timer overhead
can inflate phase contrasts, so speed claims use uninstrumented public pairs.
Most remaining native time remains in absorption. The 3,374-entry archive
verifies SHA-256 `2c27996c1f8b9620d6cb914662d8f7db5610e67bed0093b67d248a8ffdb3074c`.

**Next bounded subsystem:** return to public PPML breadth, starting with a small
current-source screen of higher-dimensional/slope designs and saved-output
costs. Performance coverage there remains incomplete. Establish comparable
samples/inference/outputs and profile changing-weight state separately before
choosing an implementation. Do not extend OLS cache lifetimes across IRLS
frames or reopen closed window/serial-slope tuning. Seeds 202609283–288 are
used; 202609289 onward remain unused here. Static allocation source drift,
weighted constrained fits, nonretaining CG span instability, factor-base
provenance and SCC/Linux deferrals remain open. Frozen 77/16/111 definitions
are unchanged. This local committed milestone is not completion of the broad
goal, a qualification, push or release. This opening supersedes older next steps.

## Parallel fresh owned-FE certificates accepted — 2026-09-23

The [parallel-certificate milestone](benchmarks/reports/development/parallel-certificate-local-2026-09-23/README.md)
and ADR 0117 evaluate independent groups on the existing worker team, then
merge each group/RHS result in the original scalar order, including RMS FMA.
It requires admitted current-weight geometry, at least 16,384 observations
and more than one installed worker. Direct solvers, serial/small inputs and
uncached contexts retain serial checks. Every residual norm, span/raw moment,
checkpoint, rank, tolerance, iteration, contraction, route and publication
rule stays unchanged. No state crosses absorption or PPML weight generations.

Additional scratch is bounded by `8*max_effect_levels*(actual_RHS+1)` bytes.
One effect bank is reused within each certificate and released before return.
Allocation/size refusal uses the serial check. CG may drop scratch while
retaining geometry, or release geometry too if required. LSMR still releases
the context before its existing admission. Earlier admitted attempts remain
charged. No new team or observation-sized worker bank is introduced.

`parcert1` is `84de9f95` plus the archived certificate/automatic-routing overlays;
all 529 computational inputs match. Plugin SHA-256:
`d7c556d44ec6eeae06b16715d60b5e194be83b91bede821ded1819ce81776439`.
Seven threaded slope cases gain 16.4–26.9% by paired medians. Original commands
improve 267→211 and 327→238 ms; fresh unbalanced three FE 1939→1498 ms,
low memory 292→245 ms, and AW/PW about 237→184 ms. Additional tracked heap
is 0.064–1.065 MB. Stock still wins all five comparable rows. Explicit MAP
has a 1.9% paired median loss (9–19 ms per pair); wide serial includes +19 ms,
PPML +5 ms and categorical controls +2 ms. No threshold or tile tuning follows.

All 93 headline calls pass without timeout, overlap or monitor error. All 39
native pairs and 52 repeat checks pass with identical scientific exports and
non-clock solver fields. Fifteen direct stock headline checks pass. Both native
artifacts pass three N2400 independent explicit-design pilots. Stock passes two;
weak covariance fails two coordinates (1.08998e-10 maximum, 1.02173 times the
unchanged envelope), while native error is 2.81e-16. Weak stock timing remains
unranked; earlier discrepancies and stopped campaigns remain untouched.

Fifty distinct Rust tests pass, including an independent weighted two-column
projection for the parallel kernel at 1/2/4/8 workers, cross-thread bit checks,
rank/units/RHS boundaries, and exact-budget scratch/geometry fallbacks. Five
harness tests, actual release-plugin ABI/panic containment and four licensed
OLS/PPML/saved-slope/transaction fixtures pass. No optimized Rust unit-test run
or qualification is claimed. The diagnostic confirms the same work counts,
with initial certificate time 113→40 and 124→43 ms; all 21 parallel group
calls use the requested four-worker team. Projection remains about 109–116 ms.

**Next bounded subsystem:** generic owned-FE projection sweeps. Investigate
independent group coefficient work followed by disjoint row subtraction,
with explicit memory admission, unchanged raw Gram/pivot/rank rules and
per-entry arithmetic. Retain current-weight scope and fresh certificates.
Close parallel-certificate tuning; do not chase small serial/control losses.
Seeds 202609277–282 are used; 202609283 onward remain unused here.
The archive verifies 3,362 entries, SHA-256
`9004a9eccd30e3ce54ebb6a9e6c4dfd37993a475198f3d3ba116c5590ac4bb5a`.
Static allocation source drift, weighted constrained fits, nonretaining CG span
instability, factor-base provenance and SCC/Linux deferrals remain open. Frozen
77/16/111 definitions are unchanged. This is a local implementation checkpoint,
not completion of the broad goal, qualification, push or release.

## Scoped owned-FE certificate geometry accepted — 2026-09-23

The [geometry milestone](benchmarks/reports/development/certificate-geometry-local-2026-09-23/README.md)
and ADR 0116 retain weighted QR bases within one immutable design/current-weight
generic automatic MAP/CG absorption. Every residual norm, span projection and
raw moment remains fresh at the same checkpoints. Rank, tolerance, contraction,
routing, iterations, DoF and publication stay unchanged. Direct solvers and the
separate changing-weight reusable MAP implementation remain uncached. Shared
generic calls rebuild for their current weights; nothing crosses PPML frames.

Optional storage is admitted beside existing QR construction/solve scratch.
A refusal or allocation failure continues uncached. LSMR releases the cache
before admission; CG releases it if retaining it would crowd out continuation.
Exact-budget tests preserve affordable routes and charge discarded attempts.
Fast and low modes can benefit, with additional measured heap of 0.67–4.54 MB.

`certcache3` is `51ae723d` plus four archived FE solver/certificate overlays;
all 529 computational inputs match. Plugin SHA-256:
`21bb7a0d0a093e49e22b03e5923f4b13ab7bef2f727b7ebf4c4d30a640552e82`.
Eight automatic slope cases gain 4.7–21.1% by paired medians. Original command
medians improve 314→271 and 362→328 ms; fresh wide 1015→967 ms,
unbalanced three FE 1997→1876 ms, and low memory 373→291 ms. Stock still
wins all five comparable timing rows. Controls retain small losses, including
PPML +2 ms pairs, explicit MAP +1–7 ms and explicit LSMR +7 ms.

All 93 final headline calls pass without timeout, overlap or monitor error.
All 39 native pairs and 52 repeat checks pass; scientific exports and all
non-clock solver fields are exact. Fifteen stock headline checks pass. Three
independent N2400 pilots pass for both native artifacts. Stock passes two;
the weak pilot misses two covariance coordinates (maximum 1.38683e-10,
1.1101 times tolerance). That failure remains archived and weak timings stay
native-only. No tolerance or prior discrepancy is erased. 41 distinct Rust
tests, five harness tests, release-plugin ABI/panic containment and four licensed
OLS/PPML/saved-slope/transaction fixtures pass. No optimized Rust test run or
qualification is claimed.

Diagnostics retain identical certificate/projection counts. Including cache
construction, total certificate time falls 177→127 and 176→140 ms on the
two anchor models. Fresh span/raw moments and projection sweeps remain costly.
The preliminary candidate, untimed intermediate build, setup failures and final
confirmation are separately identified in the immutable report. Seeds
202609271–276 are used; 202609277 onward remain unused in this milestone.

**Next bounded subsystem:** deterministic parallel group evaluation of fresh
owned-FE certificates, with ordered reductions and explicit scratch admission.
Close cache-scope tuning; do not expand across mutable weights or chase small
control losses. The archive verifies 3,864 entries, SHA-256
`7e7dd504d58294761a5f835bfa8a8f6570953301b95c1acc1f08389e3ae6118d`.
Static allocation source drift, weighted constrained fits, nonretaining CG span
instability, factor-base provenance and SCC/Linux deferrals remain open. Frozen
77/16/111 definitions are unchanged. This is a local implementation checkpoint,
not completion of the broad objective, a release, push or qualification.

## Unused MAP initialization certificate removed — 2026-09-22

The [MAP initialization milestone](benchmarks/reports/development/map-initial-certificate-local-2026-09-22/README.md)
and ADR 0115 remove a full initial QR/span certificate that generic one-shot
MAP always overwrote without using. Weighted RHS norms and pre-mutation
weighted-basis validation remain. Every scheduled/final-iteration certificate,
returned residual, iteration count, automatic contraction/routing, tolerance,
rank, DoF and publication rule is unchanged. The router's used initial
certificate and the separate changing-weight reusable MAP implementation stay
intact. There is no new buffer, cache, worker team or memory-admission rule.

`mapinit1` is `7ef711a8` plus the archived `map.rs` overlay; all 529 captured
inputs match. Plugin SHA-256:
`97271f6c7980a31b3601bf32a419469c10f3c721e51a286257d07854af0a63d8`.
Eight slope rows gain 5.6–8.3% by paired medians, including wide, weak,
low-memory, AW robust and PW clustered execution. Original command medians
improve 333→310 and 386→361 ms; fresh wide 780→725 ms and low-memory
381→360 ms. All five comparable stock timing rows still favor fixest.
Categorical/PPML controls are small and mixed, including a +2 ms one-FE loss.

All 81 headline calls pass without timeout, detected overlap or monitor error.
All 33 native pairs and 44 repeat checks pass; scientific exports and every
non-clock solver field are exact. All paired tracked heap peaks match. Fifteen
direct stock comparisons, three independent explicit-design pilots on all
three artifacts, 31 Rust tests, actual-plugin ABI/panic containment and four
licensed OLS/PPML/transaction fixtures pass. Boundary tests preserve scheduled
checks on initially orthogonal inputs, final-budget checks, zero RHSs and
weighted-basis-overflow rejection before mutation. The earlier weak-reference
precision discrepancy and documented exact-rank/conservative-DoF distinction
remain visible; no stopped campaign is restarted.

Native diagnostics confirm certificate calls fall 10→9 for initial absorption
and 3→2 for final residuals, with unchanged projection counts. An isolated,
pinned fixest diagnostic matches stock b/V/sample/DoF/RSS exactly. It spends
only 1–2 ms on FE setup and most absorption time in coefficient iteration;
its different stopping policy is not authority to weaken native certificates.
Its failed header-path build and interrupted standalone-handshake attempt are
preserved, followed by successful runs with verified thread settings.

Close the initialization experiment. **Next bounded subsystem:** remaining
generic owned-FE certificate geometry and scheduling. A cache must own an
immutable design/current-weight scope, admit construction and solve peaks,
and preserve affordable fallback routes; measure it before acceptance.
Parallel work must preserve deterministic reduction and routing. Retain fresh
residual/span checks and checkpoint frequency. No cache is implemented here.

The archive verifies 2,996 entries, SHA-256
`8c110b6a45f0135f6bd58c28d89eea30dd0a0640a4569b10046ca8248382c1df`.
Static allocation source drift, weighted constrained fits, nonretaining CG
span instability, factor-base provenance and SCC/Linux deferrals remain open.
Frozen 77/16/111 definitions and reserved seeds are untouched. This is a local
implementation checkpoint, not broad goal completion, qualification or release.

## Owned-FE projection RHS arithmetic accepted — 2026-09-22

The [projection milestone](benchmarks/reports/development/owned-fe-projection-local-2026-09-22/README.md)
and ADR 0114 bind contiguous RHS moment rows and reconstruct fitted values in
fixed 32-entry tiles. Per-entry arithmetic order, raw Grams, pivoting, rank,
certificates, automatic routes, DoF, weights and publication are unchanged.
One 256-byte stack tile is added; no heap state or geometry cache persists
across calls or PPML weight frames. The one-partition cache guards stay intact.

`projrow1` is `738ce242` plus the archived `projection.rs` overlay; all 529
captured inputs match. Plugin SHA-256:
`4bec0e08a01eb8f642c87f9398186d1c3345ce1b9ba3b090b78c6c4b9bc5c522`.
Seven eligible slope rows gain 6.4–18.4% by paired medians. Original commands
improve 370→333 and 428→393 ms; fresh K33 improves 1027→901 ms and fresh
three-partition slopes 902→736 ms. AW/PW controls gain 6–7%, and the new weak
native control 8.7%. Disconnected gain is below 1%; low memory and categorical,
PPML and cached-path controls are effectively flat with individual losses.
All six direct stock timing rows still favor fixest. No tile or guard tuning
follows these results.

All 96 headline calls pass without timeout, detected overlap or monitor error.
All 39 native pairs and 52 repeat checks pass unchanged scientific envelopes;
every pair has exact scientific exports and identical tracked heap peaks.
All 18 direct stock headline comparisons pass. Validation also passes 31
distinct Rust tests, two optimized projection tests, actual-plugin ABI/panic
containment, five harness tests and four licensed OLS/PPML/transaction fixtures.

**Independent science remains explicit.** Three current-candidate bounded
pilots pass b/V/sample/RSS/rank checks. A fourth three-partition pilot has exact
FE rank 315 but the registered conservative count 316 on both estimators.
Its exact-rank covariance checks fail; a separate independent channel/graph
count and explicit-dummy covariance using the declared policy pass. Both
records remain archived; no oracle, tolerance or production DoF change is made.
The first positive-DoF weak pilot has native rank 167 and residual DoF 1747,
with stock missing four covariance coordinates. A fresh weak pilot passes,
but the first discrepancy remains and the larger weak timing is native-only.
Prior weak_d3 failures and disconnected stock discrepancies remain open.

Separate diagnostics put projection at 125→108 and 135→98 ms, mainly from
RHS moments; certificates stay about 176 ms. Close this projection experiment.
**Next bounded subsystem:** generic owned-FE fresh certification. Trace
same-call weighted normalization/QR reuse with explicit ownership and memory
admission, and compare scheduling in an isolated pinned fixest diagnostic
before introducing a cache. Keep fresh residual/span checks independent.
Do not tune either tile, chase the disconnected <1% residual, or reopen closed
serial-slope/window work.

The archive verifies 3,581 entries, SHA-256
`f2f1dcf149f93dcd6e368e919619e18f164aa50f44e7c4b76ddf6f6daa7a5b13`.
The static allocation inventory still fails documented source drift; weighted
constrained fits, nonretaining CG span instability, factor-base provenance and
SCC/Linux deferrals remain open. Frozen 77/16/111 definitions and reserved
seeds are untouched. This is a clean local implementation milestone, not broad
goal completion, qualification, push or release.

## Owned-FE raw moments accepted; breadth gaps verified — 2026-09-22

The [generated-term/multi-partition milestone](benchmarks/reports/development/owned-fe-certificate-local-2026-09-22/README.md)
and ADR 0113 batch fresh owned-FE raw diagnostic moments across 32 RHSs.
Per-entry scaling, compensated arithmetic, observation order and overflow
saturation remain exact. Weighted rank-aware QR, relative certificates,
solver routing, weights, DoF and publication rules are unchanged. Three fixed
arrays add 768 bytes of stack payload; no heap or cross-call/PPML cache is added.

`rawtile1` is `cd599498` plus the archived `certificate.rs` overlay; all 529
captured inputs match. Plugin SHA-256:
`016f3fb22bbf2b50f81b5b900a4c90d0e82997507686aefb57e41382cb9ab343`.
Six eligible multi-partition slope rows gain 10.6–19.8% by paired medians.
Two original slope command medians improve 421→362 and 519→428 ms; fresh
K33 improves 1360→1091 ms. AW robust and PW clustered controls gain about
14–15%. Diagnostic raw moments improve 120→55 and 176→76 ms. TS and
categorical/PPML control changes are small and mixed; individual losses remain.

All 42 successful native pairs and 56 repeat checks pass unchanged envelopes.
Thirty-nine native pairs and six saved-output binaries are exact. Three
parallel disconnected pairs show at most 1.01e−16 coefficient and 3.19e−17
covariance differences; both artifacts also vary across their own repeats.
Five independent keyed-TS/explicit-slope pilots, 32 distinct Rust tests, two
optimized tile tests, actual-plugin ABI containment, four harness tests and
four licensed OLS/PPML/transaction fixtures pass.

**Comparison limits stay visible.** Direct stock checks pass 24/28. The three
disconnected slope comparisons disagree in coefficients, V and RSS and remain
unranked; this is not just a DoF rescaling question. A weak multi-slope design
fails native warmup with r(459) on both artifacts and stock reports nonconvergence
and more parameters than observations. One further baseline attempt was
interrupted and remaining weak repetitions stopped; the other fresh cases
completed unchanged in a separate root. No native error localization or
independent rank adjudication is claimed. Seven of eight comparable stock rows
still favor fixest. Wide TS launches originally exceeded Stata's argument buffer;
captured per-run includes fix the harness, and the corrected comparisons pass.

The acceptance blocks retain 114 completed attempts (111 successes, one stock
nonconvergence, two native warmup failures), plus one interrupted attempt. The
initial screen's three startup crashes remain additional visible failures.
There are no completed-call timeouts, detected overlaps or monitor errors.
Native tracked peaks are unchanged outside parallel disconnected variation
(−19.16 to +22.28 MB); there is no tighter whole-action memory claim. Static
allocation source drift, weighted constrained fits, nonretaining CG span
instability, factor-base provenance and SCC/Linux deferrals remain open.

**Next bounded task:** investigate the generic owned-FE projection/certificate
lifecycle and reusable local geometry, where strong/unbalanced multi-partition
slopes still lose substantially. Establish a positive-residual-DoF weak-design
pilot first and keep the disconnected stock discrepancy separate. Close this
raw-tile experiment; do not tune its width, chase narrow TS noise, or reopen
closed serial-slope/window work. This is a clean local implementation milestone,
not completion of the broad goal, qualification, push or release.

## Scratch influence rows accepted; low-memory tuning bounded — 2026-09-22

The [scratch robust covariance milestone](benchmarks/reports/development/low-covariance-local-2026-09-22/README.md)
and ADR 0112 reuse the accepted compensated row helper for production influence
outer products. Scalar compensated bread multiplication, HC1/weight placement,
per-coordinate observation order and independent certificates are unchanged.
The helper moves to the shared sandwich module with an internal OLS re-export;
there is no new buffer, cache, worker team, reduction tree or admission rule.

`covrow1` is `fc2b0fc4` plus the archived `sandwich.rs` and
`prepared_unadjusted.rs` overlays; all 529 captured inputs match. Plugin SHA-256:
`6f4ad854b739f5fd694f51f98f507dcc0e865d96ec4486564e6314dea0b07162`.
Wide command medians improve 1046→945 ms (9.7% paired), fresh K40 1305→1155 ms
(11.6%), and fresh unbalanced factors 523→480 ms (8.5%). Narrow gains are small
and three-FE results essentially flat. Eleven of twelve comparable stock rows
still lose; ordinary stock memory execution is not an equal-budget comparison.

All 155 headline calls pass without timeout, detected overlap or monitor error.
All 58 native pairs, 80 native repeat checks and three saved-output binary pairs
are exact. Five independent dummy pilots, 31 distinct Rust tests, four optimized
serial tests, actual-plugin ABI containment and three licensed OLS oracle/
transaction fixtures pass. The 165→71 ms wide outer-product phase explains
the gain; compensated bread remains about 188 ms in the diagnostic.

**Losses remain visible.** Original two-FE +31 ms, factor +6 ms and fast-control
+7 ms calls are retained. A declared five-pair confirmation on identical two-FE
data does not reproduce the large loss (maximum +1 ms); the additional fast
control has equal 86 ms medians, with individual losses up to +2 ms. No candidate
edit or case-specific guard followed the measurements. Direct stock checks pass
36/39; disconnected three-FE covariance has native residual DoF 48079 versus
stock 46560 and remains unranked. Diagnostic rescaling is not rank adjudication.

Tracked native peaks are unchanged outside parallel multi-FE variation
(−1.60 to +0.50 MB). Four existing fallible covariance workspaces remain;
there is no tighter whole-action or constrained-budget memory claim. Static
allocation audit still fails source drift and was not regenerated. Weighted
constrained fits, nonretaining CG span instability and Linux/SCC deferrals
remain open.

**Next bounded task:** return to breadth: screen generated time-series terms
and multi-partition slopes on current source, including sample/time gaps,
width/thread/output controls and independent explicit-design oracles. Select
another implementation from measured gaps. Do not continue tuning narrow
covariance cases or reopen closed serial-slope/window experiments. Factor-base
provenance, prior comparator discrepancies and broader topology/memory coverage
remain open. This is a clean local development milestone, not completion of
the broad goal, qualification, push or release.

## Guarded scratch finite solve accepted — 2026-09-22

The [low-memory finite milestone](benchmarks/reports/development/low-memory-cholesky-local-2026-09-22/README.md)
and ADR 0111 reuse the existing full-rank guarded Cholesky trial on compensated
scratch moments. Weak/deficient or uncertified trials retain unchanged QR
omission decisions. Scratch and response remain untouched by the trial; fresh
original-data residual reconstruction, FE projection, score, bread and inference
certificates still run. No dense within matrix or PPML generation cache is added.

`lowchol2` is `f8c2a0d6` plus the archived `prepared_fit.rs`/`memory.rs` overlay;
all 529 captured inputs match. Plugin SHA-256:
`45e2c724d80ee38e45d935b5c0d658c45f3d9c144b8f888041fd11a4d0b3b1da`.
Fifteen eligible low-memory rows gain 8.3–38.8% by paired medians. Wide robust
improves 1661→1046 ms; fresh K40 improves 2126→1304 ms. Factors, serial,
one-/two-/three-FE, IID, weighted robust/clustered and saved-output cases benefit.
The fast control has exact scientific exports and ratios .947–1.000; no new
fast-path algorithm claim follows. Stock still wins eleven of twelve comparable
rows; ordinary stock memory strategy is not an equal-budget comparison.

All 135 headline calls pass without timeout, detected overlap or monitor error.
All 48 native science pairs, repeats and three saved-output comparisons pass
unchanged envelopes. Status/DoF/subset/FE metadata are identical; low numerical
exports and condition proxies can change. Five independent dummy pilots,
twenty distinct Rust tests (plus optimized direct trial), actual-plugin ABI
containment, three licensed OLS fixtures and nine combinations in the additional
PPML low-categorical fixture pass. Controller/packaging failures and corrected
sentinel/environment handling remain archived, not silently discarded.

The existing internal finite memory allowance covers at most five square banks
and linear trial storage. No row-sized buffer, scratch size or admission policy
changes. Independent allocator-frame checks pass. Whole-action peak differences
remain zero outside parallel multi-FE variation (−3.54 to +2.22 MB); no tighter
whole-action or constrained-budget guarantee follows. Static allocation audit
still fails source drift; no successor inventory was generated.

**Comparator boundary:** fresh disconnected three-FE native residual DoF 48104
versus stock 46560 leaves all three stock covariance comparisons failed and
unranked. Diagnostic rescaling agrees within 2.19e−13, without independent
full-size rank adjudication. Direct stock checks pass 36/39.

**Next bounded performance task:** profile current-source production covariance
and independent checks. The wide finite phase is now 299 ms (282 ms moments),
while the rest of the command still costs roughly 0.74 seconds. Confirm a large
avoidable cost before changing another kernel. Factor-base provenance, generated
TS, multi-partition slopes, broader topology/memory coverage, earlier scientific
discrepancies, constrained-fit/CG failures and SCC deferrals remain open. Do not
reopen closed window/serial-slope experiments. This is a clean local development
milestone, not completion of the broad goal, qualification, push or release.

## Compensated scratch certificate rows accepted — 2026-09-22

The [scratch-certificate milestone](benchmarks/reports/development/low-memory-gram-local-2026-09-22/README.md)
and ADR 0110 group independent compensated matrix updates into contiguous rows.
The fresh scratch score Gram and robust covariance certificate retain each
coordinate's products and observation order, full matrices and separate fresh
traversals. Every nonfinite product/sum/correction still fails before publication.
Production influence accumulation, rank/DoF/weights, dense kernels, PPML iteration
state, memory admission and allocation ownership are unchanged.

`gramrow2` is `38935943` plus the archived three-file overlay; all 529 captured
inputs match. Plugin SHA-256:
`812326ed980df16ff9b60df3e21359971fad4437a0fe25fb02c9645d8f501b6f`.
Eligible low-memory paired median gains span 5.5–21.4%. Wide robust improves
2088→1662 ms and fresh K40 improves 2680→2106 ms. AW/FW robust gain about 14%;
PW clustered gains 5.5%. Fast/IID control paired medians are 1.00, with a +2 ms
individual fast loss preserved. Eleven of twelve comparable stock rows still
lose. Ordinary stock memory strategy is not an equal-budget comparison.

All 135 headline attempts pass without detected overlap, timeout or monitor
error. All 48 native scientific pairs and three saved-output binary pairs are
identical. Direct candidate/stock checks pass 36/39. Five small pilot cases pass
independent full-coefficient/covariance/sample/RSS checks on both artifacts.
Twenty-seven distinct Rust tests, including optimized new bit/error tests,
actual-plugin ABI containment and three licensed independent oracle/transaction
fixtures pass. Original test-draft and incomplete-build failures remain archived;
neither reached a timed fit. The 4,163-entry archive verifies every hash.

**Comparator boundary:** the disconnected sparse three-FE stock covariance
fails with native residual DoF 48061 versus stock 46560. Diagnostic rescaling
by their ratio agrees within 3.06e−13, but does not independently adjudicate
rank or alter the original comparison. This stock timing remains unranked.
Tracked peaks are unchanged outside parallel multi-FE variation (−7.21 to
+6.91 MB); the implementation adds no buffers, and no tighter memory guarantee
or constrained-budget claim follows.

**Next bounded performance task:** profile low-memory finite fitting in detail.
It still costs roughly one second of the 1.66-second wide command. Separate
moment construction, QR updates and reconstruction before changing the solver
or transport. Production covariance remains another 366 ms and must preserve
its influence arithmetic. Do not reopen closed window/serial-slope experiments.
Factor-base provenance, generated TS, multi-partition slopes, broader topology/
memory coverage, earlier comparator discrepancies, static allocation drift,
constrained-fit/CG failures and SCC deferrals remain open. This is a clean local
development milestone, not completion of the broad goal or qualification.

## Low-memory QR norm reuse accepted — 2026-09-22

The [low-memory milestone](benchmarks/reports/development/low-memory-qr-local-2026-09-22/README.md)
and ADR 0109 remove redundant scratch reads before finite-QR pivot selection.
Each next norm uses the just-written resident target and the same stable norm
arithmetic. Initial/final independent scratch scans, rank policy and all fresh
residual/inference certificates remain. No allocation, memory admission or
cross-fit/weight-generation cache changes.

`qrnorm1` is `f69cf21f` plus the archived `prepared_fit.rs` overlay; all 529
captured inputs match. Plugin SHA-256:
`f106400d4ed2561fcde5eedb367bcbe384e0c768aa563cc4df6840f09eb8846a`.
Eleven low-memory rows improve by paired medians of 1.8–4.5%, including fresh
serial, two-FE, factor, wide and saved-output cases. Wide robust changes
2169→2081 ms; fresh K40 changes 2765→2687 ms. Two fast controls are flat and
2 ms slower, respectively, with individual losses retained. All eleven
low-memory medians still lose to ordinary stock fixest; this is not an
equal-memory-budget comparison.

All 231 headline attempts pass with no detected overlap, timeout or monitor
error. All 65 native scientific pairs and ten native saved-output binary pairs
are identical; all 65 direct candidate/stock checks pass. Four independent
small designs pass on both artifacts. Fifty-one distinct Rust tests, actual
plugin ABI containment and three licensed oracle/transaction fixtures pass.
Two generator refusals were corrected before fitting and remain archived.
The 6,184-entry evidence archive verifies every hash. Native tracked peaks are
unchanged outside threaded three-FE variation (−1.24 to +0.87 MB); no new storage
is introduced and no constrained-memory claim follows.

**Factor-rebasing defect remains open.** The new diagnostic proves candidate
metadata for `i.f` and `ib1.f` is identical but final-sample expansion differs
after level 1 disappears. Ordinary default/explicit-other/no-base forms can
return r(459); failed calls preserve prior results and output absence. A safe
general repair needs original base/level-selection provenance in the model
wire. No blind minimum-base rewrite or compatibility change was adopted.

**Next bounded performance task:** scratch-backed compensated Gram/covariance
loops. The wide trace spends 310 ms in the fresh score Gram certificate,
368 ms in covariance and 339 ms in its independent certificate; score copying
costs only 7 ms. Preserve per-coordinate arithmetic, fresh traversals and
influence accumulation order. Do not spend another cycle tuning norm reads.
Low-memory topology/weighted timing, generated TS, multi-partition slope and
constrained-memory coverage remains incomplete. Earlier comparator discrepancies,
static allocation drift, constrained-fit/CG failures and SCC deferrals remain
open. This is a local development checkpoint, not completion of the broad goal,
qualification, push or release.

## Generated OLS name reuse accepted — 2026-09-22

The [generated-surface milestone](benchmarks/reports/development/generated-surface-local-2026-09-22/README.md)
and ADR 0108 reuse parser factor names when the validated native receipt proves
candidate and retained physical-row counts are equal. Any row deletion keeps
the existing final-sample expansion. This ado-only change removes a measured
48 ms repeated expansion from the wide interaction case; no native arithmetic,
cache, memory admission, rank policy or publication validation changes.

`factor_names1` is `56e4ff79`/`batch1` plus the archived
`stata/_fereg_estimate_ols.ado` overlay. All 527 captured inputs match; the new
fixture is separately executed and hash-bound. The reused plugin remains
`a191585c5a156809f8a572bbce37d948b2ede1e0ca8579a85f15e4cb32547d14`.
The accepted ado hash is
`407ae24785bc028aef1a3d5b808f0bb210d27017c801546b5bcde835883abc62`.
Wide interactions improve 341→288 ms (15.5% paired) and fresh 480→421 ms
(12.1% paired). Narrow factor/output paired gains are about 3–8%; numeric
controls are flat, with individual losses preserved. Six of eleven comparable
stock rows still lose. A fresh three-FE beta/RSS discrepancy is unranked.

All 144 headline calls pass application checks without detected overlap.
All 36 native pairs have byte-identical scientific exports; direct candidate/
stock checks pass 33/36, with the three failures above retained. Six supported
small cases pass independent full-coefficient/covariance/sample/RSS oracles,
including normalized requested outputs. Three licensed fixtures pass, including
the new ten-scenario dummy-OLS/weight/sample-boundary fixture, established
categorical rebasing and transaction checks. No native rebuild or Rust test
rerun was needed. The 4,324-entry evidence archive verifies every hash.

**Newly exposed baseline defect:** ordinary `absorb(g)` returns r(459),
coefficient-name count disagreement, when missing outcomes remove an entire
factor level. Both baseline and candidate reproduce it; categorical tuple
rebasing passes. Original failing fixtures/log hashes are preserved. An invalid
zero-FW fixture draft correctly returned r(401) and was corrected to missing
FW; the failure remains documented. Continuous-product RHS r(498) is an
intentional unsupported case, not a performance result.

**Recommended next bounded task:** localize and repair the ordinary factor-level
rebasing defect before further factor tuning. Two-/three-FE solver costs remain
larger independent opportunities; closed RHS-window experiments stay closed.
Current low-memory, generated TS, multi-partition slope and broad saved-output
performance coverage remains incomplete. Native tracked peaks are unchanged
outside threaded three-FE variation (−2.89 to +2.97 MB). Static allocation drift,
constrained-fit issues, comparator discrepancies and SCC deferrals below remain
open. No qualification, historical-win transfer, SCC retry, push or release.

## Three-FE scheduling experiment closed; native checkpoint retained — 2026-09-22

The [three-FE window screen](benchmarks/reports/development/three-fe-window-local-2026-09-22/README.md)
remeasures `37af7d82`/`batch1` against stock and rejects an isolated generic
threaded-window experiment under its predeclared mixed-gain stop rule. Dense
K50/T16 improves only 4.3% by paired median; dense K31/T4 loses 1.1%. Sparse
K31/T8 improves 8.6% natively, but its stock beta/V discrepancy stays unranked.
The current production dense medians still lose to stock: 765/605 and 624/568 ms
in the paired block. All 45 baseline/screen attempts complete cleanly; all nine
native pairs are byte-identical scientifically. Tracked peaks fall 2.54–161.44 MB
in the experiment, without a constrained-memory acceptance claim.

No production native source or memory policy changed. The isolated instrumenter
and build option are retained solely to reproduce the rejected experiment.
Five focused Rust transport tests and ABI containment pass. A new independent
three-FE explicit-LSMR oracle passes on production and experiment for unit/AW,
robust inference and T1/T4/T16. The 1,100-entry evidence archive verifies all
hashes and preserves the unrun fresh confirmation declarations. Do not restart
window-size tuning or fit a guard to the favorable cells.

**Recommended next subsystem:** generated/factor-term command cost and requested
outputs, with a small matched current-source screen first. Their performance
coverage remains incomplete. Three-FE upstream operator/preconditioner work
needs a separate algorithmic investigation; the bounded transport/scheduling
experiments are closed. The accepted narrow PPML native checkpoint and all
deferred issues below remain unchanged. No qualification, stopped campaign,
push, release or SCC authentication retry was performed.

## Narrow PPML checkpoints accepted — 2026-09-22

The [narrow-batch milestone](benchmarks/reports/development/ppml-narrow-batch-local-2026-09-22/README.md)
and ADR 0107 extend admitted two-FE checkpoint Schur to 2–15 actual RHSs.
Tiny batches probe all available columns; OLS retains its original selection.
Current weights and numerical state are freshly rebuilt, and initial/residual,
score and final-Hessian certificates remain mandatory. There is no persistent
cache or new memory bank. `batch1` is `af38bce7` plus the archived borrowed-map
patch and new oracle fixture; all 527 captured inputs match the working source.
Plugin SHA-256:
`a191585c5a156809f8a572bbce37d948b2ede1e0ca8579a85f15e4cb32547d14`.

Selected dense/weak/disconnected/unbalanced PPML paired gains are about 10–34%;
serial and PW/cluster controls gain 37% and 19%. Screen degree-six improves
296→232 ms, dense K4 228→197 ms. Fresh sparse loses about 1.4%; excluded paths
are mostly flat with small losses retained. Seven of nine pairwise-comparable
rows still lose to stock fixest. All 129 headline attempts, 51 native scientific
pairs and 27 direct candidate/stock checks pass; 18 native pairs are byte-identical.
Max beta/V differences are 2.10e−16/1.01e−17. Validation passes 391 distinct Rust
tests, actual-plugin ABI containment and four licensed fixtures. The new cutoff
fixture proves actual narrow admissions and independent dummy-Poisson/Hessian
sandwich agreement for K1/2/6/12, unit/FW/PW, T1/T4 and saved means.

Tracked peak differences span −15.92 to +4.93 MB, with unchanged admission;
sampled RSS and its limitations are recorded. Static allocation-source drift
still fails and was not regenerated. The 4,367-entry evidence archive verifies
all hashes; an initial archive failure on Finder metadata is recorded separately.

This bounded narrow-checkpoint experiment is accepted and closed. The later
three-FE screen above supersedes its next step; do not chase the small sparse
probe loss or tune around a near-null tiny-batch RHS. Generated/factor,
broad saved-output and constrained-memory performance coverage remains
incomplete. All deferred issues below stay open; SCC authentication must not
be retried without renewed access. No qualification, historical win transfer,
stopped campaign, push or release is claimed.

## Single-residual PPML checkpoint accepted — 2026-09-22

The [single-residual milestone](benchmarks/reports/development/ppml-single-residual-local-2026-09-22/README.md)
and ADR 0106 extend the existing current-weight Schur probe to one actual RHS
on admitted two-FE checkpoint-LSMR sessions. Fresh initial/residual certificates,
bounded attempts, untouched-input fallback and final-Hessian checks remain.
No persistent buffer or changing-weight cache is added. The 2–15 RHS route
stays closed. `residual2` is `55e9c48f` plus the archived borrowed-map patch;
all 525 captured inputs match the final source, and the new public oracle
fixture has a separate executed-file hash. Plugin SHA-256:
`44090d1ab6db8a6a9afea8640c28e440de1eb4562010308bd22e3d604800c792`.

Degree-six two-FE screen/fresh medians improve 324→292 and 346→287 ms; paired
gains are about 10% and 17%. The PW/cluster control gains about 4%; dense,
weak and excluded controls are mostly flat. Sparse two-FE has a small measured
loss. Four of five pairwise-comparable rows still lose to stock fixest.
All 93 headline attempts complete without detected overlap/timeouts; all 39
native pairs pass science, 36 byte-identically. Other beta/V differences are
at most 1.00e−16/7.62e−21. The final source passes 378 distinct Rust tests,
ABI containment and five licensed fixtures. The new explicit-LSMR fixture
proves six single-RHS Schur acceptances and checks unit/FW/PW, T1/T4, saved
means and current-Hessian sandwiches against independent dummy Poisson.
Tracked peak differences span −5.14 to +0.21 MB, with unchanged admission.

This local continuation is complete and its experiments are closed. It adds
the publication repair, fresh means and single-residual checkpoint improvements
to the earlier transport/reference work. No universal win, qualification,
historical aggregate transfer, push or release is claimed. All deferred issues
below remain open, including static allocation-source drift, and serial-slope
tuning remains closed.

The later narrow-batch milestone above supersedes this recommendation:
narrow multi-RHS PPML canonical checkpoints.
The degree-six diagnostic still spends about 38 ms on the initial 12-RHS frame
and 45 ms on the strict 11-RHS frame. Study current-weight batch geometry and
assembly separately before considering their guard; do not reopen the accepted
single-RHS experiment. Three-FE upstream solves remain a separate algorithmic
opportunity. Large disconnected PPML/sparse three-FE OLS reference discrepancies
remain unranked, and broader generated/output/constrained-memory coverage is
incomplete. SCC authentication must not be retried without renewed access.

## Fresh PPML means accepted locally — 2026-09-22

The [fresh-means milestone](benchmarks/reports/development/ppml-fresh-means-local-2026-09-22/README.md)
and ADR 0105 bind immutable numeric sources during uncached reported-intercept
assembly and schedule independent fresh weighted means. There is no weighted
cache, new heap buffer, matrix owner or changed certificate. `means1` is
`346d80e4` plus the archived `ols/prepared_fit.rs` patch; all 525 captured inputs
match the working source. Plugin SHA-256:
`52c3996f5936486cb423f5c904f7275d42504d76735e436370f063af6eae5e9e`.

One-FE screen/fresh medians improve 163→144 and 441→388 ms; wider two-FE improves
491→440 ms. Three-FE is flat, K65/slope controls have small losses, and all six
stock-comparable PPML rows still lose to fixest. Disconnected large PPML stock
coefficients exceed the frozen tolerance and remain unranked. All 111 headline
attempts complete; 45 native pairs have byte-identical scientific exports.
The artifact passes 382 distinct focused Rust tests, ABI validation and four
licensed fixtures. Fresh explicit-design checks pass after independently
pruning and verifying singleton sample IDs; the original oracle sample mismatch
and setup failure remain archived. Tracked peak differences span −0.71 to
+3.79 MB; the static allocation inventory still fails and was not regenerated.

This mean experiment is closed. The later single-residual milestone above
supersedes its next-step recommendation. All deferred items below remain open,
and serial-slope tuning stays closed.

## PPML simplex/IR publication repaired locally — 2026-09-22

The [publication repair](benchmarks/reports/development/ppml-simplex-publication-local-2026-09-22/README.md)
and ADR 0104 resolve the zero-outcome r(3499) discovered at `cc6e40fa`.
Stata had rejected a truthful false certificate flag for a memory-skipped
simplex attempt despite later certified IR on the same generation. The
validator now checks the accepted status/closure semantics; native detectors,
sample decisions, weights and certificates are unchanged. Unresolved simplex
without terminal certified IR and simplex-only over-budget requests still
fail closed.

`separation1` reuses the exact `reference1` plugin, with the archived ado/test
overlay on `cc6e40fa`. Two licensed fixtures and 21 focused Rust tests pass.
The new public fixture covers independent dummy Poisson, current-Hessian V,
positive-interior rank, unit/FW/PW, T1/T4 and atomic simplex-only failure. The
original benchmark now matches stock on sample/beta/V/objective in all three
repetitions; a fresh-seed small case independently proves exactly 50 zero-only
FE drops and passes explicit-design inference. Original-artifact failures,
two failed setup attempts and all distributions remain archived.

The repaired zero target takes 115 ms versus stock 37 ms; the positive control
is 146→147 ms with identical scientific exports and native tracked peak.
No speedup is claimed for this correctness repair. The later assembly and
residual milestones above supersede its next-step recommendation.

## Broader optimization: transport and PPML refresh accepted locally — 2026-09-22

The [current-source inventory](benchmarks/reports/development/next-subsystem-inventory-local-2026-09-22/README.md)
binds all 523 computational inputs at `7189e57a` to `dtk1` and records 57
application attempts. Dense three-FE OLS loses to stock fixest by 28% at K10/T4
and 54% at K50/T16. Public PPML one-FE and degree-six two-FE take 3.43×/3.56×
the comparable stock command time. These cells pass pairwise checks and their
small independent-oracle counterparts pass. Current sparse three-FE reference
discrepancies remain unranked. One explicit-LSMR diagnostic fails its warmup
with r(459); all other calls complete without detected competing fits/builds.
This is bounded development evidence, not PPML/F0 or release qualification.

The [transport milestone](benchmarks/reports/development/lsmr-transport-local-2026-09-22/README.md)
at `87b21d4c` and ADR 0102 retain tiled input/output copies in owned and borrowed LSMR,
with existing buffers and unchanged numerical policy. All 150 application
calls complete; 55 native pairs pass science, four new small-design oracles
pass for both artifacts, 76 focused Rust tests and three licensed fixtures
pass. Threaded wide three-FE gains are about 4–8% by medians, with little
change in narrow, serial, four-FE or PPML controls. The source adds no matrix
bank; threaded upstream allocation peaks fluctuate, including positive
paired differences, so exact peak neutrality is not claimed.

The [PPML refresh milestone](benchmarks/reports/development/ppml-reference-refresh-local-2026-09-22/README.md)
and ADR 0103 retain numeric source binding and independent-column scheduling
for fresh current-weight reference statistics. `reference1` is `87b21d4c` plus
the archived `ols/linear_session.rs` patch; all 523 computational inputs match
`cc6e40fa`. Its plugin SHA-256 is
`357118cece7f4eaf1813e5e7ab25458ca999ae20fb0bed8afc7dd4d6354d58ea`.
One-FE screen/fresh confirmation medians improve 179→162 and 158→143 ms;
sparse two-FE improves 359→347 ms. Other gains are small and the narrow control
loses about 1.7%. Six of seven successful stock-comparable PPML rows still lose
to fixest. No new heap buffer, weighted cache, rank policy or certificate
relaxation is introduced. Threaded PPML tracked peaks vary by −2.51 to +3.49 MB;
no exact peak-neutrality claim is made.

All 39 completed native pairs have identical scientific exports. Of 108
application attempts, 102 complete and six native zero-outcome warmups fail;
there are no timeouts or detected overlaps. The final source passes 375
distinct focused Rust tests, the actual-plugin ABI harness and four licensed
fixtures. The new fixture uses independent dummy Poisson and current-Hessian
sandwiches across one/two/three FE, unit/FW/PW, offsets, robust/clustered
inference, saved means and T1/T4. Full distributions, failures, exact source
identities and immutable receipts are archived. No qualification campaign,
push or release was performed.

The zero-outcome failure recorded in that exact-artifact report predates both
optimizations and is repaired by the newer publication milestone above. The
historical failed receipts remain unchanged. Strict checkpoint residual
projection and assembly remain the subsequent performance opportunity.

Both bounded performance experiments are closed. Diagnostic instrumentation
remains isolated. Historical serial-slope tuning remains closed; the 77
primary, 16 holdout and 111 native controls and reserved validation seeds stay
unchanged. The allocation-source inventory still fails and was not refreshed;
weighted constrained-fit failures, nonretaining CG instability and deferred
Linux/constrained-memory confirmation remain open. Do not restart stopped
campaigns or repeat the known SCC authentication failure.

## OLS optimization round closed locally — 2026-09-22

Johannes requested finishing this round and moving to other optimization work,
even with remaining serial-slope losses. The
[closeout report](benchmarks/reports/development/ols-round-closeout-local-2026-09-22/README.md)
and [ADR 0101](docs/adr/0101-ols-optimization-round-closeout.md) retain the six
remaining graph/residual/window files as a local development checkpoint.
All 523 computational inputs match the previously measured `dtk1` source;
the closeout introduces no additional numerical change. Seventy distinct
release-mode Rust tests pass in 71 executions. Three licensed Stata/MP 19
fixtures pass with explicit return codes and terminal sentinels, covering
weighted dummy inference, frequency expansion, saved outputs and multiple slopes.
Rejected experiments remain reverted. No push, installation or release is
authorized or performed.

The original baseline is `0efa78bd`. Historical primary/holdout, control,
graph-coverage and cycle-validation evidence below remains exact-artifact
evidence. The original all-domain goal is not achieved: serial AW/PW slope
losses remain, Linux confirmation is deferred pending SCC access, and cycle
routing is not adopted. The existing static allocation-source audit still
fails on frozen-inventory drift; the closeout preserves that failure and does
not refresh the inventory. A reviewed successor remains qualification work.

Resume with a different subsystem after Johannes selects it. Do not launch
another serial-slope experiment or restart the original broad campaign merely
because this historical plan remains here. Preserve the 77 primary, sixteen
holdout and 111 native-control definitions. Seeds 202609211 and 202609212 are
reserved to the completed frozen cy2 validation and must not be used for tuning.

### Committed checkpoints

- The [baseline](benchmarks/reports/development/slopes-baseline-local-2026-09-21/README.md)
  reproduced threaded K10/K50 slope times of 336/854 ms versus fixest 54/193 ms.
  Cached rank-aware geometry and fresh certificates are committed at `d2ec66e3`
  under ADR 0087. Direct recovery and fresh parallel certificates are committed
  at `12de1adb` under ADR 0088. The [recovery report](benchmarks/reports/development/slopes-recovery-local-2026-09-21/README.md)
  gives K10/T1, K10/T16, K50/T1, K50/T16 medians of 116/51/397/141 ms versus
  fixest 102/58/684/184 ms. Serial K10 remains a loss.
- `987e8336` commits the [slope-only loader](benchmarks/reports/development/slopes-loader-local-2026-09-21/README.md)
  and [narrow ordered-cluster reduction](benchmarks/reports/development/narrow-cluster-local-2026-09-21/README.md)
  under ADRs 0089/0090. The constrained slope check admits low/auto and rejects
  fast at 90 MB usable, with matching numerical results. OLS v38/v39 and action
  v23/v24 allocation inventories preserve their remaining proof gaps.
- The [full small graph matrix](benchmarks/reports/development/two-fe-full-science-local-2026-09-21/README.md)
  is committed at `ffa3bde1`: 1,518 cells, 4,158 completed calls and 3,036 passing
  native explicit-dummy/literal-FW checks. All paired native samples and outputs
  are exact. The 766 default-fixest discrepancies remain failures. Tighter
  diagnostics leave six sparse reference discrepancies; below-floor rejections
  and all failed generation/checker attempts remain recorded.
- `fa71f94e` archives the [partial original large screen](benchmarks/reports/development/two-fe-large-screen-partial-local-2026-09-21/README.md):
  943 records, 114 complete cells, one baseline unit-ring saved-output timeout
  and a deliberately interrupted next call. This recovery2 screen counts 82
  wins and four losses/ties, plus 28 FW controls. All 114 native inference and
  168 candidate output audits pass. Remaining cells are open. Never resume the
  old `target/g21l` root under a changed controller.

### Current graph and slope drafts

The [finishing-experiment archive](benchmarks/reports/development/two-fe-finishing-experiments-local-2026-09-21/README.md)
preserves 663 calls, twelve native artifact identities, source overlays,
independent audits and failed experiments. It is development evidence, not
acceptance of the uncommitted kernels or completion of this milestone.

`m5` plugin `a07732eea15a47a185903bf7403cd74f111a5eebbd5e558fbb371f02728bef56`
contains retained Schur-fallback products, actual-solver follow-through, fewer
raw-residual passes, guarded weighted overlap, stable CMG correction products
and final same-weight graph reuse. Shared callers remain excluded. All focused
known-solution, weight/thread/output, underflow and budget tests pass. Earlier
failures remain recorded. The exposed explicit/insufficient-budget nonretaining
CG span instability is unresolved; this is not a general CG repair.

The 32-cell ring/cycle K6/K50 unit/AW/PW/FW three-pair screen at `target/g21c5`
completes all calls. All 24 comparable cells beat default fixest, with eight FW
controls; 64 native inference checks, 192 exact repeats and 48 output audits
pass. All 24 default-reference oracle discrepancies remain. The unit-ring
robust m5/m4 paired regression of 8.8% still requires a direct repeat. The
preceding narrow screen gives degree-six unit/AW/PW full-output medians of
107/116/124 ms versus fixest 132/157/162; shuffled AW K6 is 262 versus 295 ms.
These are screens, not final-block acceptance.

The compact slope Gram draft stores only actual channel-square matrices and
preserves FMA/elimination/QR checks. Its focused tests and 54 application calls
pass. `sg1` medians are 115/51/400/141 ms for K10/T1, K10/T16, K50/T1, K50/T16
versus m5 122/52/411/144 and fixest 93/56/692/197. PW three-slope-only robust/all
is 97 versus 95/119; clustered/all is 113 versus 113/133. Source-formatting
scope is recorded separately. The earlier `ss1` indexing experiment was
rejected and reverted; retain its patch and evidence.

### Routing and memory findings

`target/g21rd1` completes 72 whole-command attempts across six solver families:
52 pass science, 20 fail at the application boundary, and all twelve automatic
calls pass science. The first dense-unit automatic call has a possible overlap
and cannot rank routes. Weighted-dense explicit CG returns r(459) on baseline
and m5. Unit ring, unit cycle and AW cycle auto/fastest-valid ratios are
1.209/1.241/1.449 in the one-repeat exploration. Full solver reports distinguish
Schur-CG from explicit symmetric CG.

`cap40` changes only the captured narrow Schur continuation allowance from 64
to 40. Sixteen focused Schur tests pass. The fourteen-cell three-pair screen at
`target/g21cap1` passes all 28 native inference checks, 84 exact repeats and 21
output audits. Three weak cells have possible external-process overlaps. Clean
unit-ring robust/full-output paired ratios versus m5 are 0.937/0.976; dense
changes little. The production cap remains 64. Broader routing and holdout
validation remain open.

The [expanded ring/cycle auto/LSMR comparison](benchmarks/reports/development/ring-cycle-route-calibration-local-2026-09-21/README.md)
(`target/g21r16`, `g21r8`, `g21r1`)
completes 288 attempts and 288 independent scientific passes across K6/K50,
unit/AW, robust/no outputs and clustered/full outputs. T16 cycle/AW/K6 robust
auto repeat one has a possible overlap and is excluded. All T8/T1 timings have
no detected overlap. Maximum paired auto/fastest-valid ratios are 1.617 at T16,
1.889 at T8 and 2.096 at T1. Some robust-only T16/T8 cells favor auto, so a
blanket LSMR switch is unsupported. Auditor sources and whole-command reports
are preserved under `target/ols-next-20260921/ring-cycle-route-*`; both fresh
validation seeds are still unused. The portable archive verifies 9,742 stored
entries, 144 omitted local arrays and complete m5 source reconstruction.

The new `cb1` draft batches public two-FE automatic CMG subtraction across
rows for N>=16384, at least three RHS columns and at least eight workers.
It checks all residuals before mutation and adds no data bank. Explicit/shared,
higher-dimensional and smaller/serial calls retain the old subtraction.
Plugin `206fa4d9769dff2e31a57f3e4d1b453e0a68cc94c4b5039747cccfeef4c3d76f`
built and passed the ABI fixture; its bitwise/finite-rejection test passes.
The ten-cell screen at `target/g21cb1` completes 93 calls, twenty native inference
checks, sixty exact within-arm repeats and twenty-four output audits. All thirty
native pairs have bit-identical numerical exports. Two unit-ring robust peaks
are lower in cb1; the remaining twenty-eight pairs have equal peaks. The extra
m4 independent oracle also passes. Clean wide AW ring T16/T8 and cycle
T8 paired cb1/m5 ratios are 0.9383/0.9493/0.9459. Four possible-overlap cells
remain excluded. Their first repeat at `target/g21cb2` completes 39 calls, eight
native inference checks and six output audits. The clean unit-ring robust
m5/m4 paired ratio is 0.9900, resolving the earlier 8.8% regression on this
repeat. The remaining sparse and wide-cycle timing exclusions need another run.

The timing watcher could mistake a newly started own worker for a competitor
between its descendant snapshot and process scan. A live-ancestry check now
excludes verified own workers while preserving exclusions when ancestry cannot
be read. Four focused monitor tests pass. Earlier warning records are unchanged;
fresh measurements, not retrospective reclassification, will resolve them.
The monitor repair and its four tests are committed locally at `18b4cd83`.
The two remaining cb1 cells at `target/g21cb3` finish all eighteen calls
without detected overlap. Four native inference checks, twelve exact repeats,
three output audits and six exact cross-artifact numerical pairs pass. The
wide AW cycle paired ratio is 0.9389. All ten cb1 cells now have clean timing
evidence from the original screen or explicit repeats; no old record changed.

A single m5 T1/unit/ring/K50/full-output phase trace passes application and
terminal checks and localizes 2.826 seconds to saved-effect recovery after a
4.867-second initial projection. The `wr1` draft preserves requested wide Schur
original-input fallback products, using the existing LSMR admission and retained
parent budget. Successful Schur and final one-RHS reuse still omit products;
no new graph selection rule is introduced. Plugin
`00aa53f474c961e9f2438bfd7b41a5ddd76f9b05f4353999d51c48d61ce9a12d`
built and passed the ABI fixture. Its expanded weight/thread/oracle fixture
passes twelve retaining forced-cap combinations plus the existing successful-
Schur, order, iteration and budget checks. Both narrow Schur fixtures pass.
The eight-cell screen at `target/g21wr1` finishes 72 calls with clean isolation,
sixteen native inference checks, forty-eight exact repeats and twenty-four
output audits. T1/unit/ring/K50 full-output medians are 8.555 versus 5.701 seconds
(paired ratio 0.6651), with default fixest 88.344 seconds. T8 is neutral and
T16 regresses 6.1%; unrestricted wide retention is rejected. A separate wr1
trace reduces recovery from m5's 2.826 seconds to 0.00555 seconds. The serial
allocation peak rises by 34,930,744 bytes; threaded peaks vary substantially.

The `wr2` draft restricts new wide fallback retention to serial public OLS.
Plugin `2b956b420b141b2d5c1ddb119451f48e69fa50510ab77f7ff7ee52c202f23b03`
builds and passes ABI checks. Its native-only T1/T8/T16 confirmation at
`target/g21wr2` finishes eighteen clean calls, six independent inference checks,
eighteen exact repeats and nine output audits. Paired wr2/cb1 ratios are
0.6637/1.0022/0.9598. All finite numerical exports are exact; serial saved
vectors differ within independent tolerances. Threaded outputs are bit-identical.

Memory roots `target/g21mc1`, `g21mc2` and `g21mc3` preserve the initial reserve/
CLI mistake, baseline-and-m5 preparation rejection at 300 MB usable, and fast
rejection plus low/auto 120-second timeouts at 400 MB usable. Traces show that
the limited budget excludes LSMR and selects expensive CG; explicit LSMR
rejects with r(909). Do not weaken the admission model. The unconstrained
20-second diagnostic was too short: later m5/m6 runs complete around 38/39 s.
Those concurrent-build diagnostic times are not a performance comparison.

The first low-final-product draft, `m6` plugin
`53a0a60edb839a637b896e295ab24064cf3bbc8046a3b5383cc64195b6802ab7`, passes a
32-case weight/component/thread/pool reconstruction fixture and the K50/AW
public output audit. Its three-cell narrow low-mode screen at `target/g21low6`
finishes; one ring cell has a possible overlap. It has no effective public
retention change because the adapter still disables products in low mode.
The corrected `m7` draft requests products only for the final public ordinary
two-FE automatic projection. Earlier low-memory batches retain their original
nonretaining behavior. New storage is admitted before enabling CMG retention;
insufficient budgets preserve recovery. Its isolated build and 32-case fixture
pass. The public cycle/K6/AW/full-output run at `target/g21mc7` passes and takes
24.966 s, with only 6 ms spent recovering effects. Live sampling
(`target/ols-next-20260921/m7-low-sample1`) localizes the remaining cost to
cluster-ordered scratch-score reads repeatedly decoding large cache blocks.
The low-final-product drafts do not establish a speed benefit and are rejected;
their source snapshots and diagnostics remain preserved. A bounded read-ahead
change resolves that bottleneck in the exercised cases.

ADR 0091 accepts the [scratch-score cache change](benchmarks/reports/development/low-score-cache-local-2026-09-21/README.md)
for local development. `sc1` plugin
`76f216103849051d2d234c8e6f3b937ced9003f2d620a4ddb520b7e15a1d33dd`
passes 27 calls, six native independent inference checks, eighteen exact repeats,
six output audits and three focused core fixtures. Cycle/AW/K6 and K50 low-mode
clustered full-output medians improve 23.121/37.150 to 0.753/9.261 seconds versus
fixest 3.268/12.009. All nine native pairs have bit-identical exports and equal
allocation peaks. The dense/unit/K6 robust low-mode control is 1.887 versus
previous native 1.919 and fixest 0.378: its remaining reference gap is explicit.
No cell has detected overlap. Both cycle reference oracle failures remain.
The 57-call follow-on archive preserves m6/m7/sc1 and all four source identities;
1,380 stored entries, 48 omitted local arrays and source reconstruction verify.

The new serial retention memory diagnostic at `target/g21wm1` gives matching
cb1/wr2 r(909) preparation rejections at 300 MB usable. Both calls at 1.5 GB
usable (`target/g21wm2`) time out after sixty seconds. These failures remain
open. A tiny diagnostic linked to the tested wr1 core prints exact modeled
workspace costs: for T1/K50/ring, default Schwarz LSMR is 3,489,835,280 bytes,
diagonal is 1,143,472,656 and off is 967,475,728, before the execution parent.
At `target/g21wm3`, both default fits pass with 4 GB usable; explicit diagonal/
off LSMR fail warm-up certificates with r(459) before target invocation.
`target/g21wm4` tests those options through auto's stricter recurrence target:
both pass at 2/1.5 GB usable, with diagnostic elapsed times 46.337/54.019 seconds.
All four successful memory fits pass independent inference and output audits,
with tracked peaks below their caps. The ten-attempt memory receipt preserves
two allocation rejections, two timeouts and two warm-up failures. No model or
preconditioner policy changed; the traced times are not paired benchmarks.

The [CMG and wide-retention archive](benchmarks/reports/development/cmg-batch-wide-retention-local-2026-09-21/README.md)
preserves all 250 attempts, including six application failures, both rejected
guards and every original overlap exclusion. Integrity verification covers
5,845 stored entries, 193 omitted local output arrays and all five complete
source reconstructions. Forty-eight cb1 native pairs have exact numerical
exports; forty-three peaks match and five unit-ring peaks are lower. This
corrects the earlier overly broad statement that every peak matched.

SCC read-only preflight and a subsequent batch-mode recheck fail authentication
before any remote inspection or mutation. A renewal question is pending with
Johannes; no SCC job was submitted.
The cluster skill was read. Linux confirmation remains blocked on access.

The expanded-matrix isolation and failure-preserving score changes are committed
at `991081b7`: six fixtures and the 168-cell dry plan pass. A separate historical
partial-screen recheck keeps 114 complete cells and does not retroactively claim
clean isolation. The native-only audit option has been exercised by wr2's three
cells; its earlier auditor source is preserved before that small extension.

The [serial LSMR window experiment](benchmarks/reports/development/serial-lsmr-window-local-2026-09-21/README.md)
completes 26 successful calls. `sw1` enforces one live serial RHS and uses that
same bound in its captured memory model. Five batch tests and 63 memory tests
pass. The four-cell clean paired screen has exact numerical exports/samples/
saved vectors, eight independent inference checks, twenty-four exact repeats
and twelve output audits. Serial paired runtime ratios are 1.0011/1.0011/1.0092;
tracked peaks fall by 122–136 MB. Both 1.5/1 GB usable ring calls now pass
independent inference and outputs at a 523,613,047-byte tracked peak. The prior
wr2 timeout and the corrected output-auditor glob failure remain preserved.
The archive verifies 860 entries and both full source reconstructions.
This is an isolated generic experiment; an OLS-scoped implementation and Linux
evidence remain open. No production memory/admission rule was changed.

`be5079f7` commits broad protection timing isolation and v2 scoring. Three
score tests, four monitor tests and the unchanged 77/111-cell dry inventories
pass. Unrecorded or possible-overlap timings remain visible but cannot rank.

The `sf1` serial ordered-slope fusion screens complete 87 calls without detected
overlap. All thirty native pairs have bit-identical finite exports and samples;
all eighteen weighted output pairs also have exact saved vectors. The embedded
independent output audits pass. Unit K10/T1 is 118 to 113 ms versus fixest 92 ms
(paired native ratio 0.9573); K50/T1 and threaded controls are essentially stable.
Serial PW three-slope-only robust/all is 237 to 230 ms versus fixest 151 ms;
clustered/all is 261 to 262 versus 164 ms. Those serial reference gaps remain.

ADR 0092 accepts the compact Gram storage and ordered serial slope passes for
local development. The [source-bound report](benchmarks/reports/development/serial-slope-fusion-local-2026-09-21/README.md)
preserves 101 successful screen/diagnostic calls, three captured sources and
all thirty exact native pairs. Three focused slope fixtures and the plugin ABI
pass. Only indentation differs from the measured sf1 source. The updated slope
profiler passes four additional T1/T16 diagnostic calls with exact six-bundle
and sample equality; its trace and identity are in `profiler-check.json`.

Serial weighted output traces place about 26 ms in preparation, 85 ms in the
fit and 38 ms in outputs, including 14 ms in group recovery and 18 ms in fresh
finishing/certification. These phases overlap their subtotals; the fit's
`within_norms` also includes score-cache construction. Retaining single-slope
products for outputs remains an unimplemented memory/ownership question.

The [remaining graph coverage](benchmarks/reports/development/two-fe-coverage-tail-local-2026-09-21/README.md)
completes twenty-four cells with 198 successful calls and clean isolation.
All eighteen ordinary-fixest comparisons win by more than ten percent; six FW
cells use the native baseline. All 48 native inference checks, 144 exact repeats
and 36 candidate output audits pass. Nine reference inference audits pass and
nine disagree; all discrepancies remain. Integrity verifies 4,708 entries,
99 omitted local arrays and both complete source reconstructions.

`67d88596` commits the accepted compact/ordered slope kernel and its profiler.
The [protected-control report](benchmarks/reports/development/ols-protected-controls-local-2026-09-21/README.md)
completes all 111 unchanged cells against baseline `0efa78bd`, using sf1.
All 666 applications, 333 native numerical/sample/status/DoF comparisons and
129 independent candidate output audits pass with clean isolation. There are
252 exact six-bundle pairs and 78 exact output pairs. Two auditor fixtures
exercise a valid pair and nine corrupt/missing/failing cases.

One original control timing exceeds 1.05: N=16,384, K6, unit IID gives 1.06897
with 31/30 ms marginal medians. Its fresh five-pair single-command confirmation
passes at 1.04412 using the calibrated monotonic clock and 1.03571 using the
unchanged ordinary timer. All ten calls/five numerical comparisons pass. The
original failure and individual 0.895–1.189 ratios remain; no kernel adjustment
is justified. All control limits are met using that explicit confirmation.
Tracked peaks rise in eighteen cells, by at most 25,772,088 bytes; this is not
memory-neutral or complete allocation-proof evidence. The 676-call archive
preserves both clocks, all source identities and independent output audits.

An isolated direct-auto-LSMR diagnostic, `dl1` plugin
`2e18fcbba16f80eedb0b72dce4c8dfa0c2b4228442d2794477f7f737b16bb9ba`,
builds and passes the ABI fixture. It bypasses preceding Schur/CMG attempts only
inside captured public fast two-intercept OLS, preserving automatic LSMR
accuracy/admission/fallback. The production route is unchanged. The prepared
sixteen-cell controller is `target/ols-next-20260921/direct-route-screen1.py`.

Sixty descriptive locality measurements on twenty captured training inputs
show that pair collisions alone cannot select LSMR: they also flag weak graphs
where the current route wins. No threshold/rule is chosen. The first locality
invocation failed on an unmatched dense-size glob; its error is preserved and
the corrected results are `graph-locality2.json`. Both fresh routing seeds are
untouched. A third read-only SCC authentication check still fails; no jobs
have been submitted and the renewal question remains unanswered.

The [direct automatic LSMR screen](benchmarks/reports/development/direct-ols-lsmr-local-2026-09-21/README.md)
completes 32 cells and 192 successful calls at `target/g21dl1` and `g21dl2`,
with clean isolation. All 64 native inference checks, 192 exact within-arm
repeat checks and 48 candidate output audits pass. Cycles favor direct LSMR
across unit/AW, K6/K50, T1/T16 and both output requests: paired ratios range
from 0.5291 to 0.7673. Rings are mixed. AW threaded K6 regresses 14.6–16.7%,
and AW threaded K50 robust regresses 7.4%; the blanket switch is rejected.
Unit wide serial and threaded rings improve, but narrow serial unit differences
are within 5%. No production routing guard is selected. The complete portable
archive verifies both source reconstructions and auditor bindings.

The initial-norm draft uses the existing `certify_initial` helper immediately
after automatic absorption computes reference norms on the unchanged input.
Every FE moment remains freshly checked; only the duplicate norm pass is
removed. `in1` plugin
`11ba0ccff2155287bae6493302a76c57a5a8d6962fdda58179e8a345d4c2e3a3`
builds and passes the ABI fixture. Its focused exact certificate/workspace
fixture passes. The [sixteen-cell screen](benchmarks/reports/development/initial-ols-norm-local-2026-09-21/README.md)
completes 96 clean calls, 32 independent native inference checks, 96 within-arm
repeat checks and 48 candidate output audits. All 48 native pairs have exact
finite exports, samples, status, FE metadata, fresh certificates, saved vectors
and solver-report fields other than times. Paired ratios are 0.9653–1.0189:
this is a small pass reduction, not a broad speed breakthrough. Forty paired
peaks match and eight differ by -2,536,968 to +51,081,104 bytes in threaded
fits; the unchanged allocation model is not evidence of observed memory
neutrality. ADR 0094 accepts this norm handoff for local development. The first
archive attempt's missing-controller error and original script are preserved;
the corrected archive verifies both complete sources and auditor bindings.

ADR 0093 proposes an isolated exact-cycle guard. It recognizes one connected
degree-two smaller-axis Schur graph, three physical rows per larger-axis group,
moderate weights, and a narrow measured size/width/thread envelope. Its
label/row/effect-order, disconnected/path/chord, weight and budget fixture
passes. `cy1` plugin
`d160fc043d1def1728a17021a76c75d3b7d3586b6f96664be03f87e19cbc2eec`
builds and passes ABI checks. Both accepted and rejected probe costs are charged
outside early Schur returns. The detector exists only in a captured-source
transform; production routing remains unchanged. The 46-cell training screen
at `target/g21cy1` completes all 276 calls with clean timing isolation.
The [cycle screen archive](benchmarks/reports/development/cycle-ols-lsmr-local-2026-09-21/README.md)
preserves 92 passing independent native inference checks, 276 exact within-arm
repeat checks, 96 output audits and 138 routing comparisons. All 28 selected
cells improve, with paired ratios 0.5067–0.7500. All eighteen excluded/explicit
controls remain within 1.05 (worst 1.0465), and their numerical exports,
samples, saved vectors and solver fields other than times remain exact.
Selected tracked peaks increase by 18,571,188–482,354,402 bytes; this is a
material memory tradeoff. Joint descriptor accounting, constrained fits,
fresh validation and Linux evidence remain required before adoption.
The archive verifies 8,505 stored entries, 192 omitted local output arrays
and both complete source reconstructions, including the added detector module.

The [current in1 primary screen](benchmarks/reports/development/current-ols-primary-screen-local-2026-09-21/README.md)
at `target/g21pri1` completes all 77 unchanged
cells and 462 applications, with three pairs per cell and clean isolation.
`current-primary-score1.json` reports 77 wins; 75 meet the ten-percent target.
The two narrower margins are unbalanced K6/AW (0.92208) and shuffled K5/unit
(0.91892). All 77 independent native inference checks, 231 exact within-arm
repeat checks and 24 candidate output audits pass. The reference audit gives
61 passes and sixteen discrepancies, all preserved. The captured controller is
`target/ols-next-20260921/current-primary-screen1.py`; these are training
screens, not either final five-pair block. The portable archive verifies
8,457 stored entries, 48 omitted local arrays and the complete in1 source;
the separate privacy scan passes 8,467 members. Two earlier archive-checker
failures involving the checker's own path-pattern literals remain preserved in
`failed-current-primary-archive1` and `failed-current-primary-archive2`.

The graph auditor now supports one-native primary and two-native paired
inventories, preserving separate reference discrepancies. It also checks
per-arm repeat completeness so a missing native run cannot be offset by an
extra run in the other arm. Two focused fixtures pass, including missing,
extra, failed and altered native exports. Earlier auditor sources are archived.

ADR 0095 proposes a narrower implementation of the serial LSMR window: only
the original-input wide public serial Schur fallback with retained products
selects it. The same private policy reaches admission and transport, including
preconditioner fallback. Generic, shared, explicit and out-of-envelope calls
retain their default policy. The first `ow1` build failed on two legacy
memory-estimator callers; its source hashes and build errors are preserved in
`failed-ow1-source1.json` and the original logs. The corrected `ow2` build keeps
their original interface. Its build and four transport fixtures pass, but the
new memory fixture incorrectly required a strict peak decrease even when
unchanged preconditioner construction dominates. That failed assertion is
preserved. The corrected fixture checks exact construction cost in that case;
it changes no production memory estimate.

A further narrow-residual draft reuses existing tiled numeric subtraction only
for public automatic one-/two-FE overlap with six or seven model columns.
The earlier per-column residual streaming remains elsewhere. The combined
`nrt2` plugin is
`f57e1a4b50aa905c54759d452db211cec5db3288c0d3cdce5075b2e83cb5c28a`.
Its build/ABI, three narrow arithmetic/full-fit fixtures, four transport
fixtures, corrected memory fixture and forced-cap independent MAP fixture pass.
The captured ow2 source predates the narrow patch. A rejected `nr1` artifact-name
collision and all earlier build/test failures remain preserved.

The six-call constrained diagnostic `target/g21owmem1` completes. At 1 GB
usable, nrt2's unit ring K50/T1/full-output fit passes, with 523,613,047 tracked
allocation bytes and passing independent inference/output checks; the in1
comparison rejects with r(909). Both weighted 1 GB calls and both nrt2 300 MB
calls reject with r(909). These are admission diagnostics, not timing evidence
or a failure-transaction proof. The visible-error follow-up at `g21owmem3`
identifies the existing joint automatic memory preflight for the 1 GB weighted
rejection. Its preceding controller-only assertion failure at `g21owmem2`
is preserved. Both weighted 1.5 GB calls time out at ninety seconds. The
follow-up completes all four attempts; this scoped change does not resolve
the weighted constrained case.

The [scoped-window archive](benchmarks/reports/development/scoped-ols-window-local-2026-09-21/README.md)
completes both isolated screens. All thirteen window cells and 78 calls have
clean isolation, 26 passing native inference checks, 78 exact repeats and
72 output audits. All 39 cross-artifact pairs have exact exports, samples and
saved vectors. Unit ring K50/T1/all saves 121,868,720 tracked bytes, from
645,481,767 to 523,613,047, with paired time ratio 0.9948. Other peaks are
unchanged; all native time ratios lie between 0.9908 and 1.0171.

The fourteen-cell narrow-residual screen completes 126 calls with clean
isolation, 28 native inference checks, 84 exact repeats and six native output
audits. All 42 native pairs have exact exports, samples and allocation peaks.
The runtime gains are inconsistent, with ratios 0.9783–1.0345 including
excluded controls. This experiment is rejected and reverted; prepared_fit.rs
again exactly matches ow2. The reference audit preserves thirteen passes and
one discrepancy. A mistaken invocation of the native-only output auditor on
fixest is preserved as a controller error, not a scientific reference failure.

Auditor v3 optionally reuses independently solved equations only with a
matching solver source, complete prior audit, weight/VCE and verified original
input hash. New inputs use fresh solves. Four focused controller fixtures
pass; two fresh independent re-solves exactly reproduce cached unit/weighted
oracles. This avoids repeating the same dense factorization in later blocks;
each new export and each native runtime certificate is still checked.

The rebuilt `owf1` candidate is
`a2d0e6bd836c3f9af8992b9d49adcfaf669e3bac326e36ecc2f9deb12479ccb0`.
It contains the scoped window and the preceding residual traversal. Its build,
actual ABI and eight focused core fixtures pass. The 214-attempt archive
verifies 5,415 stored entries, 82 omitted arrays, five successful source trees
and the failed ow1 tree; the separate privacy scan passes 5,432 members.

The [current final Mac blocks](benchmarks/reports/development/current-ols-final-blocks-local-2026-09-21/README.md)
complete all 1,860 application calls, with no detected timing overlap in any
of the 186 cells. Both five-pair blocks win all 77 primary and sixteen holdout
medians. Worst primary ratios are 0.92369/0.91016; worst holdout ratios are
0.91304/0.91200. Each primary block has 76 cells at or below 0.90; the holdout
blocks have fifteen and thirteen. Shuffled unit K5 remains the narrowest win.
All 186 independent native inference checks, 930 exact within-arm repeats and
100 saved-output audits pass. Each primary reference audit preserves sixteen
discrepancies, and each holdout audit preserves two. The archive verifies
33,461 stored entries, 200 omitted local arrays and the complete owf1 source;
the privacy check passes 33,471 members. This is exact-artifact Mac evidence,
not completion of the broader milestone.

The [current protected controls](benchmarks/reports/development/current-ols-controls-local-2026-09-21/README.md)
complete 666 successful calls across all 111 cells with clean isolation. All
333 native scientific comparisons and 129 independent output audits pass;
252 six-bundle pairs and 78 saved-output pairs are bit-identical. Two original
timing failures remain recorded: AW K1/N61440 at 1.05263 and unit K14/N61440
at 1.09434. Fresh five-pair, single-command monotonic-clock confirmations give
0.99254 and 1.01545; both ordinary-clock medians are 1.0. All twenty new calls
and ten scientific comparisons pass with clean isolation. All 111 timing
limits are met using these explicit confirmations. Peaks increase in eighteen
cells, at most 25,772,088 bytes; no memory-neutrality claim follows. The
686-call archive verifies 21,120 entries, 258 omitted arrays and both complete
native source trees; its privacy check passes 21,132 members.

The [current expanded graph coverage](benchmarks/reports/development/current-two-fe-coverage-local-2026-09-21/README.md)
accounts for all 168 unchanged candidate cells. Original attempts total 996:
994 pass and two baseline FW ring calls time out. Two FW cycle saved-output
comparisons stop after their first pair. Fifty fresh confirmation calls pass
with clean isolation. All 168 candidate inference checks and original or fresh
per-arm repeats pass; saved-output normal-equation checks pass. All 126 ordinary
fixest comparisons win using one fresh confirmation: the original unbalanced
AW K6 nested ratio 1.37398 becomes 0.68852 (83/121 ms). The FW degree-two
original ratio 1.14758 becomes 0.91358 (369/404 ms). Forty FW cells have complete
native pairs within 1.05; the two baseline ring timeouts remain unranked.

A new independent Decimal cycle oracle resolves the FW saved-effect mismatch
in the candidate's favor. It uses original inputs and independent finite
coefficients, solves the grounded cycle equations at fifty and eighty digits,
and requires bit-identical float arrays. Both focused oracle tests pass against
dense weighted and literal-expansion equations. All ten fresh candidate fits
pass the original effect tolerances with maximum errors below 4e-11; all ten
baseline effect checks fail near 5.8e-8. Both arms pass inference and residual
checks. K6/K50 candidate medians are 312/920 ms versus baseline 4896/5563 ms.
All original failures, missing repeats and fresh paired mismatches remain.
The first joiner failed on an absent timing field in incomplete cells; its
source and failure are preserved. The 1,046-call archive verifies 22,594 stored
entries, 525 omitted arrays and three complete source trees; privacy passes
22,608 members. No new routing seed was used.

The [serial slope-product experiment](benchmarks/reports/development/serial-slope-products-local-2026-09-21/README.md)
retains computed products for serial, ordered, full-rank narrow saved fits.
The captured srp1 plugin passes ABI and three focused release tests, including
weighted full-fit MAP parity and the retention admission boundary. Initial
compile and test-fixture errors remain preserved. Its nine-cell screen completes
78 clean calls: 27 native pairs have exact numerical exports/samples, and 24
saved-output pairs pass the original tolerances. PW three-slope robust/clustered
medians improve 234/263 to 219/255 ms, versus fixest 148/164 ms. AW/FW and unit
one-slope cases change little. Targeted peaks rise by 8.3–9.4 MB; excluded
controls have exact outputs and equal peaks. Six exact profiling diagnostics
show recovery savings partly offset by recording/combining products. The
84-call archive verifies 2,088 entries, 75 omitted arrays and three source trees;
privacy passes 2,120 members. This remains an experiment. The two changed slope
files are restored exactly to owf1, verified against both captured identities;
`srp1-checkout-restoration1.json` records the local hashes. Serial losses remain.

The [cycle memory and routing checkpoint](benchmarks/reports/development/cycle-routing-memory-local-2026-09-21/README.md)
captures cy2 plugin `8a16cb9c4157f1f0dc2de60b6fcfb2a95e0c7794d81f1e1b914dc3b28369dff6`.
It budgets descriptor construction, checks LSMR admission before selection and
reuses the validated descriptor with retained-byte accounting. Six focused
release tests and the ABI fixture pass. The 46-cell three-pair screen completes
414 clean calls, 138 independent inference checks, 414 exact within-arm repeats,
288 saved-output audits and 138 routing comparisons. All 28 selected cells
improve, with paired cy2/owf1 ratios 0.5017–0.7325. All 18 excluded controls
retain exact science and solver reports and stay within 1.05; worst 1.0254.
The 28-cell auto/explicit LSMR comparison completes 168 clean, independently
accepted calls. Auto is fastest in 23 cells and within 4.05% in the other five.
The 18 constrained attempts preserve two allocation rejections and eight
60-second timeouts. All eight completed fits pass independent inference;
four saved-output fits pass reconstruction. Two tight-budget no-output pairs
retain the old route with exact numerical exports. The selected cycle route
is admitted for unit/T1 saved effects at 3 GB and AW/T16 at 12 GB. The existing
conservative threaded memory model still excludes it at 3 GB; it is unchanged.
The 600-attempt archive verifies 18,722 entries, 376 omitted arrays and three
source trees; privacy passes 18,743 members. A verifier-only privacy failure
and its correction are preserved. No source, tolerance or measurement was
changed during these comparisons. The production core still excludes cy2.

The cycle-memory checkpoint is committed locally at `c9962b38`. The candidate
is frozen in `target/ols-next-20260921/cycle-candidate-freeze1.json`, which binds
both plugin identities, the controller, scientific sources, guard sources,
training evidence, two seeds and thresholds. `cycle-validation-queue1.py`
completed two five-pair blocks: each has 46 current/candidate/reference cells
(FW omits fixest) and 28 automatic/explicit LSMR comparisons. Planned roots
are `target/g21cyval1`, `g21cyroute1`, `g21cyval2` and `g21cyroute2`. Their
per-phase logs are under `target/ols-next-20260921/cycle-validation-block*`.
All 1,920 application calls complete with clean isolation. Both blocks pass
independent native inference, saved outputs, route behavior and every frozen
timing criterion. Selected cy2/owf1 ratios are 0.5138–0.7610 and 0.5330–0.7628;
excluded-control maxima are 1.0383 and 1.0233. All 44 comparable reference
timings lose to cy2 in each block, but 34 default-reference scientific
disagreements remain failures in each. Worst automatic/fastest-valid ratios
are 1.0455 and 1.0412. The [validation archive](benchmarks/reports/development/cycle-routing-validation-local-2026-09-22/README.md)
verifies 49,181 entries, 1,230 omitted arrays and two complete source trees;
privacy passes all 49,199 scanned members.
The initial verifier's wrong field name, failed log and correction are
preserved; the corrected receipt is `integrity2.json`. The joined adjudication
accepts local validation only. Never retune the guard from these seeds;
production adoption still requires Linux confirmation.

The verified fresh cycle checkpoint is committed locally at `d2057e39`.
All headline cycle timing and audits are finished.

During the timing wait, source-only review identified a separate serial slope
experiment: accumulate OLS's max-normalized weight norms during the existing
ordered reconstruction, keeping raw-weight projection and QR certificates.
The captured `sn1` plugin
`9a491b4f6192d8233780673d8c1a3a1ca768946acccb9a7e76feb963f1c27cda`
builds and passes its ABI check and all five focused release tests in
`target/ols-next-20260921/sn1-tests1.json`: fresh normalized norms, exact budget,
actual worker-pool exclusion, existing independent slope oracle and ordered
serial fusion. The nine-cell three-pair screen completes 78 clean calls with
27 exact native science/output pairs. Two further four-cell five-pair native
confirmations complete 80 clean calls with 40 exact pairs. AW K10 paired ratios
are 0.9849 in both confirmations; AW K50 is 0.9701/0.9810. Three-slope-only PW
K6 instead regresses to 1.0308/1.0044. Unit-control ratios vary from 0.9583 to
1.0261. Six diagnostic calls retain exact results and confirm the norm scan
shrinks while absorption takes more work. The AW K10 phase saves about 6.6 ms
but adds 1.2 ms to absorption; wide AW saves 28.2 ms but adds 14.3 ms.

This broad norm handoff is not adopted. The [164-call archive](benchmarks/reports/development/serial-slope-normalized-norms-local-2026-09-22/README.md)
preserves both complete source trees, all five tests, the screen, confirmations,
filtered profiles and exact native audits. Integrity verifies 4,584 entries,
135 omitted arrays and two sources; privacy passes all 4,626 members. Its
source-disposition receipt records
restoring the builder and removing the active experiment files only after
archiving their exact bytes. The native slope source and six earlier core
drafts remain unchanged. Do not infer a broad optimization from removing one
pass: the measured multiple-slope whole-command result is worse.
Focused Linux confirmation remains blocked on SSH renewal. The final primary/holdout
archive is committed at `f8610cee` and current controls at `1439c23c`. Focused Linux confirmation
remains required before closing the milestone. The
initial norm handoff is committed at `4708834b`, cycle experiment evidence at
`9e4daec9`, and the primary training screen at `d764894f`. Six core files remain
modified, including the tested but unaccepted window and earlier graph
kernels. Keep old routes where cross-platform confirmation is missing;
never tune thresholds from the completed validation seeds. Serial weighted
slope losses remain.

### Serial slope data-pass checkpoint — 2026-09-22

The current-command `ssdiag1` profile completes twelve applications with four
exact native pairs. It locates the AW K10 serial robust costs at about 111 ms
in fitting, 40 ms in inference and 22 ms in outputs. PW three-slope recovery
alone takes about 35 ms. The pinned instrumented fixest phases are captured;
these are diagnostic traces, not headline comparisons.

ADR 0096 retains two exact-arithmetic refinements for local development.
`st1` traverses contiguous dense sandwich and OLS score-Gram triangle rows;
`rn1` adds one serial cached-slope traversal for reference norms and maxima.
Their two eight-cell three-pair screens complete 138 calls. Two frozen
seven-cell five-pair confirmation blocks add 200 calls. All timing isolation
checks pass, and all 118 native pairs have exact numerical exports, samples
and saved vectors with unchanged tracked allocation peaks. The candidate's
four focused release fixtures and ABI pass; st1 also passes thirteen sandwich
fixtures and four OLS/PPML fixtures. No solver-selection rule changes.

The combined plugin is
`28b1b049b15347256829e384ef725f3a1752f454fc2e6dd6b0beaa1836ec7281`.
AW K50 robust/full-output paired ratios are 0.8694/0.8520; medians are
894/858 ms against owf1 1,012/1,005 and fixest 1,990/1,963. AW K10 robust
ratios are 0.9768/0.9847, clustered 0.9467/0.9764. PW K6 robust remains a
small native regression, 1.0043/1.0090, and roughly 1.6 times fixest's time;
its cluster ratios are 0.9962/0.9624. This is a useful refinement, not closure
of the serial losses. Six final diagnostic calls add three exact native pairs:
AW/PW reference passes fall from 14.2/6.9 to 6.4/3.7 ms, and wide covariance
construction from 307.6 to 174.7 ms.

The [356-call archive](benchmarks/reports/development/serial-slope-data-passes-local-2026-09-22/README.md)
verifies 8,465 stored entries, 356 omitted local arrays and four reconstructed
source trees. Privacy checks pass 8,534 members. Measurements include the six
preceding graph/window drafts; old final blocks retain their original source
scope. The normalized-norm handoff rejected at `42d7f5ad` remains rejected.

### Serial slope recovery geometry checkpoint — 2026-09-22

ADR 0097 retains a bounded cache within serial direct recovery. Fit geometry
cannot simply be handed across: its raw Gram/QR rank policy differs from the
scaled, compensated recovery elimination. Instead, recovery retains its own
scaled Gram and channel scales for the same call's fresh residual certificate.
Residual scales, cross moments, rank solve and all finishing checks stay fresh.
Exact joint-budget admission preserves uncached direct recovery and MAP fallback.
PPML and threaded recovery remain unchanged; automatic routing is unchanged.

The `rgc1` plugin is
`eaaafa92105a11c5fc93335aead00f9b9931994c95ce7cc07cb19b82a21719ef`.
Its twelve-cell screen completes 105 calls and two independent seven-cell,
five-pair native confirmations add 140. All pass with clean isolation; all 106
native pairs have exact numerical exports, samples and requested saved arrays
with unchanged tracked peak allocation. PW full-output robust paired ratios
are 0.9871/0.9731, clustered 0.9762/0.9838 and saved-only 0.9571/0.9671.
FW robust ratios are 0.9724/0.9636; AW robust is neutral at 0.9922/1.0123.
This is a modest improvement, not closure of the serial reference gap.

Five focused release fixtures and normal/instrumented ABI checks pass. They
cover ranks, extreme scales, exact fresh certificates, threaded rejection,
budget fallback and protected PPML allocation/atomicity. Six diagnostics add
four exact native comparisons: PW recovery falls from 34.5 to 26.7 ms and AW
from 20.5 to 16.8 ms. The [251-call archive](benchmarks/reports/development/serial-slope-recovery-geometry-local-2026-09-22/README.md)
verifies 6,978 stored entries, 233 omitted local arrays and three complete source
reconstructions; privacy checks pass 7,018 members. The six preceding graph/window drafts remain separately
unaccepted; old final blocks do not transfer to this new source.

### Serial recovery row-cache checkpoint — 2026-09-22

The geometry checkpoint is committed at `8779f219`. ADR 0098 retains a subsequent
fixed four-double row cache within that admitted serial recovery. It removes
repeated slope/target divisions while preserving each compensated entry's
multiplication and row order. Generic MAP, uncached/threaded recovery and PPML
retain the old loop; no heap owner, admission or routing change is introduced.

`rrc1` plugin
`481f2aecc4a86f77925fc383ab5d0cfa71f7a58aa1b250bceb98f774a532e2f2`
passes the ABI check and five focused release fixtures. The seven-cell screen
and two frozen five-cell five-pair confirmations complete 163 successful calls
with clean isolation. All 71 native pairs have exact numerical exports, samples
and requested saved arrays, with unchanged allocation peaks. PW robust/full
paired ratios are 0.9825/0.9784, clustered 0.9960/0.9883. AW robust ratios are
0.9878/0.9959. Saved-effects-only remains mixed at 1.0204/0.9898. The original
AW screen regression is preserved. This is a small refinement, not reference
gap closure. The [163-call archive](benchmarks/reports/development/serial-slope-recovery-row-cache-local-2026-09-22/README.md)
verifies 4,515 stored entries, 154 omitted local arrays and two full sources;
privacy checks pass 4,537 members.

The row-cache checkpoint is committed at `d74d262d`. The subsequent [packed-cache
diagnostic](benchmarks/reports/development/serial-slope-packed-cache-profile-local-2026-09-22/README.md)
completes six calls with three exact native comparisons and an instrumented ABI
pass. The added timer finds AW K10/PW K6 packed-cache row passes of 10.13/5.64 ms.
Initial solver reports reveal a larger PW cost: geometry setup is 25.80 ms
(AW K10 is 12.63 ms). That setup is already included in the coarse `rhs` phase.
The K50 packed-cache event is warm-up-only; its actual fit uses the strided cache.
Audit 2 explicitly separates observation counts; the original audit is retained.
The archive verifies 270 stored entries, six omitted arrays and two full sources;
privacy checks pass 284 members.

### Serial slope geometry experiments — 2026-09-22

The `sgp1` diagnostic completes four calls with two exact native comparisons.
At N=245,760 and 49,152 groups, AW/PW QR costs about 7.62/18.09 ms, compared
with 1.50/3.05 ms for the raw Gram and 3.99/6.56 ms for normalization. Per-group
clock overhead raises total setup; these are diagnostic values, not headlines.

The combined raw-row-basis cache and first-pivot norm reuse (`sgn1`) passes
science and improves setup, but unit robust paired ratios are 1.0136/1.0220
in two five-pair blocks. Norm reuse alone (`snp1`) removes the raw cache and
still saves roughly one millisecond of setup; PW robust ratios remain
1.0136/1.0090. Other clustered/FW cells improve modestly, but neither experiment
consistently improves the remaining serial targets. Both are rejected and
removed from the working computational source, restoring the full captured
rrc1 source at that checkpoint. Do not reintroduce a weight/VCE-specific guard from
these timings or describe either experiment as accepted.

The two twelve-cell screens and four frozen seven-cell confirmation blocks
complete 490 successful calls with clean isolation. All 212 native pairs have
exact numerical exports, samples and requested saved arrays with equal peaks.
All 28 non-timing solver fields match in every pair. Each candidate passes
three focused release runs containing five Rust fixtures; all three new plugins
pass ABI. The [494-call archive](benchmarks/reports/development/serial-slope-geometry-refinement-local-2026-09-22/README.md)
preserves both rejected patches and verifies 13,642 stored entries, 458 omitted
arrays and four complete source reconstructions; privacy checks pass 13,705
members. Nothing is promoted across
source revisions, and the preceding six graph/window drafts remain unaccepted.

### Serial slope score-cache binding checkpoint — 2026-09-22

ADR 0099 retains numeric-column binding for packed inference-cache means after
successful serial cached-slope absorption. It reuses the existing bounded
NumericOlsColumns representation, preserving each mean's scale, multiplication,
row order and fixed-block compensated reduction. Cache layout, fresh checks,
heap ownership and unrelated admission guards are unchanged. Unsupported
recipes/widths retain the generic evaluator; threaded/strided and protected
explicit/shared/PPML paths retain their previous eligibility and arithmetic.

`scb1` plugin
`c68c679ecb3790139ece883bfe93da422cae212e338d693891a631d8986e1dc9`
passes four focused release fixtures in three invocations and the actual ABI
check. The twelve-cell screen and two unchanged seven-cell five-pair blocks
complete 245 successful calls with clean isolation. All 106 native pairs have
exact numerical exports, samples, requested saved arrays and all 28 non-timing
solver fields, with equal allocation peaks. AW robust/full paired ratios are
0.9560/0.9710; PW robust/full 0.9721/0.9766; PW clustered/full 0.9919/0.9917.
Unit robust/full is mixed at 1.0045/0.9679. The remaining serial reference
losses remain open; this is a small local improvement.

Nine diagnostic calls preserve exact outputs across prior/candidate/diagnostic
artifacts. Actual-N cache row times fall from 9.860 to 5.435 ms for AW K10 and
5.765 to 3.204 ms for PW K6. Actual K50 uses the strided cache; its warm-up
packed trace is excluded. The [254-call archive](benchmarks/reports/development/serial-slope-score-binding-local-2026-09-22/README.md)
verifies 7,103 stored entries, 236 omitted arrays and four complete source trees;
privacy checks pass 7,146 members. Both new plugins pass ABI.

### Fresh slope certificate traversal experiment — 2026-09-22

The score-cache binding checkpoint is committed at `a7f67b55`. A subsequent
contiguous-slice rewrite of cached slope checks (`sct1`) preserves arithmetic
but does not establish a consistent benefit. Its sixteen-cell three-pair
screen has AW K10/T1 robust ratio 1.0172, PW K6/T1 clustered 1.0247 and AW
K10/T16 robust 1.0270, alongside favorable FW and unit cases. These are mixed
screening ratios, not confirmed regressions. Single diagnostic fresh-check
times fall from 10.414 to 8.331 ms for AW K10 but rise from 8.834 to 9.988 ms
for PW K6. K50 is slightly faster. The rewrite is rejected; no width/weight/VCE
guard is fitted and no five-pair confirmation controller is executed.

All 141 screen calls pass with clean isolation. All 48 native pairs preserve
exact numerical exports, samples, requested saved arrays and all 28 non-timing
solver fields, with equal allocation peaks. Nine diagnostic calls have exact
outputs across prior/candidate/diagnostic artifacts. Five independent/budget/
eligibility Rust fixtures pass in three release invocations; both new plugins
pass ABI. The [150-call archive](benchmarks/reports/development/slope-certificate-traversal-local-2026-09-22/README.md)
preserves the rejected source and verifies 3,612 stored entries, 132 omitted
arrays and four complete source trees; privacy checks pass 3,656 members.
After rejection, the computational source matched captured scb1. The rejected
experiment and its archive are committed at `c52a59e3`.

### Fixed small covariance triangles checkpoint — 2026-09-22

Statistical sampling of owned Stata workers localizes about 80%/74% of prior
AW K10/PW K6 covariance samples to reduction, scaling and traversal, versus
16%/19% to row construction and 4%/7% to multiplier/reference-weight helpers.
These are stack-sample shares, not isolated triangle phase times. Two controls
and two sampled workers complete 162 target fits with exact final exports and
non-timing solver fields. Corrected audit v2 includes tail-called weight frames;
v1 remains preserved. Repeated-fit durations are diagnostic only.

ADR 0100 retains a fixed-array update for every reported dense sandwich
dimension 1..16. It borrows existing arrays, checks all shapes before mutation,
and preserves both meats' operand/observation order, separate multipliers,
finite checks, block partitions and forward/reverse compensated reductions.
Wider designs retain the generic loop. Shared OLS/PPML arithmetic and tracked
allocations remain unchanged; no solver policy or public surface changes.

`dtk1` plugin
`77b2b35764ae216267503b34a9965834bd3a6c8ef60e0f307bdf62c379f5b151`
passes nineteen focused release Rust fixtures in six invocations and the actual
ABI check. The sixteen-cell screen, two unchanged seven-cell five-pair blocks
and sixteen ordinary-FE controls complete 377 timed calls with clean isolation.
All 166 native pairs have exact six numerical exports, samples, requested saved
arrays and all 28 non-timing solver fields, with equal allocation peaks.
Confirmation AW K10 robust/full paired ratios are 0.9556/0.9667; PW K6
robust/full 0.9543/0.9821; unit K10 robust/full 0.9340/0.9358. Clustered changes
remain small/mixed, with AW ratios 1.0142/1.0253. All ordinary one-/two-FE
K1/5/15/16, T1/16 controls remain below the 1.05 limit (maximum 1.0333).

Nine separate diagnostic calls have exact outputs across all three artifacts.
AW K10 covariance falls from 24.031 to 11.511 ms; PW K6 from 11.285 to 6.327 ms.
Wide K50 remains about 171 ms. Both new plugins pass ABI. The
[local archive](benchmarks/reports/development/fixed-dense-triangle-local-2026-09-22/README.md)
contains 390 successful worker results and 548 target fits excluding warm-ups,
including the repeated profiling fits. It verifies 11,089 stored entries,
272 local-only arrays and four complete 523-file source trees; privacy checks
pass 11,149 members. Two failed archive verifier versions are preserved.

The accepted covariance checkpoint is committed at `e6ce7808`.

### Small Gram solver specialization experiment — 2026-09-22

`sgk1` specializes the existing Gaussian elimination loops for dimensions 1..4,
preserving validation, pivot/rank thresholds, swap/FMA order and allocations.
Prior stack samples attribute 25.4%/30.6% of AW/PW serial projection work to
this helper; the relevant sampled and current-baseline sources match. This
is historical statistical attribution, not new phase timing evidence.

The sixteen-cell three-pair screen completes 141 calls with clean isolation.
All 48 native pairs have exact six numerical exports, samples, saved arrays
and 28 non-timing solver fields, with equal tracked allocation peaks. Ten
focused release Rust fixtures pass in six invocations, including 5,400 exact
legacy arithmetic combinations and shared projection/PPML guards. Both new
plugins pass actual ABI checks. Nine separate diagnostic calls have exact
outputs across all three artifacts.

The screen is mixed: serial AW K10 robust/full ratio 1.0000, robust/no-output
1.0335; PW K6 robust/full 0.9953 and robust/saved 1.0106; unit robust/full
1.0147 and AW K50 robust/full 1.0224. PW clustering improves to 0.9297.
Single diagnostic projection/reconstruction falls from 10.743 to 9.380 ms for
AW K10 and 13.344 to 11.547 ms for PW K6, but the whole-command benefit is
not established. Reject the candidate without fitting weight/VCE guards or
running confirmation blocks. These screens are not confirmed regressions.

The [150-call archive](benchmarks/reports/development/small-gram-kernel-experiment-local-2026-09-22/README.md)
preserves the rejected code and verifies 3,624 stored entries, 132 local-only
arrays and four complete source trees; privacy checks pass 3,668 members.
The solver rewrite is reverted and its experiment-only tests are removed;
every computational source hash again matches captured dtk1. The six earlier
graph/window drafts are unchanged.

The rejected small-matrix experiment is committed at `28f3f47d`.

### Score-cache and normalized-norm fusion experiment — 2026-09-22

`scn1` computes fresh max-normalized within norms during the existing serial
packed score copy/mean loop. It preserves every norm's recurrence and row
order, including outcome and the excluded absorbed intercept, plus fixed mean
blocks and their compensated merge. Cache budget rejection and other guards
remain unchanged. It adds no heap owner and never rescales raw absorption norms.

The sixteen-cell screen, two unchanged seven-cell five-pair confirmations and
nine separate diagnostics complete 290 successful calls. All 118 timed native
pairs have exact six exports, samples, saved arrays and all 28 non-timing solver
fields, with equal allocation peaks. Seven focused release fixtures pass in
five invocations; both new plugins pass actual ABI checks. Diagnostic triples
also preserve exact exports and saved arrays.

Reject the candidate: AW robust/full paired ratios are 0.9866/1.0088, PW
robust/full 1.0144/0.9860, PW robust/saved 0.9896/1.0266 and FW robust/full
1.0263/1.0087. The screen's PW gains do not hold consistently. The combined
cache/norm diagnostic phase is 14.523 versus 14.486 ms for AW K10 and 8.431
versus 7.897 ms for PW K6. The new copy timer includes norm work, so its value
cannot be compared directly with the old copy/mean-only subphase. K50 remains
strided. No weight/VCE-specific guard or broad pass-fusion claim is accepted.

The [290-call archive](benchmarks/reports/development/score-cache-norm-fusion-experiment-local-2026-09-22/README.md)
preserves rejected source, tests, instrumentation and all evidence. It verifies
7,923 stored entries, 272 local-only arrays and four complete 523-file source
trees; privacy checks pass 7,975 members. Every computational source hash is
restored to dtk1, the instrument adapter matches its prior version, and all six
pre-existing graph/window drafts are unchanged.

### Sparse reference stopping diagnosis — 2026-09-22

The [six-case diagnosis](benchmarks/reports/development/sparse-reference-stopping-local-2026-09-22/README.md)
reproduces indices 704/722/737 and 773/791/806 on current dtk1: K1/K6,
unit/AW/PW, N1920, seed 8675309, crossed clustering and all outputs. All six
native grounded-SVD checks pass. A separately formed joint-design QR and
cluster sandwich also pass, with maximum native output error 2.621e-13.
The graph has 19 components and FE rank 415. The six stock reference
comparisons remain failures; index 773 additionally fails two covariance cells.

An isolated clone of the pinned fixest archive records that all 27 projected
columns exit at its forty-iteration SSR-change check. Instrumentation leaves
all exported numerical arrays byte-identical to stock. Disabling SSR stopping
removes the covariance failure and lowers maximum output error from 7.654e-5
to at most 8.383e-9, but near-zero outputs still miss the unchanged envelope.
With SSR stopping disabled and an explicitly diagnostic internal tolerance of
1e-14, all six reference fits pass the existing envelopes; maximum output error
is 2.209e-11. Tightening alone leaves the two unit-weight cases failing. Saved
recovery reconstructs the fitted contribution to below 7e-16: the discrepancy
originates in projection, not harness normalization or a second recovery solve.
The internal tolerance bypass is not a supported stock comparator invocation.

Twelve ordinary worker calls and thirty reference diagnostic fits complete.
All timings are diagnostic only. The archive verifies 560 entries, all 523
dtk1 computational source hashes, exact trace/stock arrays and input/artifact
bindings. Initial preparation and verifier failures remain recorded. No fereg
production code, tolerance, reference pin or historical outcome changed; the
original 766 reference failures are not reclassified. The six graph/window
drafts remain untouched. The six small sparse discrepancies are now explained,
not converted into reference passes.

### Batched fresh slope-check experiment — 2026-09-22

`sbf1` checks newly projected residuals in batches of 256 row-ordered groups,
using existing buffers, exact QR/raw-moment arithmetic and ordered reduction.
An enlarged final residual norm or invalid provisional check triggers the
ordinary complete certificate. Other errors remain fatal. The 513-group
fallback fixture covers RHS1/12/52, 1e200 inputs and zero/tiny reference norms;
all residuals, fresh final certificates and peaks match the ordinary path.
Six focused release fixtures pass in three source-bound runs, and ABI passes.

The sixteen-cell three-pair screen completes 141 clean calls. All 48 native
pairs have exact numerical exports, samples, saved vectors and 28 non-timing
solver fields, with equal allocation peaks. Reject the draft: AW robust/full
is 237/237 ms, paired ratio 1.0000; PW 212/211 ms, ratio 0.9953. Clustered
ratios are 1.0071/1.0203 and FW is 1.0090. Unit-weight robust/IID screen ratios
of 0.9659/0.9664 do not establish a general target gain. No weight-specific
guard or confirmation campaign is adopted.

Six diagnostic calls preserve exact outputs. Initial absorption, including
fresh checks, changes 26.172 to 25.529 ms for AW K10, 26.130 to 26.737 ms for
PW K6, and 83.076 to 85.336 ms for AW K50. Cache locality alone has not shown
a consistent phase improvement. The [147-call archive](benchmarks/reports/development/batched-fresh-slope-check-experiment-local-2026-09-22/README.md)
verifies 3,488 entries, 129 omitted local arrays and two complete source trees.
The failed initial edit and preliminary runs remain recorded. All 523 current
computational source hashes are restored to dtk1; the six old drafts are intact.

The 2026-09-22 closeout supersedes the previous instruction to continue serial
slope experiments. Computing each active QR column's norm in its last mutation
pass remains an untested idea, not part of this checkpoint. The small-matrix,
cache/norm fusion and batched-check experiments remain rejected; do not restore
the earlier raw/normalized norm handoff or first-pivot norm reuse alone.
Remaining serial reference losses include AW K10 robust/full at 226 versus
188 ms and PW K6 robust/full at 209 versus 152 ms in the screen. Initial slope
projection, fresh checks and PW setup/recovery remain costly. The preserved
native stack samples and diagnostic traces support a future, separately chosen
task. Do not restore rejected experiments or widen numeric binding based on
serial results. The six graph/window files are retained by the local closeout;
Linux acceptance and any broader validation on the combined source remain
deferred. Prior final timing blocks do not transfer to that source.

The fresh read-only SCC probe again returned permission denied; its local log
is `target/ols-next-20260921/scc-auth-current-20260922-1.txt`. Do not repeat the
same failed probe without changed access. If Johannes resumes Linux validation
after renewing access, use the existing OLS development smoke before focused
same-node T1/8/16 comparisons and constrained-memory checks. Do not launch a
qualification matrix, push, install or adopt the cycle route. All local
benchmark processes and closeout checks are finished.

## Repository cleanup and development push — 2026-09-21

Johannes requested committing and pushing all outstanding work, with cleanup
first. The [cleanup receipt](benchmarks/reports/development/repository-cleanup-2026-09-21/README.md)
records archival of the unreferenced CMG container draft outside active source,
restoration of six orphan-only inventory edits, and removal of six regenerable
LaTeX temporary files. No compiled estimator behavior changes. Previously
uncommitted allocation-history reports/manifests, Veneto benchmark sources and
aggregate receipts, and the absorbed-slope profiling report are included.

Original machine-local requests and the stopped-campaign evidence ZIP remain
local and unchanged. Portable exports preserve all numerical contents and map
original hashes to path-redacted derivatives. The stopped campaign remains
stopped. Private data, installed environments and raw licensed logs remain
ignored; this does not authorize any new SCC work or campaign continuation.

The repository checker reports allocation source/inventory drift against the
older pinned snapshots after the recent optimizations. Preserve that failure;
a new reviewed inventory checkpoint is needed before qualification. This push
is a development checkpoint, not release qualification. JSON/Python parse and
portable archive integrity checks accompany the cleanup. The completed local
two-way milestone below remains exact-artifact evidence for its captured source,
not a new qualification claim about this archival commit.

## Broad two-way OLS milestone complete — 2026-09-20

The [final development report](benchmarks/reports/development/two-fe-broad-primary-local-2026-09-20/README.md)
and its `development-acceptance.json` close the predeclared local categorical
two-FE OLS milestone. Both five-pair primary blocks win all 77 cell medians
(worst 0.960159 and 0.964567); all 770 individual primary pairs win. Both
sixteen-cell fresh holdout blocks also win every median (worst 0.952191 and
0.947368). Across primary and holdouts, 928/930 individual pairs win.
All 111 native controls have scientific coverage; the same-command precision
adjudication resolves the small timing control while preserving earlier
rounded failures. The aspirational ten-percent margin remains open in the
reported narrow/shuffled/weighted cells.

All 360 primary native graph oracle checks and eighty independent output
audits pass. Every primary cell preserves all six scientific bundles, samples
and requested output vectors bitwise across ten calls. The final separate
[reference accuracy report](benchmarks/reports/development/two-fe-final-reference-accuracy-local-2026-09-20/README.md)
covers all 28 distinct default-reference discrepancies: eighteen tighter
references agree, ten bottleneck/cycle references do not converge, and all
28 native calls reproduce headline science and outputs exactly. The independent
direct oracle passes all 26 applicable successful tighter graph calls. Default
reference discrepancies remain explicit and tighter timings replace no headline.

The [memory follow-up](benchmarks/reports/development/two-fe-memory-followup-local-2026-09-20/README.md)
records 42 successful calls across the twenty-one controls with earlier peak
increases or early-estimate exceedances. All prior science/output bundles are
bitwise exact; paired peak differences are unchanged. Observed peaks are at
most 1.058273 times the existing whole-action forecast. Strict proof-gap bits
125/381 and the seven earlier peak increases remain; no model was changed and
no complete memory-proof or release claim is made.

The accuracy follow-up preserves one Stata startup SIGTRAP before estimation.
The macOS trace identifies checked-string overflow; a fresh shorter benchmark
root/labels repeat the failed cell and complete the remaining calls. Original
failure and successful calls are archived. No estimator or worker change.
Final timing, science, memory, reference-adjudication and joined acceptance
checks pass; no live dispatch remains. Raw evidence is under
`target/two-fe-broad-20260920`, including `final-usefulpilot`,
`memory-report-followup`, `final-reference-precision` and its `rp2` continuation.

Production remains `cdfbcf58`, plugin
`79141637b5252f27d6f86a721fb04cf5648df6b8d0d82ee26ea7e39b9b94de14`;
all 516 captured source files match. This checkpoint changes reports only.
Five final-artifact Stata fixtures and earlier weighted/unit/oracle tests remain
linked. The baseline, fresh reference identity and actual worker hashes are
bound throughout the joined evidence. No installed package, remote state or
qualification changed. Preserve unrelated lower STATUS.md edits, the six
allocation manifests and existing untracked files.

Further optimization should target narrow shuffled computation and weighted
degree-six margins, then broaden graph-by-weight/inference/output combinations
before slopes, higher-dimensional FE or PPML. Those are new milestones, not
unfinished requirements of this declared local matrix. Release/platform and
strict allocation-proof gates remain separate.

## Broad two-way OLS milestone: full native controls — 2026-09-20

The [full-control report](benchmarks/reports/development/two-fe-broad-native-controls-local-2026-09-20/README.md)
accounts for all 111 declared controls and 129 independently audited output
pairs. All scientific comparisons pass. Production remains `cdfbcf58` and the
`usefulpilot` plugin `79141637b5252f27d6f86a721fb04cf5648df6b8d0d82ee26ea7e39b9b94de14`;
all 516 candidate source files match. A fresh audit also matches all 514
baseline source files to `e9d593bd`. Two additional final-artifact Stata
fixtures pass weighted explicit-dummy/frequency/rescaling and input boundaries.

The initial inventory completed 99 cells and preserved twelve one-FE saved-
output exporter failures. Estimation returned zero; a stale cached worker
requested a nonexistent second FE. The same twelve cells, inputs, binaries
and three pairs pass in a fresh controller directory. Original failures and
both worker versions remain archived. Controller source/generated-file
manifests now reject stale caches before dispatch. The independent output
checker now covers one FE; focused cache and six valid/six corrupted output
checks pass. No estimator code changed in this checkpoint.

One timing gate is still open: control index 56, AW IID at N=7,680/K=6 and
sixteen workers. Its original ratio is 1.05882; a fresh five-pair single-call
confirmation is 1.0625 (17 versus 16 ms). Five separate sixteen-command batch
pairs give 0.992, but do not replace the failed single-call gate. The report
retains 688 standard worker invocations (twelve export failures), ten batch
workers, 343 successful pairs and every failed attempt. All processes finished.

Seven cells have small measured peak increases, all below unchanged early
estimates; the largest is 5.112 MB in FW K=1. Fourteen distinct cells exceed
1.25 times the early estimate in BOTH artifacts, with identical peaks and
estimates. Those preexisting coverage gaps remain explicit; there is no new
complete-memory or release qualification claim.

Next use a separately documented high-resolution clock around the exact
public command to resolve the one-millisecond single-call timing question;
retain the ordinary timer and original failure, and do not substitute batch
timing. Then measure the sixteen predeclared fresh holdouts and both five-pair
77-cell primary blocks. The goal remains active. Artifacts/raw evidence are
in `target/two-fe-broad-20260920`. The legacy root controller is intentionally
unversioned and will now fail preflight; use a fresh measurement directory
with `--artifacts` pointing to that root. The repaired, manifest-checked root
is `target/two-fe-broad-20260920/onefe-controller-repair`. Preserve unrelated
dirt. No installed package, remote state or qualification changed.

## Broad two-way OLS milestone: useful narrow pilot — 2026-09-20

The [useful-pilot report](benchmarks/reports/development/two-fe-useful-pilot-local-2026-09-20/README.md) records 232 successful application calls,
92 native pairs without allocation peak increases, seventeen independent
output audits and eighteen graph/weight oracle checks. Sixteen Schur tests,
three final-artifact Stata fixtures and the actual-plugin ABI/panic check pass.
[ADR 0086](docs/adr/0086-useful-narrow-ols-schur-pilot.md) retains full-width pilot
state for already-admitted 3–8 RHS calls, preserving exact selected-column
weighted admission, original-input rejection, guarded unsafe-column fallback,
fresh certification and the existing eight-plus-64/shared iteration budget.

All sixteen three-pair headline cells beat default fixest (worst median paired
ratio 0.948). Weighted degree-six K=6 improves 77 -> 64 ms versus 74 ms;
shuffled unit K=5 improves 255 -> 243 ms versus 250 ms. Three initially slower
controls remain recorded; fresh three-pair dispersed/low-memory/sparse AW
ratios are 1.02287/0.98571/1.03333. Dispersed-output variability remains material.
Nine default-reference output discrepancies remain explicit. Fifty-three of
92 native pairs and eight of seventeen output pairs are bitwise exact; the
remaining pairs meet registered scientific comparisons and independent audits.
Failed initial compile/tests and their numerical localization are retained.

Plugin `79141637b5252f27d6f86a721fb04cf5648df6b8d0d82ee26ea7e39b9b94de14`
uses base `558f4c46` plus three core overlays; all 516 captured source files
match. Traces reduce charged initial steps 43 -> 35 on shuffled designs and
18 -> 10 on degree-six designs. All checkpoint processes have finished.

The full goal remains active. Next run the 111 native controls against the
milestone baseline, then sixteen predeclared fresh holdouts and two final
five-pair primary blocks. Use the `usefulpilot` artifact in
`target/two-fe-broad-20260920`; preserve every cell, reference-accuracy
adjudication and unrelated worktree change. No installed package, remote
state or qualification changed.

## Broad two-way OLS milestone: useful pilot cost — 2026-09-20

The [full-width pilot cost report](benchmarks/reports/development/two-fe-full-pilot-cost-local-2026-09-20/README.md)
records sixteen successful diagnostic calls and eight exact native scientific
and non-timing solver pairs, including full-precision contraction. No native
allocation peak increases. Production remains `3fa869fa`: the isolated
`fullpilotcost` artifact advances every narrow RHS but still discards its state.
Its one-file overlay replays exactly; the other 515 files and original shared
three-column function are unchanged. Plugin
`c9f1837fa142cc05ad9e9902b6af4b70f7577a2e4a36fe37201f58fa93665e5a`
passes the ABI check. All processes have finished.

Full-width pilot costs are 8.257/9.378/9.749 ms for weighted degree-six,
degree-three and unbalanced designs, versus 7.669/6.922/7.283 ms for the old
three-column pilot. The premium is promising relative to repeated full-solve
initialization and eight steps, but retained-state speedup is not implemented
or proven. Shuffled unit K=5 and AW K=6 cost 39.237/46.414 ms versus
23.120/34.703 ms; their net benefit particularly needs measurement.

Next implement an opaque same-call handoff owning original values, full graph
and recurrence scalars. Preserve the selected-column admission decision and
handle unsafe extra-column state via the prior certified solve. Rejected
probes must return untouched data, release state and record actual peak/cost.
Count useful steps once within the unchanged former eight-plus-64 allowance;
certification, current-value norms and fallback remain authoritative. Tests
must cover near-absorbed columns, weighted cuts, rejection and charged caps
before benchmarking. The full goal, 111 controls, sixteen fresh holdouts and
two five-pair final blocks remain active. Raw work remains
`target/two-fe-broad-20260920`; preserve unrelated dirt. No installed package,
remote state or qualification changed.

## Broad two-way OLS milestone: fused reconstruction — 2026-09-20

The [fused reconstruction report](benchmarks/reports/development/two-fe-fused-reconstruction-local-2026-09-20/README.md)
retains 125 successful application calls, 49 bitwise-exact native scientific
pairs (47 production, two diagnostic), thirteen exact and independently
audited output pairs, fourteen Schur tests and three final-artifact Stata
fixtures. All native allocation peaks are unchanged. [ADR 0085](docs/adr/0085-ols-schur-restoration-during-reconstruction.md)
restores cached left means during the existing reconstruction traversal,
preserving per-sum arithmetic and group totals. No cell median paired native
regression exceeds 5%; nine default-reference output discrepancies remain.

Diagnostic reconstruction falls 7.897 -> 5.684 ms for shuffled unit K=5 and
5.679 -> 4.724 ms for AW K=6. Three-pair shuffled AW IID improves 295 -> 285 ms
versus fixest's 303 ms, while shuffled AW full outputs are 1.1% slower; retain
that result and do not attribute all application-time movement to fusion.
Shuffled unit K=5 still trails fixest, 255 versus 248 ms. Plugin
`ec0a780f7b2714dea64339b7f2e581f57e490625b43efec170f8541a7d3ac29e`
uses base `830ca43c` plus one core overlay; all 516 source entries match.
The ABI check passes. A trace-count archive assertion was corrected without
replacing measurements; its failure log is retained. All processes finished.

The full goal remains active. Next retain useful narrow Schur probe work,
with unchanged weighted admission, original-input rejection, shared iteration
charging and memory bounds. Then run the 111 native controls, sixteen fresh
holdouts and two final five-pair primary blocks. Raw work remains
`target/two-fe-broad-20260920`; preserve unrelated dirt. No installed package,
remote state or qualification changed.

## Broad two-way OLS milestone: weighted statistics overlap — 2026-09-20

The [weighted statistics overlap report](benchmarks/reports/development/two-fe-weighted-statistics-overlap-local-2026-09-20/README.md)
retains 170 successful application calls, 67 bitwise-exact native scientific
pairs (65 production, two diagnostic), sixteen exact and independently audited
output pairs, nine focused tests and three final-artifact Stata fixtures.
All native allocation peaks are unchanged. [ADR 0084](docs/adr/0084-weighted-independent-statistics-overlap.md)
extends bounded independent source scans to AW/PW/FW, keeping exact source and
fitted-scale checks and original deferred error order. No cell median paired
native regression exceeds 5%; nine default-reference output discrepancies
remain explicit.

Three-pair shuffled AW K=6 IID improves 305 -> 292 ms versus fixest's 302 ms;
ordered K=50 improves 748 -> 721 ms. Degree-three/six/unbalanced K=6 AW remains
84/79/82 ms versus 83/72/79 ms. Weighted narrow one-FE also still trails fixest.
The isolated profile moves 22.170 ms of independent source work beside fitting
and reduces the large shuffled post-fit weight report 10.192 -> 0.001 ms.
Plugin `6456f0911703da8c16db50692ae9705459ded36074edd39e81b8a26b1d354561`
uses base `70a79e8c` plus three OLS overlays; all 516 source entries match.
Both production/diagnostic ABI checks pass and the diagnostic instrumentation
replays exactly. All checkpoint processes have finished.

The full goal remains active. Next fuse left-mean restoration into existing
reconstruction, then reduce duplicate Schur probe/initialization. The 111
native controls, sixteen fresh-seed holdouts and two final five-pair blocks
remain outstanding. Raw work is `target/two-fe-broad-20260920`; preserve
unrelated dirt. No installed package, remote state or qualification changed.

## Broad two-way OLS milestone: weighted statistics profile — 2026-09-20

The [weighted statistics profile](benchmarks/reports/development/two-fe-weighted-statistics-profile-local-2026-09-20/README.md)
records eight successful diagnostic calls and four bitwise-exact native
scientific pairs. It instruments only an isolated copy of
`prepared_unadjusted.rs`; production remains `3ea948de`. The reusable builder
now supports `--profile-statistics`. Plugin
`59c6a80b4c8eb460ffc93ed64cf539852dae335107cb86bb322f0c347f75e564`
uses base `3ea948de`; 515 source files remain unchanged and the one-file
instrumentation replays exactly. Actual ABI/panic checks pass. All processes
have completed; no production algorithm or installed package changed.

Large shuffled AW IID spends 10.309 ms in weight validation/scaling and
10.248 ms in outcome statistics; RSS already overlaps the latter and costs
4.404 ms. Degree-six AW at N=245760 has corresponding 2.668/2.668/1.140 ms
phases. FW weight reporting alone costs 20.218 ms. Next test overlapping
independent weight-report scans AND outcome statistics with fitting, with
scalar same-call state and original deferred error order. Validate fitted
scale bits before accepting speculative TSS. Its multiplier must use the
same direct compensated normalized sum as the fit, not a raw-sum multiply/
divide round trip. Preserve the original scan on a scale mismatch. Then
reduce duplicate Schur probe/initialization and fuse left-mean restoration
with reconstruction. These are proposals, not implemented optimizations.

The full goal remains active: 111 native controls, sixteen fresh holdouts and
two final five-pair blocks are outstanding. Raw work remains
`target/two-fe-broad-20260920`; preserve unrelated dirt. No remote state or
qualification changed.

## Broad two-way OLS milestone: compact weighted cache — 2026-09-20

The [compact weighted-cache report](benchmarks/reports/development/two-fe-weighted-both-cache-local-2026-09-20/README.md)
retains 100 successful application calls, 41 native pairs without a peak
increase, sixteen independent output audits, thirteen Schur tests and three
final-artifact Stata fixtures. [ADR 0083](docs/adr/0083-compact-weighted-ols-edge-scratch.md)
admits both compact weighted gathers and offsets in existing idle banks;
capacity rejection keeps the prior one-sided cache. Weight precision,
certification and explicit OLS authority remain. PW/FW, concentrated weights,
tiny bridges, low memory, threads and protected unit/one-FE controls pass.
No native cell median paired regression exceeds 5%; nine default-reference
output disagreements remain explicit.

Three-pair shuffled AW K=6 IID improves 336 -> 316 ms, still above fixest's
298 ms; full outputs improve 486 -> 466 ms versus 545 ms. Ordered K=50 IID
improves 770 -> 747 ms versus 1957 ms. Plugin
`e2325d207fa4a7a5afe23c9cecf0c4bcc73c383294586f7e31f3fca317b023f3`
uses base `f44fabb9` plus the recorded core overlay; all 516 source entries
match. All checkpoint processes have finished. The full goal remains active.
Next profile the roughly 21 ms weighted IID statistics phase more finely,
then overlap independent weight/outcome checks where ownership and exact
scale checks permit. Reduce duplicate Schur probe/initialization and fuse
left-mean restoration with reconstruction after that. The 111 controls,
sixteen fresh holdouts and two final five-pair blocks remain. Raw work is
`target/two-fe-broad-20260920`; preserve unrelated dirt. No installed package,
remote state or qualification changed.

## Broad two-way OLS milestone: two-sided unit cache — 2026-09-20

The [two-sided unit-cache report](benchmarks/reports/development/two-fe-unit-both-cache-local-2026-09-20/README.md)
retains 101 successful application calls, 34 native pairs without a peak
increase, four independent output audits, thirteen Schur tests and three
final-artifact Stata fixtures. [ADR 0082](docs/adr/0082-two-sided-unit-ols-edge-scratch.md)
admits the other counted gather in existing idle left-RHS storage for narrow
unit OLS. Scratch authority keeps shared/PPML behavior unchanged. All native
numerical comparisons and exact status/DoF checks pass; four default-reference
output discrepancies remain explicit. Three-pair shuffled K=3/5/6 improves
233/268/259 -> 225/258/252 ms; K=5 still trails fixest's 248 ms. No native
cell has a median paired regression above 5%.

Plugin `c26b715a42a9c704e26b4d7caf46afee27e626776f4369fd2e6483ad0cc7ee3c`
uses base `33b8b7ba` plus the recorded core overlay; all 515 source entries
match. All checkpoint processes have finished. The full goal remains active.
Next: compress/transcribe both weighted edge gathers into existing idle banks,
then reduce restoration/finishing scans and duplicate probe initialization.
The 111 controls, sixteen fresh holdouts and two final five-pair blocks remain.
Raw work is `target/two-fe-broad-20260920`; preserve unrelated dirt. No installed
package, remote state or qualification changed.

## Broad two-way OLS milestone: narrow input checkpoint — 2026-09-20

The [narrow-input report](benchmarks/reports/development/two-fe-narrow-input-local-2026-09-20/README.md)
retains the 107-call incomplete-consumer prototype and corrected 226-call
screen. All 114 native pairs preserve the checked science exactly; seven
independent output audits, focused transport/consumer/weighted tests and three
final-artifact Stata fixtures pass. The consumer separately admits narrow
loaded-RHS binding, preserving other numeric-kernel guards and exact final
value checks. [ADR 0081](docs/adr/0081-narrow-ols-loaded-rhs-reuse.md) records
scope and the accepted earlier-lifetime memory cost: two K=1 output peaks rise
about 4 MB (2.8–3.3%) within unchanged estimates; one robust case adds 72 bytes.
Two unchanged low-memory/four-thread controls retain pre-existing gaps relative
to the early fixed estimate; no full memory-qualification claim is made.

Three-pair shuffled unit K=5/6 improves 285/269 -> 273/258 ms; narrow one-FE
K=1/5 improves 78/114 -> 67/100 ms. Weighted shuffled K=6 IID is still
338 ms versus 300 ms fixest. Weighted degree-six is 82 versus 71 ms;
other narrow losses remain. No native cell has a median paired regression
above 5%. Seven default-reference output disagreements remain explicit.

Plugin `ed58adbeb34ef6ff34a56b3ad90b98ea9962f387b939f554d4896f9add30a24b`
uses base `c9635415` plus five overlays; all 515 source entries match. All
narrow-input builds, tests, fixtures and benchmarks have finished. The full
goal remains active. Next measure whether both repeated-edge operator gathers
can use existing idle banks, then weighted finishing scans and duplicate
probe work. The 111 controls, sixteen fresh holdouts and two final five-pair
blocks remain. Raw work: `target/two-fe-broad-20260920`. Preserve unrelated
dirt; no installed package, remote state or qualification changed.

## Broad two-way OLS milestone: routing diagnostics — 2026-09-20

The [forced-solver report](benchmarks/reports/development/two-fe-narrow-solvers-local-2026-09-20/README.md)
records thirty successful K=6 degree-three, degree-six and unbalanced calls,
unit/AW, at N=245,760. Auto is fastest in all six cells; every scientific
comparison passes. A simple forced-family substitution cannot close these
gaps. The next route work should target duplicate Schur probe/initialization.

The narrow-input prototype remains unaccepted. Its 107-call first screen
preserves all scientific files exactly but is mostly neutral: an inner core
width guard discarded the newly loaded RHS. The corrected candidate now
widens only loaded-RHS binding; predictor, residual and ordinary RHS guards
stay unchanged. Its pointer-reuse and weighted tests plus artifact build are
running in `target/two-fe-broad-20260920` (sessions 76727 and 44788).
Retain the first screen under the uncommitted narrow-input report. The final
candidate still needs measured gains and completed-artifact fixtures.

`broad_holdouts.json` predeclares sixteen fresh-seed cells without replacing
the original 77. `score.py` mechanically verifies timing/input/sample/artifact
identities and retains every failure; it reproduces 69 wins, one tie and seven
losses on the completed screen and does not adjudicate scientific acceptance.
The full goal, 111 controls and two final five-pair blocks remain active.
No installed package, remote state or qualification changed.

## Broad two-way OLS milestone: weighted cache checkpoint — 2026-09-20

The [weighted-cache report](benchmarks/reports/development/two-fe-weighted-edge-cache-local-2026-09-20/README.md)
records 121 successful calls, fifty native pairs without a peak increase,
22 independent output audits, thirteen focused Schur tests and three Stata
fixtures. Existing idle banks hold right-neighbor codes and f64 weighted
edges; left means are restored before certified reconstruction. PPML/shared
reusable calls opt out. [ADR 0080](docs/adr/0080-weighted-schur-edge-scratch.md)
defines the admission and lifetime. Weighted shuffled K=6 IID improves
372 -> 340 ms, still above fixest's 299 ms. Ordered K=50 IID improves
858 -> 768 ms. Three initially slower controls pass repeated confirmation;
the initial results and test-only tolerance localization are preserved.

Plugin `337deed4ecdae50f490f84cd8731f1be68beb69b3151bdde7a0ce2a5be4edb9f`
uses base `455150af` plus the recorded overlay; 514 source entries match and
the added licensed fixture is separately hashed. All weighted-cache processes
are complete. The full goal remains active. Next measure narrow input/RHS and
moment overlap at K=1/3/5 before changing guards, then graph probe/routing costs.
The 111 controls, broad weight/science holdouts and two five-pair blocks remain.
Raw work: `target/two-fe-broad-20260920`. Preserve unrelated worktree changes;
no installed package, remote state or qualification changed.

## Broad two-way OLS milestone: complete first screen — 2026-09-20

The [77-cell screen](benchmarks/reports/development/two-fe-broad-screen-local-2026-09-20/README.md)
records 69 strict wins, one tie and seven losses against default fixest. All
154 application calls succeed. Every ordered dense case and every K=50 case
wins; remaining losses concentrate in narrow shuffled and well-connected
K=6 designs. This single-pair screen does not close the two-block target.
All 36 native other-graph cells pass the independent sparse-direct oracle;
eight native output audits pass. Eight reference output and sixteen reference
coefficient mismatches remain explicit, with separate prior adjudication.

Source is `455150af`, plugin `8ec5df3a1686c9f476dcf2a83b63318ee252f789b1e49b75c624d5300d76efb2`;
all 514 captured source entries match. A weighted right-edge scratch prototype
is now under validation, improving shuffled AW K=6 IID 372 -> 340 ms versus
299 ms fixest in three pairs. Its weighted operator/MAP/fallback tests and
three Stata fixtures pass, but its broader weight/control screen is pending;
do not yet treat it as accepted. Raw work is `target/two-fe-broad-20260920`.
The full goal remains active: finish weighted-cache validation, narrow input
and routing work, the 111 controls, fresh holdouts and two five-pair blocks.
No installed package, remote state or qualification changed.

## Broad two-way OLS milestone: narrow cache checkpoint — 2026-09-20

The [narrow-cache report](benchmarks/reports/development/two-fe-narrow-edge-cache-local-2026-09-20/README.md)
records the accepted guarded one-bank unit Schur cache and its failed initial
admission. Existing idle scratch can cache the more expensive right gather
when two banks do not fit; a bounded repeated-edge sample avoids unprofitable
sorting. No heap owner, memory allowance or scientific contract changes.
[ADR 0079](docs/adr/0079-narrow-one-sided-schur-edge-cache.md) defines the guard.

Final three-pair ordered K=3/5/6 medians improve 241/270/264 -> 224/257/242 ms.
Shuffled medians improve 266/300/284 -> 243/279/265 ms but remain slower than
fixest (paired ratios 1.0084/1.1377/1.0315). The first prototype's 5.9% degree-six
regression is preserved; five guarded-candidate pairs have median ratio 1.02985.
All sixty native pairs preserve status/DoF, pass numerical comparisons and
raise no native peak. Two focused tests and three final-artifact Stata fixtures
pass. The report retains all 164 calls and both prototype source patches.

Final plugin `8ec5df3a1686c9f476dcf2a83b63318ee252f789b1e49b75c624d5300d76efb2`
uses base `329bc17c` plus the recorded core overlay; all 514 source entries match.
All cache screen/validation processes have completed. The full goal remains
active. Next run the predeclared 77-cell single-pair primary screen using this
artifact, then prioritize weighted operator reuse and narrow input/probe passes
from the remaining losses. The 111 controls, two final five-pair blocks, broad
weight-distribution/bridge science and routing holdouts remain pending. Raw
work remains in `target/two-fe-broad-20260920`. No installed package, remote
state or qualification changes; preserve unrelated worktree dirt.

## Broad two-way graph and weighted-oracle checkpoint — 2026-09-20

The [graph/science report](benchmarks/reports/development/two-fe-broad-graph-science-local-2026-09-20/README.md)
archives 80 application calls and preserves all reference and harness failures.
The first eighteen graph cells use K=6 and unit/moderate AW weights. An independent
grounded sparse-direct oracle passes all native coefficients, covariance, RSS
and effective N; eight default fixest coefficient comparisons fail. Twelve
small explicit-dummy SVD self-checks validate that independent implementation.
Four tight-reference degree-two/sparse comparisons pass; all four tight
bottleneck/cycle references reach their 10,000-iteration cap. Neither failed
reference timing nor a tightened reference substitutes for the default target.

The disconnected clustered covariance discrepancy is localized to the pinned
reference's nonnested/exact rank correction (242 versus independent/native
243). Explicit full/exact correction passes when neither FE is nested; the
harness verifies that precondition. Zero-width outputs, literal FW expansion
including clustered covariance, and weighted one-FE output controls pass.
The broad controller expands the frozen plan to 77 primary cells and 111 native
controls; the full screen and final acceptance blocks remain pending.

Production remains the committed weighted-input checkpoint `329bc17c`. A
one-sided unit Schur edge-cache prototype improves dense K=3/5/6 but slows the
degree-six control by 5.9%. Its evidence is retained under the uncommitted
narrow-edge report. A revised bounded repeated-edge guard passes two focused
operator/projection tests and its actual-plugin ABI build. Its paired screen
and final-artifact fixtures are running under
`target/two-fe-broad-20260920/screen-narrowedges2.py` (exec session 2743).
Do not accept the cache until that measured control passes. The full goal
remains active; weighted/narrow operator work, graph routing, broader controls
and repeated scoreboards remain. No installed package or remote state changes.

## Broad two-way OLS milestone: weighted input checkpoint — 2026-09-20

The [weighted input report](benchmarks/reports/development/two-fe-weighted-input-local-2026-09-20/README.md)
records the next bounded change: supplied AW/FW/PW columns join the existing
numeric transport and speculative raw RHS path. Authoritative preparation and
exact final matching remain; weighted raw moments retain their prepared scan.
[ADR 0078](docs/adr/0078-weighted-native-input-overlap.md) defines the scope.

Three-pair AW K=50 IID improves 937 -> 851 ms; full outputs 1112 -> 1033 ms.
K=6 gains are smaller. Shuffled AW and strong-d3 AW fitting still lose to
fixest. All 34 native scientific pairs and 15 saved-output pairs are exact,
with no native peak increase. Fifteen independent output audits, three Stata
fixtures, focused weighted transport/pipeline tests and actual ABI checks pass.
The first low-memory pair's 6.6% slowdown did not repeat: three confirmation
pairs give median ratio 0.967611. All 110 calls, including the 39-call inactive
prototype and original slower control, remain in the report.

Candidate plugin `7adc6377c38d280007de94985fb05478456459def6263ebf6cfd6941ff21cfdf`
uses `3a02a7da` plus the recorded plugin overlay; all 514 source entries match.
The full goal remains active. The raw root is `target/two-fe-broad-20260920`;
weighted-input validation/confirmation and the 18-cell first graph screen are
complete, with no running process. Next: archive the graph/DoF/zero-width
harness evidence, independently adjudicate eight default-reference coefficient
discrepancies, and target narrow/shuffled operator costs and graph routing.
The wider predeclared scoreboard has not run. Preserve unrelated worktree
changes; no installed package, remote state or qualification changed.

## Broad two-way OLS milestone: weighted finishing checkpoint — 2026-09-20

The full user-approved plan remains active: weighted lifecycle and semantics,
weighted loading/passes and narrow operators, other graph families, automatic
routing boundaries, and the predefined two-block performance scoreboard.
The local baseline is `e9d593bd`; no platform or release qualification is implied.

The [weighted finishing report](benchmarks/reports/development/two-fe-weighted-finish-local-2026-09-20/README.md)
records the first completed change: accepted weighted initial probes may be
reused by the independently certified strict final projection, and weighted
two-intercept outputs may use certified graph recovery. Existing thread,
sample-size, memory and fallback guards remain. [ADR 0077](docs/adr/0077-weighted-final-projection-and-graph-recovery.md)
describes this bounded extension.

Three-pair ordered AW K=6 full-output medians improve 1013 to 493 ms versus
706 ms default fixest; saved FE improve 980 to 447 ms versus 717 ms. Default
reference output discrepancies remain explicit. Six separate tight-reference
AW/PW comparisons at K=6/50 pass. Shuffled AW IID still loses (paired ratio
1.199); shuffled full outputs approach parity (0.991). Profile calls localize
strict final projection to 237 -> 45 ms and recovery to 369 -> 37 ms.

Plugin `732ec4b95ed7a37aea70abfd65485b443a5788ba7e8218d28de43ead689cf90e`
matches 514 captured source entries on `e9d593bd` plus the three recorded core
overlays. Twelve Schur tests, three graph-recovery tests, the actual-plugin
ABI check and three Stata fixtures pass. All 82 application calls succeed;
22 independent output audits pass; no native allocation peak increases in
25 paired comparisons. This evidence does not close the full milestone.

The raw work directory is `target/two-fe-broad-20260920`. Its weighted finishing
screen and validation process are complete. The next work is weighted/narrow
operator and input profiling plus graph-family baseline measurements. A new
`benchmarks/two_fe_dense/broad_milestone.json` fixes the intended performance,
control and science inventory; its harness extensions and wider scoreboard
remain under development. Preserve the unrelated STATUS/allocation-manifest
changes and untracked historical artifacts. No installed package or remote
state has changed.

## Completed narrow/weighted two-FE development screen — 2026-09-20

The [weighted/small-width report](benchmarks/reports/development/two-fe-weighted-small-local-2026-09-20/README.md)
records measurement before guard changes and the resulting initial Schur
admission extension. At least eight workers may now attempt 3–15 actual RHS
columns with unit or supplied weights, retaining the actual-weight probe,
memory admission, certificates and continuation. Wider/thread policy and
weighted loading, final projection and output-recovery guards remain unchanged.
[ADR 0076](docs/adr/0076-narrow-weighted-ols-schur-admission.md) describes the scope.

All nine ordered unit/AW/PW fitting/inference cells beat stock fixest in all
three pairs; median paired ratios span 0.634218–0.900552. Native fitting improves
about 25–42%. The three shuffled holdouts improve over native baseline but still
lose to fixest. Weighted full outputs improve only about 12% and remain about
43% slower than default fixest. Default reference outputs miss the unchanged
tolerance; separate tight-reference comparison and seven independent weighted
output audits pass. Sparse/dispersed-weight rejection costs of 7/8 ms remain
explicit, as does the one-FE unit control's 74→76 ms change.

Candidate plugin `1e0bf355e5e9cd9b37494386ee6808c33122dffb4aac699122517beb12ecffdd`
matches all 514 captured source entries on base `b84cea02` plus overlays. Twelve
focused solver checks, the actual-plugin ABI check and three Stata fixtures
pass. All 21 control result bundles/allocation peaks are exact; none of 70
native pairs raises peak allocation. The archive preserves all 289 application
attempts, including two harness worker failures and the failed oracle/reference
comparisons. This is a local development checkpoint, not universal speed or
release evidence. The timing work has finished; no installed package or remote
state changed. Further work should address narrow weighted operator cost,
weighted strict final projection/recovery and cheap rejected-probe handling.

## Completed local dense two-FE optimization — 2026-09-20

The [final dense two-FE report](benchmarks/reports/development/two-fe-dense-final-local-2026-09-20/README.md)
closes the frozen local 10% margin target. Both independent five-pair blocks
pass all 24 cells: twenty primary inference/output requests and four fresh-seed/
order holdouts at N=983,040, K=10/50 and sixteen workers. Median paired
fereg/default-fixest ratios span 0.356625–0.891304; every cell wins ten of ten
pairs (240/240 total). All forty-one regression controls pass, with maximum/
geometric-mean ratios 1.034483/0.995921 and no confirmation runs.

The retained source extends eligible multi-FE numeric loading/RHS/moment reuse,
independent outcome overlap, narrow unit-weight Schur admission, strict final
probe reuse, counted graph edges in idle scratch, bounded certificate reduction,
certified graph recovery and final-projection/bread overlap. Guards, independent
certificates, memory admission, original error ordering and fallback policies
remain. [ADR 0075](docs/adr/0075-dense-two-fe-ols-data-and-solver-passes.md) records
the development decision and its scope.

Final plugin `4e8e640ec2fb50ec1d7ca8b07066281a3be531ebdda2887fa1d72ccdcc31599e`
uses baseline `637f2407` plus the recorded overlays. All 513 captured source
entries match. Twelve Schur, seven outcome-overlap and two graph-recovery tests,
actual-plugin ABI checks and three completed-artifact Stata fixtures pass.
The archive contains 742 successful application calls. All 123 checked regression
result bundles and native allocation peaks equal baseline. All 24 final cells
repeat their checked native outputs exactly across ten calls.

Default fixest saved outputs miss the unchanged output tolerance in 80 final
comparisons; those discrepancies remain explicit. Eight separate tight-reference
output cases pass, as do independent group, normalization, reconstruction and
normal-equation audits. Default-reference speed and tight-reference accuracy
remain separate evidence. The previous failed shuffled candidate and every
stopped/failed attempt remain in the linked historical reports.

The completed campaign is `target/two-fe-dense-final-20260920`. It is local Mac
development evidence, not a claim for every graph, weight, width, thread count,
PPML or platform. No installed package, remote state or release qualification
changed. There is no pending timing or regression gate for this local target.

## Completed local one-FE optimization — 2026-09-20

The [final input-loop report](benchmarks/reports/development/one-fe-input-unroll-local-2026-09-20/README.md)
closes the frozen local comfortable-win target. Both independent five-pair
blocks pass all twenty one-FE OLS cells (K=10/50, IID/robust/one- and two-way
clustering, saved FE and full outputs). Median paired fereg/fixest ratios span
0.413353–0.844278; all cells win at least nine of ten pairs, with 199/200 total
wins. No outlier is removed. The earlier failed widecore block remains recorded.

The retained C row loop handles four SDK reads per main-loop iteration and a
scalar tail, preserving every call, immediate error check, coordinate/type/
main-thread guard, ordering and destination. It adds no allocation or scientific
arithmetic change. In the five-pair incremental narrow screen, input medians
fall from about 69 ms to 48 ms; IID improves 126→103 ms and full outputs
186→166 ms. The preliminary pragma-only variant is archived but was never
timed. Shared-loader eligibility remains unchanged; this is not evidence for
all paths, PPML or other platforms.

Final plugin `4437ffac11ed4018ced4cadf934d82cbe9bc09eaaa44505d446d50de5e86abb7`
uses base `6f3fd059` plus the C bridge and exhaustive mock-test overlays.
All 512 source entries match the frozen artifact and working production files.
The report contains 616 successful measured application calls, 303 exact
nonself native comparisons and 109 byte-identical saved-output bundles.
All 270 parent allocation peaks are identical; no control allocation peak
increases. Actual ABI checks, UBSan input/output guards and three licensed
Stata oracle/transaction fixtures pass. No scientific rule was weakened.

All thirty-one original-baseline regression cells pass: breadth maximum/
geomean 0.966234/0.616879; size/thread/group-layout 1.010582/0.947205.
No confirmation run was needed. This completed target supersedes the pending
prototype/final-block work described in the earlier checkpoints below.
No installed package, remote state, release or qualification changed.

## RHS profiling checkpoint — 2026-09-20

The [RHS diagnostic report](benchmarks/reports/development/one-fe-rhs-parallel-local-2026-09-20/README.md)
preserves the rejected parallel row-map/code-validation prototype. Eleven RHS
tests, three Stata fixtures and ABI checks pass, but five narrow IID pairs give
125/126 ms before/after and only one candidate win. Median native time improves
only 0.4 ms. The prototype was removed; its complete patch and all thirty
screen calls remain. Late native/fixest output spikes were not discarded.

Eight exact diagnostic calls on the accepted ordered-factor source split the
remaining RHS setup cost. Narrow finite checks take 1.11–1.13 ms, row map/count
0.94–1.12 ms, categorical code/value checks 0.82–0.90 ms, exact matrix matching
1.24–1.49 ms and FE-descriptor construction 1.22–1.29 ms. Frame checks are
negligible. Wide exact matching costs about 8 ms. The report contains 38 total
application calls, fourteen exact nonself native comparisons and five exact
saved-output bundles. Diagnostic instrumentation is isolated from production.

The goal remains active. Production core is the committed ordered-factor
checkpoint. A separate uncommitted C-only loading prototype asks Clang to
unroll the inner numeric SPI row loop fourfold. Inspection of the parent
assembly confirmed one loop-control sequence per value. All SDK calls, their
order, prevalidation and first-error return remain. Expanded UBSan mock tests
cover widths 1–64, both observation boundaries and failure at every value;
input/output guards pass. Its isolated `inputunroll` artifact build and timing
are pending. No gain is claimed. The full twenty-cell breadth refresh and both
final acceptance blocks remain pending for the eventual candidate. No installed
package, remote state or qualification changed.

## Ordered numeric factorization checkpoint — 2026-09-20

The [ordered-factor report](benchmarks/reports/development/one-fe-ordered-factor-local-2026-09-20/README.md)
archives 144 application calls. Numeric inputs with at least 16,384 rows can
encode verified ordered runs directly, preserving canonical level bits and
codes. The first inversion returns to the existing factorizer without
allocation. This shared core applies to OLS and PPML; timing evidence here is
one-FE OLS on Mac. Narrow IID medians improve 128→125 ms (four of five parent
wins); wide IID improves 371→367 ms (three wins). Narrow full outputs show no
incremental gain. Initial wide robust/clustered slowdowns remain recorded;
a separate phase run gives robust 492/492 ms and clustered saved FE 602/599 ms,
with preparation reduced by about 1.0/2.9 ms. This is not a universal speedup.

Final plugin `2538f103891fea59cb3af258149390794e013e90b4e47b831e2b2b6c72f1d58b`
passes eight focused factorization tests, three completed-artifact Stata
oracles/transaction fixtures and actual ABI checks. Sixty-one nonself native
comparisons and fourteen saved-output bundles are exact. All twenty-eight
incremental/phase allocation peaks are identical. Eleven size/thread/layout
controls pass: maximum/geomean 1.025825/0.992193. The current candidate's full
twenty-cell breadth regression and both final acceptance blocks remain pending.

A separate 264-call kernel diagnostic finds ordered dense groups at 2.195→1.175
ms on sixteen workers. A last-row ordering violation adds about 0.5 ms before
fallback (paired kernel ratio 1.233); this explicit tradeoff is preserved, with
no corresponding command-level late-violation timing claim. The premature
fixture attempt and standalone diagnostic build setup failure are disclosed.

The goal remains active. A separate uncommitted prototype parallelizes legacy
RHS row-map and categorical-code validation on existing teams of at least
eight workers and 16,384 rows, preserving the first diagnostic. Its eleven
focused RHS tests pass; artifact timing and adoption are pending. It does not
revive the rejected finite/matching fusion. No installed package, remote state
or qualification changed.

## Checked OLS output-store checkpoint — 2026-09-20

The [output-store report](benchmarks/reports/development/one-fe-output-store-local-2026-09-20/README.md)
archives 317 calls. Full-sample OLS numeric publication now batches borrowed
columns across one Rust/C call, retaining the checked SDK accessor, main-thread
confinement, scalar row/column order, first-error behavior and COMPLETE-last
transaction. The guard requires at least 4,096 rows, no drop-reason output and
at most 64 columns; the real adapter enables it only on macOS. Partial samples
and other adapters retain their prior path. No heap owner or public API changes.

Final plugin `76f8ec6018fa047fc1277880120502dc9499a9f4deba72ee900279a359d1161e`
passes final-source publication/failure tests, UBSan bridge guards, actual ABI
checks and three Stata oracle/rollback fixtures. All twelve incremental pairs
win against `widecore`, with identical native allocation peaks. Full-output
medians improve 196→192 ms (K=10) and 470→459 ms (K=50); saved FE improves
150→148 and 408→403 ms. All 31 regression cells pass: breadth max/geomean
0.969231/0.669356, size/thread/layout 1.030303/0.989931. The archive contains
141 exact nonself native and 51 byte-identical output comparisons.

RHS finite/match fusion was removed after its five-pair narrow confirmation
showed 127/127 ms and only two wins. Pure-ado deferred output initialization
remains unadopted after slower partial-sample results. Rejected patches, runs,
fixture-setup failure and correction are preserved. Production ado is unchanged.

Eight exact diagnostic calls show narrow SPI reads at about 69 ms, overlapped
consumer work at 39–40 ms and negligible producer waiting. Preparation takes
9.6–11.5 ms versus concurrent raw moments 7.1–7.6 ms; improving moments alone
would leave preparation on the critical path. Next test fewer passes for
ordered numeric FE factorization, retaining canonical codes, finite/error
behavior, memory admission and the unordered fallback. This is not implemented
yet. The goal remains active; neither final acceptance block has run for this
candidate. No installed package, remote state or qualification changed.

## Wide one-FE overlap checkpoint — 2026-09-20

The [wide-overlap report](benchmarks/reports/development/one-fe-wide-overlap-local-2026-09-20/README.md)
archives 442 calls. Existing independent-outcome and residual/bread/score
scheduling now extends through 64 generated columns, retaining the unit-weight,
one-FE, fast-mode, large-sample and eight-worker guards. No new heap owner,
changed arithmetic, certificate or error order is introduced. Incremental wide
medians improve IID 389→370 ms, full outputs 481→467 ms, robust saved FE
545→521 ms and two-cluster saved FE 626→608 ms. Allocation peaks are unchanged.

Final plugin `ee6eb5df6ef656b6765dea5bfc5b016b4154ffb6ed9395833236649bc34e20e1`
passes real-plugin ABI checks, the two final-source wide tests and three Stata
fixtures; eleven tests also pass on the preceding combined experiment.
There are 221 exact nonself native and 84 byte-identical output comparisons.
All 31 original-baseline regression cells pass: breadth max/geomean
0.976744/0.677801, size/thread/layout max/geomean 1.015692/0.992741.

The first frozen five-pair block wins all 100 comparisons but fails the 0.90
margin in narrow IID (0.907143) and narrow full outputs (0.917051). Wide full
outputs passes at 0.898855. The failed block and its original plan are retained;
block B was not run. The goal remains active and the margin is not weakened.
The next candidate needs new final blocks. Current narrow phase profiles put
legacy descriptor/RHS validation at 5.3–5.6 ms versus 1.3–1.7 ms for categorical
saved fits. Investigate redundant checks there, then output-pass opportunities,
without silently reviving prior rejected implementations. No next optimization
is implemented yet. No installed package, remote state or qualification changed.

## Immutable prepared owners checkpoint — 2026-09-20

The [validated-owner report](benchmarks/reports/development/one-fe-validated-owners-local-2026-09-20/README.md)
archives 362 calls for checked, immutable numeric-sample and topology owners.
Constructor validation remains complete; internal categorical consumers reuse
its proof without rescanning unchanged values or membership. Public validation,
working-response/weight checks, metadata, certificates, memory admission and
transactional publication remain intact. Numeric ownership applies across
categorical OLS and PPML preparation; direct borrowed-descriptor reuse requires
one noninteraction topology source and one term. Ordinary unsaved numeric OLS
uses the legacy representation and is a control for this increment.

Final `topologyowner` plugin:
`4339b23d7ff3249544ccd600751231139f21412675c43bee0535ae1a6d3f6366`.
Twenty-four focused Rust tests, nine licensed Stata fixtures and actual-plugin
ABI checks pass. Incremental screens show wide full outputs at 496/481 ms
before/after the numeric owner; narrow robust and two-cluster saved FE at
171/164 and 227/219 ms before/after the topology owner. The latter has no clear
wide-model gain. All 44 incremental/profiling native and output comparisons are
exact, and all 60 breadth comparisons match the prior `recoverycore` checkpoint
exactly, including 24 saved-output bundles.

The current twenty-cell screen wins 60/60 fixest pairs and meets the 0.90 margin
in 19 cells. Wide full outputs remain at 485/525 ms, paired ratio 0.927342.
A background Time Machine/Finder/UI CPU snapshot accompanies timing spikes;
all runs remain recorded and this screen is not a final acceptance block.
Breadth regression passes against original `normonly` (max 1.019851, geometric
mean 0.689816). All eleven size/thread/group controls also pass (max 1.046178,
geometric mean 1.002524), with 33 exact native comparisons. Original-baseline
saved-FE fit/certificate residual differences predate this change; the report
preserves the failed full-array audit and its exact prior-checkpoint adjudication.
Coefficients, covariance, samples, statuses, DoF, pass flags and saved bytes
remain exact against the original baseline.

The goal remains active. Neither final five-pair acceptance block has run.
Next measure wider independent-outcome and residual/bread/score overlap: both
current width guards stop at 16 generated columns, and residual overlap also
has a 16-slot anticipated-status array. Preserve numerical/error order and
memory allowance, then measure broad effects and controls before retention.
No widening is implemented. Raw-residual output substitution was not implemented
because its negative-FMA sequence differs from the positive-FMA predictor and
subtraction. No installed package, remote state or qualification changed.

## One-FE DoF validation checkpoint — 2026-09-20

The [DoF validation report](benchmarks/reports/development/one-fe-dof-reuse-local-2026-09-20/README.md)
archives 284 distinct calls, 102 nonself exact native scientific comparisons
and 39 byte-identical saved-output comparisons. Categorical absorbed-DoF now
reuses the same immutable single-source topology proof. Eligible narrow,
unit-weight, fast categorical IID/robust fits overlap allocation-free numeric
and topology checks with fitting, preserving error order and later metadata
allocations. Saved clustered/weighted calls use the duplicate-check removal;
ordinary unsaved one-FE commands remain controls for this increment.

Final `dofcore` plugin:
`8ef3246e53db3385580c6bff3072498155d436f0cf7783707cf084371171218a`.
Six focused Rust tests, five final-artifact Stata fixtures, the added robust-
saved explicit-dummy oracle and real-plugin ABI/panic checks pass. Narrow
saved-IID overlap medians are 160/156 ms before/after; robust saved FE is
178/172 ms against the preceding committed core. Clustered saved gains are
small; wide controls show no clear increment. The sparse 95%-missing control
passes at 149/147 ms with exact scientific/output results.

The current artifact's eleven size/thread/group-layout controls now pass
against frozen `normonly`: maximum median paired cell ratio 1.021705,
geometric mean 0.995187. These are cumulative current-core controls.
Final full-output medians are 204/222 ms (K=10) and 500/535 ms (K=50) versus
fixest, with six wins but median paired ratios 0.918919/0.934579. The 0.90
margin target is still unmet. The current artifact's complete twenty-cell
breadth regression screen and both final five-pair acceptance blocks remain
pending; the previous full breadth screen belongs to `recoverycore`.

The goal remains active. Next investigate retaining the already constructed
independent raw residual for saved-output construction, which currently scans
the same regressors again; explicitly verify target rounding and the longer
owner lifetime before reuse. Also audit constructor-enforced immutability as
a basis for reusing prepared validation beyond one stage. Neither broader
change is implemented. No installed package, remote state or qualification
changed.

## One-FE certificate and recovery checkpoint — 2026-09-20

The [certificate/recovery report](benchmarks/reports/development/one-fe-certificate-recovery-local-2026-09-20/README.md)
archives 338 calls, 74 exact native scientific comparisons and 62 byte-identical
saved-output comparisons. The retained core extends the fixed-group certificate
reduction to eligible narrow one-FE tables, reuses a same-owner immutable
validation receipt through saved recovery, and omits unused iterative buffers
on direct recovery. The final `recoverycore` plugin is
`b2cbf8bd9b1ae9fdd130ee1300e3c9ee853e13366985df3f78c93c29d111fe6a`.
Its 22 focused Rust tests, five Stata fixtures and real-plugin ABI checks pass.

The complete twenty-cell three-pair breadth screen wins all 60 comparisons
against fixest; eighteen cells reach the 0.90 median paired threshold. Full
outputs remain at 0.946667/0.907579 for K=10/50 (command medians 209/225 and
491/541 ms). Robust medians are 144/172 and 509/884 ms; two crossing clusters
are 183/253 and 574/1337 ms. Against the original fereg baseline, the twenty
breadth regression cells pass with maximum paired cell ratio 0.992288 and
geometric mean 0.697002. These are cumulative campaign gains.

Native missing-output initialization was rejected and removed. The 95%-missing
control slows from 148 to 159 ms (median paired ratio 1.081633), despite passing
its scientific, missing-store failure, transaction and old-plugin rejection
checks. The earlier half-missing control's five-pair follow-up is only 0.65%
slower, illustrating why the sparse boundary mattered. Original Stata output
initialization remains in production. Rejected source and all runs are retained.
A collector label-prefix collision was fixed before archiving controls; the
archive contains 22 distinct half-missing runs, with the erroneous draft local.

The goal remains active. Both final five-pair acceptance blocks and the current
candidate's eleven size/thread/group controls are pending. The next concrete
opportunity is reusing the same immutable topology proof in categorical DoF
construction, which currently repeats validation after prepared validation;
then assess overlap of remaining categorical DoF validation with fitting.
No installed package, remote state or cross-platform qualification changed.

## Inference residual overlap — 2026-09-20

The [inference/output report](benchmarks/reports/development/one-fe-inference-residual-output-local-2026-09-20/README.md)
records 108 primary calls plus twelve filtered-output controls. Retaining a
sandwich score cache no longer excludes eligible narrow one-FE fits from the
existing residual/bread overlap. Packed and strided owners remain immutable;
error order and memory admission are preserved. Exact-fit/error tests, observed
allocation bounds, 13 robust and 10 clustered tests, and four Stata fixtures pass.
The `inferenceresidual` plugin is
`eb29e5702a19c9e855375f3d56620a896eb2263d0adc4fc8f814e25de6bc641c`.
Three-pair medians are robust 144/170 ms, FE cluster 145/180 ms, two crossing
clusters 186/248 ms and two clusters with saved FE 241/330 ms versus fixest.
Wide controls are outside this change's guard. Native science exports are exact.

The no-fill temporary-output experiment saves 3–15 ms on complete full-output
pairs but adds about 16 ms to the wide half-missing control (453/469 ms).
It remains uncommitted experimental code, pending a way to initialize excluded
rows in the existing native publication pass. Its five corrected Stata fixtures
pass, including missing/weighted/restricted samples and all output rollback.
Two fixture setup errors using inadmissible zero FW/PW values are preserved.

The narrow certificate reduction and saved-recovery validation receipt are
later experiments outside this checkpoint. The goal remains active; final
acceptance blocks and the complete regression matrix are pending.

## Categorical validation placement — 2026-09-20

The [categorical-validation report](benchmarks/reports/development/one-fe-categorical-validation-local-2026-09-20/README.md)
archives 187 calls, 99 exact native comparisons and 85 byte-identical saved
output comparisons. Moving categorical IID/robust DoF validation into the
existing team is retained as `teamonly`, plugin
`fdc11d54843a7de44c9047c6ea9e7d7489d8281fd0eb2a4f482b5ed5304bf35c`.
Three rotated repetitions show wide saved FE at 437 versus 449 ms previously
and 528 ms for fixest; wide full outputs at 507/528/536 ms. Narrow full outputs
remain close at 216/224/218 ms. Four final-artifact Stata fixtures pass.

Parallel membership validation, the complete-sample row-map shortcut, batched
missing-initialized output allocation, sample-marker reordering and omission
of `buildfvinfo` were not adopted. Their source identities, measurements and
relevant focused tests are preserved separately in the report. The older
statistics/topology section's proposed membership experiment is now resolved.

The goal remains active. Residual/bread overlap with a retained inference score
cache and no-fill private output allocation are subsequent uncommitted work,
not part of this milestone. No final acceptance blocks or full regression
matrix have run; installed packages and remote state are unchanged.

## One-FE statistics and topology ownership — 2026-09-20

The [statistics/topology development report](benchmarks/reports/development/one-fe-statistics-topology-local-2026-09-20/README.md)
archives 162 further calls and 54 exact native scientific comparisons. Saved
output arrays are byte-identical in all 36 applicable comparisons. Three rotated
triplets per cell show narrow saved IID at 175/216 ms versus fixest and full
outputs at 221/220 ms (`topologytransfer`). The subsequent `inferencetss` artifact
records narrow robust at 156/162 ms, FE clustering at 159/179 ms, two crossing
clusters at 197/253 ms and two clusters with saved FE at 250/331 ms. Wide gains
in these increments are small or absent; all original observations are retained.

The changes transfer unchanged single-source categorical codes/membership and
extend the independent outcome-statistics overlap to saved IID, robust and
clustered one-FE fits under the narrow/unit/fast/team guard. Inference reuses
only the existing independently computed unit residual sum. The final candidate
is `inferencetss`, plugin `ded173ab46c8cdda8da33e02345f2498afa90143129d2f8d4f59e717d8cb29e1`,
based on `84905c67` plus eight verified production overlays. Focused native
checks and four final-artifact Stata scientific/prediction/transaction fixtures
pass. The report records source and artifact identities and the resolved
compile failure preceding the final build.

The goal remains active. Full outputs are close rather than comfortably ahead;
narrow robust and ordinary IID also need stronger margins. The next local
experiment parallelizes exact FE-membership validation while preserving the
first-error order. That uncommitted experiment and its diagnostic timers are
outside the `inferencetss` milestone. No new final acceptance blocks or complete
regression matrix have run, and no installed-package or remote state changed.

## One-FE breadth optimization — 2026-09-20

The [development milestone](benchmarks/reports/development/one-fe-breadth-improvements-local-2026-09-20/README.md)
broadens owned-input/RHS/moment reuse to all OLS inference, reuses FE codes when
adding clusters, accelerates certified one-FE saved output and public numeric
prediction, and reuses strided score products. The owner goal and predeclared
endpoint in `benchmarks/one_fe_breadth/acceptance-20260920.json` remain active;
the goal is not achieved.

Three-pair K=50 native/fixest medians are 509/867 ms for robust, 487/921 ms
for FE clustering, and 580/1308 ms for two crossing clusters (`scorecache`).
The later `savedprep` artifact records saved IID at 190/211 ms for K=10 and
461/542 ms for K=50. Full outputs remain the bottleneck: 240/224 and 524/532 ms.
Historical measurements retain their actual source and artifact identities.

The committed code is the `breadthmilestone` source snapshot, plugin
`f413c53e847d3a3f09794ee247c635257f1728285a8595d2040347b34ee9c246`, including the
reviewed restoration of the generic pruning mask's early release. All 511 source
files and production overlays were verified. Four Stata fixtures and the focused
construction-allocation test pass on this final artifact; eight paired follow-up
checks pass against `savedprep`. The complete report archives 288 development
calls. Earlier native checks cover 72 topology tests, 128 preparation scenarios,
weighted strided arithmetic, loaded-row fallbacks, exact budgets and fast/low
inference equivalence. All eleven three-pair size/thread/group-layout controls
against `normonly` pass on `savedprep`: geomean 0.995912, maximum 1.022222.

Source inspection corrected the prior explanation of saved-FE cost: automatic
one-FE absorption already omitted initial coefficient products, but the request
flag blocked fresh-projection and descriptor shortcuts. A checked output batch
was removed for negligible gain. Direct Mac stores failed the actual Stata
oracle despite passing the mock ABI check and were rejected. Their local failed
artifacts remain under `target/one-fe-breadth-optimization-20260920/`.

Next: reduce categorical saved-FE preparation/recovery passes and extend the
independent outcome-statistics overlap where eligible. Narrow full outputs and
comfortable margins in the narrow ordinary/inference cells remain open. The
final two five-pair blocks and complete final-source regression blocks have not
run. No installed-package replacement, push, release or cross-platform
qualification occurred.

## One-FE inference and output breadth — 2026-09-19

The [local breadth comparison](benchmarks/reports/development/one-fe-inference-output-breadth-2026-09-19/README.md)
times unchanged source `82e23357` against pinned fixest 0.14.2 for ten request
types at 983,040 rows, K=10/50 and sixteen native threads. All 120 primary calls,
28 explicit-dummy oracle calls and twelve native phase traces pass. Source,
artifact, input, sample, full b/V, DoF and requested-output evidence is archived.

At K=10, fereg/fixest medians are 208/189 ms for clustering at the FE level,
254/268 ms for two crossing clusters, and 291/220 ms for IID plus a saved FE.
At K=50 the same cells are 662/975, 748/1312 and 757/528 ms. The full saved FE,
residual, xb and xbd bundle takes 368/226 and 928/538 ms. Wide robust/clustered
fits win here; narrow one-way inference and saved IID remain clear gaps.

The loading/RHS pipeline's public guard currently excludes robust, clustered
and saved-FE requests. Saved calls also choose categorical preparation and
retain initial FE products for all RHS columns; traced K=50 initial absorption
rises from 37 to 207 ms. Suggested next work is certified one-FE final-target
saved recovery, then carrying owned loading/RHS into these fit entry points,
topology/inference pass reuse and output/prediction pass reduction. Guards for
weights, changed samples, memory and multiple/slope FEs remain authoritative.
No estimator changes, installed-package replacement or qualification occurred.

## One-FE absorbed-slope investigation — 2026-09-19

The [local slope investigation](benchmarks/reports/development/one-fe-absorbed-slopes-local-2026-09-19/README.md)
isolates repeated serial certification in `absorb(g1##c.z)`. With 245,760 rows,
49,152 groups and 16 native threads, three-run fereg/fixest medians are
342/54 ms for ten controls and 864/193 ms for fifty. Both native projections
finish after one sweep, but the generic automatic path runs six certificates
per command. Diagnostic certificates consume about 68–71% of command time;
actual projection and owned-design construction are much smaller costs.
Thirty-eight application calls pass, including a small independent dummy oracle,
fifteen stock comparator pairs and exact instrumented scientific exports.
The next slope-specific boundary is direct one-partition projection with reused
group geometry, parallel work and preserved independent terminal certification.
No estimator change is adopted. This Mac diagnostic is separate from the
intercept-only one-FE optimization and the light campaign's multiway slopes.

## One-FE optimization implementation — 2026-09-19

**The owner-approved local Mac endpoint is achieved.** The
[final unit-norm report](benchmarks/reports/development/one-fe-unit-norm-win-2026-09-19/README.md)
records two preselected independent five-pair blocks for ordinary one-FE,
unit-weight IID OLS with 983,040 rows and 16 native threads on the M2 Ultra.

| Regressors | Block A fereg/fixest | Block B fereg/fixest | Median paired reductions |
|---|---|---|---|
| 10 | 131/139 ms | 129/142 ms | 6.5% / 9.2% |
| 50 | 393/465 ms | 395/456 ms | 15.1% / 14.3% |

Fereg wins all twenty pairs. Every block/width has median paired ratio <=0.95;
all eleven protected size/thread/group-layout cases pass, with maximum ratio
1.015915 and geometric mean 0.988444. No control recheck is required. The final
archive contains 146 development calls plus forty final calls, 126 byte-identical
native scientific export comparisons, four focused native test executions,
three candidate ABI checks and nine licensed fixture passes. The final artifact
has its own three scientific/transaction fixtures. All 509 source-manifest
entries and its plugin SHA-256 are verified; reported native peaks are unchanged.

The final change specializes the one-column unit norm while preserving the
scaled recurrence, row order, finite-value errors and norm bits. The final
candidate is `normonly`, plugin SHA-256
`0ac6a0449cff224f02d6d7b7b7696b79fe6c6d182d89e3b406b4ea26a761e9fa`.
Its source identity is base `78672b85` plus the explicit borrowed_map.rs overlay.
Source and artifact identities, the acceptance plan and exact numerical receipts
are in the final report. The isolated build remains under
`target/one-fe-optimization-20260919/normonly/`.

The committed path to this result includes guarded Mac input access, a bounded
Rust-owned input/RHS pipeline, sample and topology reuse, and overlap of
independent preparation, statistics and certification work. Important adoption
receipts are in `one-fe-accessor-adoption-2026-09-19`,
`one-fe-pipeline-adoption-2026-09-19`, `one-fe-loaded-rhs-2026-09-19`,
`one-fe-input-completion-2026-09-19` and `one-fe-residual-start-2026-09-19`
under `benchmarks/reports/development/`. Earlier unsuccessful final blocks and
rejected parser, projection, metadata, validation and mean-scan experiments
remain in their historical reports; no result was rewritten or substituted.
The original local profile's fereg medians were 223 ms and 593 ms; the final
win claim uses only the fresh paired blocks above.

This completes the bounded development goal on main. Scientific contracts,
Stata/Rust ownership boundaries and the pinned fixest reference are preserved.
No installed-package replacement, push, remote run or release qualification
occurred. These results concern ordinary one-FE IID OLS; the separate slope
investigation and other release work retain their own status and evidence.

## One-FE pass and loading-overlap experiment — 2026-09-19

The owner-requested [pass/overlap experiment](benchmarks/reports/development/one-fe-passes-local-2026-09-19/README.md)
is complete. Five rotated wide comparisons measure stock fereg at 582 ms,
parallel finite-value validation at 562 ms, validation plus the earlier guarded
fast accessor at 542 ms, and those changes plus pipelined input conversion at
522 ms. Fresh pinned fixest comparisons take 456 ms. The combined prototype's
10.3% paired improvement closes about 48% of the difference between command
medians; it does not win this case. Shuffled wide input retains the overlap
gain. Narrow input gains only a few milliseconds.

Production Rust source retains only the bounded parallel finite-value check
before initial one-FE projection. It uses an already installed worker team,
preserves validation/error precedence and adds no buffer. Installed artifacts
remain unchanged. Direct RHS initialization and concurrent source-moment
variants did not improve command time and were rejected.

The double-buffered 4,096-row input pipeline remains an isolated C transport
prototype. SPI calls stay on the Stata thread, and its joined helper converts
only owned copies. Its roughly 20–25 ms incremental wide-command gain survives
full loading/allocation timing. Production integration still requires Rust-owned
and admitted tiles, accounted worker lifetime and the resolved thread policy;
the diagnostic's 3.25 MiB wide scratch plus worker stack are outside the Rust
ledger. No one-thread pipeline performance or equal-memory claim is made.

Evidence includes 104 accepted regression calls with exact native scientific
exports, five fresh fixest comparisons, six licensed fixture passes across two
artifacts, three focused Rust tests, actual-plugin ABI checks and two UBSan
transport/failure tests. Source manifests, artifact hashes, rejected patches,
raw numeric timings and independent-oracle receipts are archived. This is
bounded local arm64 development evidence, not qualification. No remote job,
branch, commit, push or installed-package change occurred.

## Stata input accessor investigation — 2026-09-19

The owner-requested [accessor investigation](benchmarks/reports/development/stata-input-accessor-local-2026-09-19/README.md)
finds a modest local gain from a guarded numeric-batch `vdata` callback plus
callback caching and loop cleanup. Five rotated wide one-FE comparisons reduce
median input from 305.7 to 279.2 ms and command time from 582 to 559 ms;
median paired reductions are 8.8% and 4.5%. Narrow, single-thread and shuffled
controls retain smaller or comparable gains. The tested row-tile transport
loses its extra write advantage when conversion back to columns is included.

The evidence binds three arm64 diagnostic plugins sharing one unchanged Rust
archive to 36 accepted command calls, exact scientific exports, independent
dummy oracles, byte-level SDK comparisons, three licensed scientific/transaction
fixtures, actual-plugin ABI checks and UBSan guard tests. ASan stalled before
test entry; its overlapping kernel timing was excluded and rerun in isolation.
No ASan, Linux/Windows, release or new fixest comparison is claimed.

Production source and installed packages remain unchanged. The isolated
candidate saves roughly 20–30 ms on the wide command, so this optimization
alone does not close the fixest gap. Further material gains require reducing
later materializations or overlapping useful native work with main-thread
reads. Avoid a global SDK fast-mode switch; only the validated numeric batch
reader was tested. No remote job, branch, commit or push was created.

## One-FE OLS versus fixest profiling — 2026-09-19

The owner-requested [local profile](benchmarks/reports/development/one-fe-profile-local-2026-09-19/README.md)
is complete for unweighted, complete-data numeric IID OLS. Source `55c19cb6`,
stock Mac artifact `0c6d5966…ebb4d37`, and the exact fixest 0.14.2 archive are
bound to 152 accepted calls, independent explicit-dummy checks, exact native
instrumentation comparisons, nested phase timings and stack summaries. At
983,040 rows/50 regressors/16 native threads, stock command medians are
0.593 s versus 0.461 s. Fereg input loading is 0.307 s, native work through
precommit 0.215 s, and the remaining command boundary about 0.070 s. The wide
single-thread case already wins, 1.712 s versus 2.325 s; narrow cases still lose.
Fewer FE groups and shuffled-row controls retain the sixteen-thread gap.

The next optimization boundary is main-thread input transport, one-FE matrix
materialization/reference/projection passes, and repeated Stata preparation.
Finite regression is already competitive; no new solver or production
optimization was adopted. Failed/discarded diagnostic attempts remain explicit.
This is local development evidence, with uncontrolled macOS core placement,
not a release or cross-platform claim. No SCC job, push or installed-package
change occurred; the separate light campaign remains untouched.

Fresh universal Mac builds and actual-plugin ABI checks now work with the
separately installed Command Line Tools selected through process-local
`DEVELOPER_DIR`. The global Xcode selection/license remains unchanged. This
resolves the local build obstacle for this profiling workflow without revising
historical blocked-build receipts.

## Light competitor benchmark — 2026-09-19

The owner-approved [light harness](benchmarks/light/README.md) fixes ten logical
28-core SCC bundles, four- and 28-core comparisons, three fresh-process repeats,
concurrent process-tree PSS and a 12-hour limit per bundle. The inventory has
40 scientific configurations and at most 1,200 fits, including bottleneck width,
saved effects, clustering, slopes and the two Veneto panels. This is development
evidence, not release qualification or a restart of the stopped campaign.

Clean candidate source is `e23f15cad16c2f0b2368326aea579921a069ca9a`; the isolated
Linux artifact is `4ee87eee…cf4b0ea9`. Eight focused local checks pass. Original
startup job `7643293` passed every native explicit-design oracle but exposed
gtools capture defects and comparator numerical discrepancies. Its failed
receipt is preserved; no main fits ran. The single operational repair corrects
gtools link/residual capture and output precision, retaining all scientific
thresholds. Comparator discrepancies remain failed comparisons. Repair startup
job `7643437` passed all 24 supported adapter executions and all six native
explicit-design checks, then began Task 1. Eight comparator oracle discrepancies
remain explicit failures. Array `7643462.2-10` is submitted with concurrency
three while Task 1 runs, keeping the campaign total at four. After Task 1 is
terminal the array limit may rise to four; do not add tasks or repeat failures.

[Submissions and local evidence](benchmarks/reports/development/light-2026-09-19/)
record the attempts. The long Veneto raw-outcome inputs are prepared. The owner
subsequently omitted short Veneto; do not recover or rerun it. Its original
unattempted receipts remain preserved. Installed artifacts, dependency repositories and unrelated work remain
unchanged. No benchmark conclusions or final report are accepted yet.

The frozen measurement root is `20260919-light-e23f15ca/repair-1` under the SCC
fereg runs directory; runtime identity is `1ce57d0f…aaeecba`. Measurement bytes
must remain unchanged. Report/collection development is separate in `analysis/`.
Local reporting now uses median paired ratios, retains reference audit failures
(with only ADR 0026's ppmlhdfe exception), and exports native phase/allocation
diagnostics. Ten focused local checks pass. The LaTeX/vector-figure layout
compiles; final measured figures still require rendering and visual review.

The owner now allows 30 minutes per PPML regression while retaining a 12-hour
task cap. Eleven focused checks pass for the bounded continuation. One separate
dependency-held job `7643785` (after array `7643462`) selects original PPML command timeouts only, includes
fresh native controls and three repeats where executions succeed, and preserves
input bytes, numerical settings and original failures. Nonconvergence errors
and OLS timeouts are not retried. Collection/reporting keeps this continuation
separate; use `--omit-short --followup CONTINUATION` when collecting the final
report. No short-panel follow-up was submitted. The increase from 15 to 30
minutes was deployed and hash-verified before the held job started; the prior
unexecuted configuration is archived. The existing directory/terminal ID retains
its historical `ppml-15min` name. Eleven focused checks still pass;
[the amendment receipt](benchmarks/reports/development/light-2026-09-19/owner-timeout-30min.json)
records the new limit and updated monitor.

## Practical whole-action memory reporting — 2026-09-18

**ADR 0074's pragmatic memory milestone is complete for the calibrated scope.**
The [development report](benchmarks/reports/development/whole-action-practical-2026-09-18/README.md)
binds source snapshot `b58efa34…081d11`, Linux artifact `02bbde02…9b7714`,
unchanged inputs and pinned references. Schema 2 now publishes combined peak and
additional reserve when Stata-19 baselines and expected native stages are known.
Strict proof gaps remain visible; unknown versions, frames or attempts remain
partial. Admission, scientific calculations and routing are unchanged.

Stata factor-variable result posting explains the missing allocation growth.
Native execution and later posting use a phase maximum instead of summing
noncoexisting owners. All 16 fixed estimator cases meet the larger-of-10%-or-
64-MiB native/process target. Four publication holdouts pass on both Mac and
Linux. Replaying the preserved German 5% receipt predicts 4.425 GB versus
4.348 GB RSS (+1.77%); native prediction stays +3.77%. This is a reporting
replay, not another empirical estimation or a new-binary German fit.

All 150 native tests, 52 Python checks, Mac/Linux memory fixtures, Linux
export/actual-loader checks, inventories and repository checks pass. Nine
alternating scientific pairs are exact; no command or phase exceeds both
+5% and +10 ms. Job 7641143 completed substantive checks but its final wrapper
had an `_rc` typo (exit 2). Job 7641149 corrected only that invocation and
verified the preserved results: terminal pass, failed=0/exit=0. Both are
terminal; no estimator measurements were repeated.

**Next boundary:** freeze and package a Mac/Linux review candidate once Mac
linking is available. The Xcode license remains unaccepted. Strict construction,
allocator/runtime proofs and formal platform qualification remain separate;
this practical milestone does not promise ten-percent accuracy on arbitrary
workloads. Installed artifacts and CMG are unchanged; no push or release.
Unrelated STATUS history, untracked work and inventory overlays are preserved.
The earlier checkpoints below retain their historical scope.

## CMG expected heap and native calibration — 2026-09-18

Implementation commit `290a2ffb` separates expected native heap from conservative
CMG admission. The forecast uses actual retained capacities, hierarchy dimensions,
component factor fill and estimated construction scratch; runtime/stack headroom
remains admitted separately. Failed attempts, OLS retained parents and PPML
prepared/frame/checkpoint overlap retain independent peaks. Automatic routing,
scientific contracts and the CMG pin are unchanged. Explicit CMG still defaults
to components, as adopted in the preceding checkpoint.

The [development report](benchmarks/reports/development/cmg-expected-heap-2026-09-18/README.md)
binds isolated baseline/candidate source and artifacts, input hashes, all focused
tests and failed attempts. Native job 7639913 passes 205 Rust checks and Linux
export/actual-loader checks. Job 7639948 passes all twelve fixed native-memory
cases and 27 alternating scientific pairs, with no command or phase timing flag.
Four holdouts pass (7639985/7639998); the original width-harness mistake is retained
and only that holdout was corrected/retested. Local Python, inventory and
repository checks pass. Compiled source matches the tested snapshot.

CMG forecasts fall from 162.05 to 22.85 MiB (connected OLS), 396.36 to 18.65
(tiny components), and 140.38 to 13.81 (saved-output PPML). Observed peaks are
14.17, 13.98 and 9.45 MiB. These pass the declared absolute allowance; they are
not ten-percent-relative results or new solver-memory savings. The
[attribution](benchmarks/reports/development/cmg-memory-attribution-2026-09-18/README.md)
identifies worst-case level reservations and runtime allowances, not a defect
in low-degree elimination or component numerics.

The single unchanged German 5% native OLS check, job 7639994, retains exactly
4,361,733 rows. All scientific tables, saved columns and sample order match the
preserved native result exactly. Predicted native peak is 3,705,301,141 bytes
versus 3,570,575,744 observed (+3.77%); command time is 285.070 seconds, without
a paired speed claim. All accepted jobs have terminal sentinels and zero
scheduler failure/exit. No reference or full-sample estimation was rerun.

**Resume boundary:** native forecast calibration now passes all twelve fixed
cases, four holdouts and the unchanged German check. Whole-action reporting
remains partial: Stata publication/allocation growth and whole-process runtime,
stack and allocator uncertainty still need calibration. The German before/after
Stata allocated increase is 101,325,876 bytes, whereas the narrower publication
estimate is 4,628,641; these are different lifetime boundaries, now measured
explicitly. Combined peak/reserve stay unavailable; strict proof limitations
remain separate. Do not infer release readiness from the native gate.

The installed plugin and CMG checkout are unchanged. Mac linking remains blocked
by the unaccepted Xcode license. No push, issue edit, release or broad campaign
occurred. Unrelated STATUS content and historical receipts remain preserved;
the uncompiled CMG draft and working inventory overlays remain uncommitted.
The earlier checkpoints below retain their historical scope.

## Retained OLS FE product capacity admission — 2026-09-18

Code commit `ded4f95b` admits initial and combined FE coefficient products before
retained solver construction and checks their actual value/layout capacities at
handoff. Uncached robust/clustered score reconstruction charges the final
retained product. Exact and one-byte-short budgets, spare capacities, injected
allocation failures and repeated calls are covered. Arithmetic and scientific
policy are unchanged; corrected live-parent admission can reject a tight budget
or use an already supported certified fallback.

The [development report](benchmarks/reports/development/retained-fe-parent-capacity-2026-09-18/README.md)
binds clean and preserved-overlay source, synthetic inputs, test binaries,
inventories and scheduler receipts. Jobs 7628936 and 7628956 pass 412 Rust test
executions across those scopes, including 21 focused robust/clustered checks;
both have terminal sentinels and zero scheduler failure/exit. Five focused Python
checks, structural inventories and repository checks pass. Both initial
fixture-extraction failures are preserved. This is native development evidence,
not a new performance comparison, licensed Stata run or qualified candidate.

**Resume boundary:** the retained-product handoff is bounded, but construction
inside its producing solver is a separate obligation. Ordinary CMG ingestion,
remaining construction/transient frames and complete inference/recovery/
publication coverage still prevent a complete whole-action ratio. Coverage bits
remain conservative; estimated runtime headroom and the earlier CMG timing flag
retain their uncertainty. Mac linked validation is still blocked by the Xcode
license. Installed artifacts, issue #27 and unrelated jobs remain untouched;
no push or release. Unrelated dirty memory work remains preserved.

## CMG integration: published ownership APIs and estimated runtime policy

The published dependency is now CMG
`1d39c113f0c397f29653d92672090e5a42f80921`, with `parallel` and
`experimental-components` compiled. Production components remain disabled and
automatic routing/scientific thresholds are unchanged. The
[CMG integration report](benchmarks/reports/development/cmg-upstream-integration-2026-09-17/README.md)
binds coherent local commits, clean source `cae41abc`, the separate preserved
memory overlay, artifacts, inputs and reference hashes. The earlier CMG
shutdown draft is superseded by this published upstream integration; no CMG
repository modification or further upstream authorization is pending here.

Checked APIs now admit covered hierarchy/factor construction, prepared numeric
assembly, plans and fixed caller/workspace banks. Shared retained owners use one
capacity union. Each owned executor reserves the explicit estimated policy:
32 MiB pool heap, 2 MiB heap per worker and 8 MiB requested stack per worker.
Known handles are counted once; joined retirement retains the runtime charge.
This is engineering headroom, not a proven runtime/whole-process bound.

Clean-source SCC job 7626076 passes 178 native tests, Linux build/export and
actual-loader checks, and both licensed OLS/PPML smoke fixtures with explicit
sentinels. The preserved memory overlay separately passes 40 CMG and 137 plugin
tests in job 7626099. All 18 paired commands match scientific outputs,
samples/order, saved residuals and non-time solver diagnostics exactly. The
clean/overlay allocation suites pass 37/39 tests; ten dependency-closure tests,
reviewed allocation inventories and repository checks pass. All integration
jobs are terminal with scheduler accounting; original failed evidence remains.

Original command medians: narrow OLS .080/.082 s, weighted clustered OLS
.195/.202 s, PPML .254/.277 s. The PPML +23 ms/+9.1% flag is preserved;
FE projection carries its comparable phase flag. A single same-binary diagnostic
(job 7626155) on another node did not reproduce it: .610/.582 s and faster FE
projection, with exact science. Traced setup costs were nearly identical.
This does not prove the original difference was noise. Performance remains
uncertain and the original gate is not relabelled as passing.

**Resume boundary:** integration is implemented and numerically validated locally;
production acceptance remains open. Whole-action coverage/ratio are still
incomplete/undefined. Ordinary ingestion, remaining fereg-owned construction,
inference/recovery/publication and strict dependency/runtime proofs retain their
separate limits. Mac linked validation remains blocked by the unaccepted Xcode
license. No installed artifacts, issue #27, push, tag, release or stopped
campaign was changed. Earlier unrelated dirty memory work remains preserved.
The earlier dated checkpoints below retain their historical source scope.

## Candidate preparation: input, generation and worker lifetime accounting

Latest tested snapshot: `5240e58-generation-capacity4`; main remains
`5240e589400c888690666265263ad96f9b694aa6`. The
[PPML generation report](benchmarks/reports/development/candidate-ppml-generation-memory-2026-09-16/README.md)
records capacity admission for transferred raw inputs, factor/cluster-only
sources, original reporting slots and both simultaneous old/new prepared
generations. Every generation checks actual retained capacities before commit.
The strict-checkpoint bound also charges the specialized caller's fifth
K-sized active-identity bank. Initial generation construction now belongs to
the preparation timing/allocation window. Classification, scientific thresholds,
weight semantics, transactional replacement and public wire definitions remain.

Final job 7608507 passes 343 native checks, Linux build/export/actual-loader
checks and nine alternating fresh-process pairs / 18 licensed commands. All
scientific outputs, samples/order, saved outputs and non-time solver fields are
exact. All 18 explicit sentinels and 36 isolation checks pass. The job reserved
28 slots, used four physical execution cores, and finished in 290 seconds with
failed=0/exit=0 and 3.343G job maxvmem. All 39 Python checks, repository checks,
OLS v29 and action v14 inventories pass. These structural inventories alone do
not establish byte coverage. All checkpoint jobs are terminal.

Baseline/candidate command medians are 0.316/0.300 seconds for saved-slope
low-memory OLS, 0.107/0.108 for narrow PPML and 1.033/1.021 for weighted,
clustered PPML with saved effects. No command or comparable phase crosses the
5% AND 10 ms threshold. The preserved preparation flag is 0.07147/0.10903:
the candidate window newly includes the existing initial-generation constructor.
This is a measurement-boundary change, not an asserted 38 ms kernel slowdown.
The first two native peaks are unchanged; weighted PPML rises 5,011 bytes to
64,690,503. RSS, scratch, zero PPML fallbacks and per-pair observations are in
the report. No universal speed or whole-command memory-reduction claim is made.
The incremental baseline is unpublished `5240e58-worker-retirement1`.

The report preserves the original generation1 test-syntax failure and the
28-slot/32-host-CPU assertion failure, plus independently bound generation2/3
checkpoints. Production Rust/Stata/build files match the tested snapshot;
source, artifact, input and reference identities are retained. Full Rust
formatting is not claimed: the direct check still reports layout differences.
The installed universal plugin remains unchanged at
`4187efdfbd6235e63c369d9b42dede6381b8a9e50b5360fd63ee237bd6c11b08`.

Other completed, separately bound checkpoints from this continuation:

- [Input/metadata admission](benchmarks/reports/development/candidate-input-memory-2026-09-16/README.md):
  bounded string dictionaries and transport/compiler preflights; 183 final
  native checks and exact-science cost/diagnostic comparisons. Original timing
  flags and the bounded diagnostic that did not reproduce them remain.
- [CMG containers](benchmarks/reports/development/candidate-cmg-memory-2026-09-16/README.md):
  descriptor/Arc/spare-capacity and terminal-factor supplements; 221 native
  checks and 18 licensed commands, with no command/phase threshold crossing.
  This is not the full dependency-construction proof.
- [Worker retirement](benchmarks/reports/development/candidate-worker-lifetime-2026-09-16/README.md):
  native pool retirement joins workers; the isolated CMG shutdown draft passes
  its focused checks and 18 exact-science commands. The 319 reported native
  test executions include repeated plugin suites. CMG's production pin has
  not changed; authorization to modify that separate repository is pending.

**Resume boundary:** whole-action coverage remains incomplete and its ratio
undefined. The retained raw/prepared omissions and strict caller bank are fixed;
complete transport/ABI, construction, numerical/fallback/recovery, dependency/
runtime and core assembly lifetimes still require proofs. Bit 7 retains its
original strict/rollback meaning. The CMG draft is reviewable but unintegrated;
its integration is not by itself sufficient to clear every remaining bit.
Xcode licensing, full formatting, candidate freeze, universal/Rosetta and
Mac/Linux packaging/installation remain later gates. The named OLS
publication/recovery-parent item is resolved; complete recovery-frame coverage
and the combined P0 item remain open in the [roadmap progress](https://github.com/johannes-schmieder/fereg/issues/27#issuecomment-5706676076).
No empirical rerun, campaign restart, commit, push, tag, release or installed
package replacement occurred. Unrelated STATUS content and receipts survive.

## Candidate preparation: core PPML certification lifetime and masks

The [core certification report](benchmarks/reports/development/candidate-core-certification-2026-09-16/README.md)
records snapshot `5240e58-core-certification1`. It releases obsolete final-fit,
Hessian and prepared-generation owners before independent certification and
includes original/candidate-row and metadata validation masks in the checked
publication allowance. Sequential validators combine by maximum; concurrent
masks add. The earlier conservative parent reserve remains. Scientific rules
and public field definitions/wire layout are unchanged.

All 260 focused native checks pass (128 plugin, 46 memory, 30 result and 56
prepared estimator), including the new four-fixture ownership probe and
original/candidate-axis bounds. All 38 Python checks, repository checks and
OLS v26/action v9 audits pass. Job 7598585's original exit-126 receipt is
preserved: the snapshot copier lost the loader script's execute permission
after native tests and compilation passed. Mode restoration changed no bytes;
the one wrapper retry, job 7598655, verifies the identical plugin and passes
Linux export/loader checks and nine alternating pairs / 18 licensed commands.
Both use 28-core reservations with four physical execution cores. The retry
finished in 61 seconds, failed=0/exit=0. All checkpoint jobs are terminal.

Scientific outputs, sample/order, solver non-time fields and saved outputs
match exactly. Complete-command baseline/candidate medians are 0.319/0.296
seconds (saved-slope OLS), 0.124/0.126 (narrow PPML) and 1.053/1.033 (weighted,
clustered PPML with saved effects). No command or reported phase crosses the
5% AND 10 ms slowdown threshold. This incremental comparison uses unpublished
`5240e58-publication-lifetime4`, not committed main. Weighted PPML measured
peak rises from 64,688,484 to 65,723,031 bytes; no whole-command memory-reduction
claim is made. Source, artifacts, inputs, phases, RSS/scratch and zero PPML
fallbacks are bound in the report; historical failed receipts remain intact.

This closes the identified certification lifetime/mask defects, not the entire
allocation proof. Construction/decoder/string, numerical and generation
overlaps, nested dependency/worker lifetimes and remaining result assembly
still require coverage. Whole-action coverage and its ratio remain undefined.
The Xcode-license prerequisite, candidate freeze, universal/Rosetta and
Mac/Linux packaging/installation gates remain. The installed Mac artifact is
unchanged. No empirical rerun, campaign restart, commit, push or release occurred.

## Candidate preparation: PPML publication parents and terminal observation

The [PPML publication report](benchmarks/reports/development/candidate-ppml-publication-2026-09-16/README.md)
records a capacity inventory for certified results, including nested metadata
and separation witnesses. The plugin admits surviving action/result parents
plus shadows before shadow allocation, and checks capacities before stores.
The scalar planner also includes declared-channel slice descriptors. Original-N
sample-mask allocation and the final OS thread-count query are removed. The
private ABI trace now separately observes action return, including failures;
public base fields and numerical policies remain unchanged.

Snapshot `5240e58-publication-lifetime2`, job 7596013, passes 202 distinct native
checks (127 plugin, 30 result, 45 memory); the 127 plugin checks also pass with
the audit enabled. Tests include independent deallocation counts, spare nested
capacity, exact/one-byte-short admission, allocation-free validation/stores,
interruption, repeated calls and a 5,334-case sample-mask oracle. All 38 Python
checks, repository check and OLS v25/action v8 audits pass. Original failed job
7595986 and static-check failures are preserved. The installed Mac plugin is
unchanged; no empirical estimation or campaign restart occurred.

Final snapshot `5240e58-publication-lifetime4`, job 7596067, passes the final
127 plugin tests, Linux build/export/actual-loader checks and all nine paired
comparisons (18 fresh Stata commands). Scientific outputs, samples/order,
solver non-time fields and saved outputs match exactly. After the requested
28-core reservation change, it finished in 242 seconds, failed=0/exit=0, with
full-node isolation checked around every command and four execution cores.
The monitor has been deleted; all checkpoint jobs are terminal.

The saved-slope low-memory timing flag does not reproduce: complete-command
medians are 0.303/0.299 seconds, narrow PPML 0.123/0.122 and weighted/clustered
saved PPML 1.014/1.020. All clear the declared 5% AND 10 ms threshold; reported
phase medians also clear it. The fixed checkpoint's cost gate passes. The
baseline is unadopted `5240e58-action-coverage3`, not committed main. Native
allocation/RSS/scratch and zero PPML fallback counts are recorded in the report;
no universal speedup or complete action-memory proof is claimed. Superseded
job 7596058 was canceled before starting; all earlier flags remain preserved.

This closes the plugin frame after core PPML certification. Whole-action
coverage remains incomplete: decoder/string/construction, full numerical and
PPML generation overlap, dependency/worker lifetimes and core certification
still need proofs. The ratio remains undefined. The Mac Xcode-license
prerequisite, candidate freeze, universal/Rosetta validation, Mac/Linux review
package and installation gates remain. No commit, push, tag, release or
installed-package replacement was performed.

## Candidate preparation: shared lifetime ledger and FE parent guards

The [action-lifetime report](benchmarks/reports/development/candidate-action-ledger-2026-09-16/README.md)
records a fixed-size OLS/PPML ledger and explicit fast-fit/uncached-inference FE
coefficient parents. The former reserves initial/combined products before a
retained solver captures its budget; the latter charges the observed final
product capacity beside score reabsorption. Both translate the nested solver
peak back to the original execution base. These are conservative admission
changes, not a measured speedup or proof of a prior whole-action cap breach.

The private version-1 trace requires each model stage, sums simultaneous owners,
maxes sequential frames and preserves unknown coverage. Its observation is
explicitly precommit, not terminal publication. Public base-memory fields and
wire versions are unchanged. The allocation inventory now gives each missing
owner family a source location and an explicit proof obligation.

Snapshot `5240e58-lifetime-ledger4` passes 121 plugin unit/mock-SPI checks both
with and without the audit flag. All 930 emitted model records validate; every
incomplete ratio remains undefined. Its job 7590673 is still recorded as failed
because a new core-test namespace was wrong. Final snapshot
`5240e58-lifetime-ledger6`, job 7590739, passes 45 distinct focused core checks,
including exact/one-byte-short fit admission and unchanged fit/inference results.
Only test bodies differ between those snapshots; production tokens/literals
match. All 38 source-contract/inventory Python checks pass, preserving the
historical manifests and F0 pins (current OLS v24/action v7). Every failed build,
fixture and initial static-check receipt is retained. The installed Mac plugin
is unchanged, and all six bounded development jobs are terminal.

Whole-action coverage is still incomplete: compiler/string construction,
complete numerical/PPML generation overlaps, dependency containers, cold and
retiring worker pools, and terminal output/publication proofs remain. No new
performance comparison was run; the saved-slope low-memory timing flag remains
open. The Xcode-license prerequisite, freeze, Mac/Linux review packaging and
installation gates remain. No production adoption, commit, push, tag, package
replacement, empirical estimation or campaign restart is claimed.

## Candidate preparation: numeric construction and frame admission added

The [construction/frame report](benchmarks/reports/development/candidate-action-frames-2026-09-16/README.md)
records additional uncommitted OLS reservations for numeric dictionary growth,
construction topology/pruning/metadata, repeated slope-channel identities,
and finite/inference frames beside retained request/prepared owners. Dense
cluster tables are charged together with their parallel partials. Numerical
policies and public base-memory fields are unchanged. Whole-action coverage
and its ratio remain undefined; these envelopes are not complete byte proofs.

Snapshot `5240e58-action-frames3` passes 114 plugin tests and Linux binary/ABI
checks in job 7590380. All 27 alternating pairs (54 fresh Stata commands) over
nine fixed cases preserve exact science, samples/order, solver non-time fields
and saved outputs. Full-node scheduled occupancy is checked before/after every
command. A stricter VCE-specific allocator test passes separately in job
7590422; only that test differs from the measured production snapshot. The
35 source-contract and three inventory Python tests, repository checker and
OLS v23/action v6 audits pass. Original stale-manifest and local-toolchain
failures remain recorded. The installed Mac binary remains unchanged.

The original weak-graph aggregate native flag does not reproduce in diagnostic
7590338 or the full-node screen. Subphase timing remains variable. The latter
screen flags absorbed slopes (.329/.376 seconds) and saved slopes in low mode
(.533/.575). One bounded diagnostic, 7590402, reverses the absorbed-slope flag
(.377/.320); identical binaries shift by +53.7 ms in their fit phase. Saved-slope
low remains flagged (.509/.568), while identical-binary controls vary by -52 ms.
That is insufficient to attribute the discrepancy or accept the cost gate.
Every original failure/flag is preserved. No additional fit retry is scheduled.

The dependency audit identifies specific missing owners in CMG's principal-array
reports: hierarchy/workspace descriptor banks, graph Arc owners, some spare
capacity and its separately owned Rayon pool. These are not complete heap reports.
Remaining memory proof: those dependency/runtime lifetimes, string dictionaries
at SPI loading, decoder/compiler work before admission, retained FE coefficient
parent overlaps, and complete PPML parent/current-generation overlap. The local
Xcode-license prerequisite and freeze/package/install gates remain open. No
adoption, commit, push, tag, installed-package replacement, empirical rerun or
campaign restart is claimed. All this task's bounded jobs are terminal; unrelated
SCC jobs were left untouched.

## Candidate preparation: request/slope admission fixed; action proof still open

The [request-owner/slope report](benchmarks/reports/development/candidate-action-owners-slopes-2026-09-16/README.md)
records two additional uncommitted OLS fixes: exact nested request/output-name
capacities charged beside each execution/publication parent, and saved-slope
recovery admission for retained coefficients, LSMR and MAP. Topology validation
is checked before allocation; LSMR includes slope whitening and its retained
certificate frame. PPML route policy and public base fields are unchanged.

Snapshot `5240e58-action-coverage3` passes 289 focused native checks, including
independent allocator deallocation/peak checks, exact/one-byte-short budgets,
weighted known-span recovery, retained products, failure atomicity and repeated
calls. Native-test job 7590299 then fails in the separate build wrapper because
Cargo was invoked outside source; that receipt remains failed. One narrow
wrapper correction, job 7590309, passes binary/ABI checks and 21 alternating
Stata pairs (42 commands), with scheduler failed=0/exit=0 and a terminal sentinel.
All scientific matrices, samples/order, solver non-time fields and saved outputs
match. Native peaks, scratch and public base bytes match in every pair.
Thirty-eight Python checks, both inventories (OLS v22/action v5), the repository
checker and diff whitespace checks pass. The installed Mac artifact is unchanged.

The earlier slope command-time flag does not reproduce beyond the declared
median threshold in the serialized single-NUMA comparison (0.193/0.200 seconds,
with identical-artifact controls 0.229/0.220). A concurrent-build noisy attempt
is preserved as inconclusive; exclusive scheduler core ownership was not verified.
The combined prototype's seven command-time medians all pass, including slopes
0.132/0.136 seconds and saved slopes in fast/low modes. A separate tiny-weighted-
bridge native-phase shift (+12.6 ms) remains explicit despite only +5 ms in the
complete command. Its branch has no numerical or solver-policy changes, but
phase attribution is not proven; no further ambiguous retry is scheduled.

Remaining: compiler/factorization construction, complete finite/inference frames,
PPML parent/current-generation overlap proof, opaque dependency/worker-pool
storage, and versioned whole-action protocol diagnostics. Whole-action coverage
and ratio remain undefined. The phase flag, Mac Xcode-license prerequisite,
freeze/package/install gates also remain open. No production adoption, commit,
push, package replacement, empirical rerun or campaign restart is claimed.

## Candidate preparation: OLS retained parents checked, cost gate open

The [retained-parent report](benchmarks/reports/development/candidate-ols-prepared-parents-2026-09-16/README.md)
records uncommitted numeric-array preflight, exact retained prepared/inference
capacity counts, internal admission reconciliation and earlier release of
obsolete pruning state. All six OLS VCE/topology branches preserve public base
fields. Saved-output admission counts simultaneous preparation/inference owners;
solver storage remains separately admitted. No action-coverage certificate is
claimed.

Snapshot `5240e58-ols-prepared-parent2`, based on main `5240e589`, matches all
163 tested native files. SCC job 7590173 passes 285 distinct native tests,
including independent allocator-deallocation checks, exact/one-byte-short
boundaries, allocation-failure, interruption and repeated-call controls.
OLS v21/action v4 inventories and the full repository checker pass. Thirty-four
of 35 source-contract Python tests initially pass; one ambiguous mutation anchor
is repaired and its focused recheck passes. All three action-inventory tests
pass. The original failed receipt and all historical source pins are preserved.

Job 7590187 passes exact scientific results in 27 alternating pairs (54 fresh
commands) over nine fixed synthetic cases. The comparison baseline is the
preceding unadopted publication prototype, not committed main. Slopes flag
0.238/0.262 s medians (+24 ms, +10.08%); the other eight cases clear the declared
threshold. Singleton-heavy PW/cluster/interaction controls retain exactly half
of 16,384/32,768 rows. The larger case's native peak falls 14.44%; this is a
local lifetime observation, not general memory qualification. The three-FE
base-only reporting discrepancy reproduces and still does not establish a cap
breach.

The one bounded attribution check, job 7590196, passes exact science in all
12 commands. Slope comparison medians remain 0.217/0.236 s, while identical
baseline controls vary 0.317/0.215 s. Attribution is indeterminate, and the cost
gate stays open. All four task jobs (including earlier snapshot 7590163) have
terminal success receipts and scheduler failed=0/exit=0. No further fit retry
is scheduled. Existing reconstruction jobs were not changed.

Next: complete construction/compiler containers, finite/inference transient
frames, slope recovery and dependency/pool byte proofs; resolve cost attribution
before adopting the combined prototype. Whole-action coverage and ratio remain
undefined. The installed Mac plugin is unchanged; no production commit, push,
package replacement, tag, release or empirical rerun was performed. The Mac/Linux
review candidate remains unready.

## Candidate preparation: OLS publication prototype, adoption pending

The [OLS publication report](benchmarks/reports/development/candidate-ols-publication-memory-2026-09-16/README.md)
records an uncommitted implementation for full reported-axis shadow matrices,
validators, requested observation outputs and nested saved-channel registries.
Admission happens before row reads; actual capacities are checked before any
store. Recovery adds its live caller buffers without charging returned channels
twice. Absorption and publication remain sequential. Public base fields are
unchanged. This prototype is not adopted yet.

SCC job 7590002 passes 189 distinct native tests, including exact/one-byte-short
fast/low admission and transactional capacity-failure checks. All 40 focused
Python checks, OLS v20/action v3 inventories and the full repository checker
pass. Both isolated Linux builds and ABI tests pass. Existing F0/historical
inventories and the installed plugin are unchanged.

Job 7590006 completes 15 alternating paired comparisons with exact scientific
matrices, sample/order, saved outputs and non-time solver fields. Four cases
clear the cost threshold; saved generated AW output fails it: 0.625/0.779 s
medians (+154 ms, +24.64%). Each arm reports identical 8,998,670 native bytes
and 5,898,240 scratch bytes. The failed build-wrapper receipt is preserved.
One native-phase diagnostic pair, job 7590011, gives 0.362/0.367 s and exact
science. It does not reproduce the slowdown, but cannot replace the original
three-pair gate. Its traced diagnostic also ran on a different host.

The [bounded control recheck](benchmarks/reports/development/candidate-ols-publication-recheck-2026-09-16/README.md),
job 7590027, completes three alternating baseline/candidate pairs and three
interleaved unchanged-artifact controls. All 12 commands pass with exact
science. Complete-command medians are 0.461/0.440 s (-21 ms, -4.56%); control
medians are 0.423/0.437 s (+14 ms, +3.31%). The earlier slowdown does not
reproduce, but one identical-artifact control pair differs by 83 ms / 17.89%.
Attribution remains indeterminate; the original failed receipt is preserved.
Native allocation/scratch match across all runs. Plugin-through-pre-store
medians rise 15.4 ms / 5.1%, while native medians rise 9.7 ms / 3.3%; complete
phase/RSS evidence is retained. Fresh repository and both inventory checks pass.
The source still matches all 162 tested native files. No estimator source or
installed binary was changed. This check adds no adoption or speed claim.

Next: continue the OLS preparation/inference/recovery parent proof,
compiler/topology metadata and dependency/pool capacities; preserve the timing
uncertainty for adoption review. No additional fit or campaign retry is
scheduled. Whole-action coverage and ratio remain undefined. The Mac/Linux
candidate is not ready; Mac prerequisite, freeze, packaging and clean-install
gates remain open. No installed package, push, tag, release or empirical rerun
was performed.

## Candidate preparation: PPML metadata and repository checks repaired

The [parent/metadata checkpoint](benchmarks/reports/development/candidate-memory-parents-2026-09-16/README.md)
adds PPML admission for nested recipes, availability-request copies, FE/cluster
labels and identities, boxed fit/result metadata, and the separate K-double
session row buffer. Prepared metadata capacities must fit the frozen preflight;
atomic replacement generations retain both parents. Publication optional-cell
validation no longer allocates index inventories.

Capacity tests distinguish frozen, staged-strict and committed-fit ownership.
The proposed extra permanent N-vector was rejected: canonical solve transfers
its residual, and strict commit releases rollback. The existing strict overlap
charge remains. Historical caps are unchanged; exact/one-byte-short boundaries
pass. No empirical cap breach or command-speed improvement is claimed.

SCC job 7589938 passes 450 distinct native checks against the isolated 161-file
snapshot, including failure/interruption/repeated-call/publication controls.
All 64 focused Python checks, both allocation audits and the complete repository
checker pass. The stale STATUS/archive and ADR-0041 wrapper checks are repaired.
Failed development receipts remain preserved. Inventories advance to OLS v19
and action v2 without changing F0 or historical pins. Installed plugin unchanged.

Remaining coverage: OLS preparation/inference/recovery/publication parents,
complete categorical/compiler metadata and opaque dependency/pool capacities.
Whole-action peak/ratio remain undefined; public base fields/wires are unchanged.
The newer checkpoint above tests OLS publication storage/admission. Candidate
freeze, licensed Mac/Linux validation, packaging/install checks and the Mac
Xcode prerequisite remain open. Nothing has been pushed or released.

## Candidate preparation: memory construction and allocation inventories

The [memory/inventory checkpoint](benchmarks/reports/development/candidate-memory-coverage-inventory-2026-09-15/README.md)
closes the PPML owned-FE construction omission on MAP and exploratory Mu routes.
The common preflight now includes count/cursor scratch, offset sentinels and
nested descriptors before allocation. Exact/one-byte-short fast/low budgets,
overflow and sequential construction checks pass. This is a component repair,
not evidence of an observed whole-action cap breach or a performance gain.

SCC job 7587586 passes 156 focused core, 96 plugin and 3 testkit checks (255 total)
on an isolated source/target with exact compiler-artifact identities. All 160
source/fixture files match the tested snapshot. The first fixture's stale
separation dimension failure is preserved. No empirical fit was restarted.

The reviewed OLS inventory v18 now watches memory helpers and binds four exact
post-F0 boundary replacements without modifying F0 or v17. A separate all-action
inventory covers 121 core/plugin source files, including PPML/separation/session
bodies. Both audits and all 37 Python mutation/structural checks pass. At that checkpoint the complete repository checker failed STATUS-heading,
historical-copy and public-PPML-wrapper assertions. Those failures are preserved;
the newer checkpoint above repairs them.

The [capacity/lifetime obligations](docs/allocation_inventory.md) distinguish
structural coverage from byte coverage. Parent/frame overlap, nested metadata,
opaque dependency capacities and saved/publication parents still block a complete
action model. Public base fields/wires are unchanged; whole-action peak/ratio
remain undefined. The installed plugin is unchanged. The candidate is not ready.

## Candidate preparation: empirical memory reproduction and remaining gates

The [new German OLS memory reproduction](benchmarks/reports/development/candidate-german-ols-memory-2026-09-15/README.md)
passes on the unchanged 5% input and the tested accounting artifact: 385.325 s,
4,361,733 retained rows, all saved columns and scientific tables exactly matching
the preserved native result. The tracked peak is again 3,570,575,392 bytes versus
2,841,380,024 public base bytes. Base plus separately admitted CG is
5,032,768,432 bytes, below the 34,359,738,368-byte usable budget. RSS is
4,421,021,696 bytes and scratch zero. Job 7586503 has successful scheduler,
process, FIT and Stata receipts plus its terminal sentinel. No reference or full
estimation was restarted. This is a memory reproduction, not a timing comparison
or complete-action memory certificate. All task jobs are now terminal.

The [static allocation-source audit](benchmarks/reports/development/candidate-source-contract-audit-2026-09-15/README.md)
fails against the unchanged frozen v17 inventory: 80 violations already exist
at requested baseline 9ecf47d, and 87 at native checkpoint a765c33. Seven new
reported categories concern intentional memory-carrier/admission changes; other
changes can share an already-failing file category. Both failures are preserved.
The source review and v18/all-action successor inventories are recorded above;
these original failures remain unchanged. Structural checks do not replace
capacity proofs or numerical tests.

The Mac/Linux review candidate is not ready. The remaining checklist is:

- [ ] Complete parent/frame, metadata and whole-action capacity/admission coverage;
  add versioned public peak/coverage diagnostics and protocol/package metadata.
  Undefined coverage must keep the action ratio undefined.
- [x] Review/version the OLS structural inventory without rewriting F0 and add
  the all-action inventory; both focused audits and mutation checks pass.
- [x] Johannes accepted ADR 0073 on 2026-09-15 for this fixed German review
  case. The failed reference residual-parity receipts remain unchanged.
- [ ] Resolve the Xcode license prerequisite, then freeze one exact SHA and run
  focused Mac/Linux checks, universal-slice audit and Rosetta loading.
- [ ] Produce repeatable deterministic Mac/Linux review archives with dependency
  audit, SBOM, licenses and publication eligibility false; validate installation
  using only the extracted package with explicit fixtures and terminal sentinels.
- [ ] Keep Windows artifacts, formal qualification and any publication as later
  explicit gates. Do not install, push, tag or release this checkpoint.

## Candidate preparation: memory accounting checkpoints

The [fixed Linux cost check](benchmarks/reports/development/candidate-memory-costs-linux-2026-09-15/README.md)
passes all 30 alternating pairs (60 fresh-process commands) and 11 predeclared
licensed fixtures. Scientific matrices, sample/order, saved outputs and non-time
solver fields match exactly. No median slowdown exceeds both 5% and 10 ms;
weak-wide medians are 6.003/5.982 seconds. Complete phase/allocation/RSS/scratch
receipts are retained; separately unobserved discarded-attempt time stays null.
The three-FE public base-only discrepancy reproduces (50,298,672 observed versus
22,983,168 base bytes), without establishing an execution-plus-solver cap breach.

The [larger singleton control](benchmarks/reports/development/candidate-singleton-scale-2026-09-15/README.md)
passes the exact predeclared 65,536/131,072-row sizes with half initially
singleton and a further quarter cascading. Both small independent oracles pass;
debug diagnostic times are 0.0570/0.1128 seconds (1.978x). No task job remains
running. The native implementation checkpoint is suitable for continued
development; it is not a frozen or validated production/review candidate.

The [isolated combined checkpoint](benchmarks/reports/development/candidate-memory-isolated-2026-09-15/README.md)
passes 880 core, 96 plugin and 3 testkit checks using a separate Cargo target
and verified compiler-reported executables. It includes the broader PPML
recovery/normalization/output envelope. Earlier shared-target receipts remain
diagnostic; this run binds its exact artifacts but does not close whole-action
coverage.

The [isolated PPML recovery-team repair](benchmarks/reports/development/candidate-ppml-recovery-team-2026-09-15/README.md)
keeps saved-effect CG/normalization on the accepted fit's worker team. A guard
rejects a count differing from memory admission before allocation, and no-save
calls retain their fast path. The unrouted control fails as expected; the
correctly hashed candidate passes all 55 estimator checks and a repeated
16,384-row generated/FW/clustered saved-output allocator test. All nine final
phase peaks fit their estimates across fast/1, low/4, fast/1 calls. The first
wrapper accidentally reused the control executable; its failure is preserved,
and one direct run of the verified candidate artifacts passes. Shared-target
checkpoint receipts remain development diagnostics; subsequent combined checks
use a distinct target and compiler-reported executable identities as required.

The [isolated PPML inference repair](benchmarks/reports/development/candidate-ppml-inference-memory-2026-09-15/README.md)
adds the retained full-axis covariance/statistics and cluster/Wald identities to
inference, saved-effect recovery and publication admission. All 42 memory tests
and 54 prepared-estimator checks pass on a clean snapshot without the larger
memory prototype. Exact and one-byte-short admission, actual rebased-output
capacities, saved slopes, allocation failure and interruption are covered. This
repairs a phase-inventory omission; it does not establish an empirical cap breach.

The current implementation extends the [earlier memory checkpoint](benchmarks/reports/development/candidate-memory-accounting-2026-09-15/README.md).
The current source-bound cost controls pass; complete-action coverage remains
open. The earlier snapshot and its original status remain historical evidence.
Its tested snapshot passes 878 core, 95 plugin and 3 top-level library checks.
The later no-budget CMG reporting follow-up passes 35 CMG and 35 PPML IRLS
checks against a separate source identity. Failed setup, fallback, inference
reconstruction and intercept recovery envelopes now propagate through a compact
8-byte carrier; unknown scopes remain unknown. Earlier failed fixtures and the
baseline PPML warm-allocation failure are preserved in the receipts.

The compact internal solver carrier, failed-attempt/fallback propagation,
specialized slope-MAP admission and PPML saved-effect solver envelope are
implemented. Whole-action parent/frame and metadata coverage and versioned public
action diagnostics remain open; the new German OLS reproduction is recorded above.
The public base-memory fields and result wires are unchanged. No whole-action
ratio or complete-action coverage is claimed. Mac builds still fail on the
unaccepted Xcode license. ADR 0073 has the owner's case-specific approval. Exact-SHA
candidate freeze, universal/Rosetta checks, deterministic Mac/Linux archives and
extracted-package installation remain blocked or pending. Nothing was installed,
pushed, tagged, released, or promoted to formal qualification.

## Candidate preparation: PPML warm-allocation repair

The [focused Linux repair](benchmarks/reports/development/candidate-ppml-thread-freeze-2026-09-15/README.md)
resolves automatic execution threads once per admitted IRLS fit while retaining
the original requested count in public diagnostics. A same-host clean-baseline
comparison records 80 warm allocations before the fix and zero afterward.
All 22 PPML plugin checks and 35 focused core IRLS checks pass, including
allocation/interrupt rollback, reconstruction and final-Hessian synchronization.
The initial clean snapshot's missing test CSV and failed compile remain in the
receipt; its one focused fixture repair passes. This is an allocation repair,
with no complete-command speed or candidate qualification claim.

## Candidate preparation: saved native PPML audit closed

The [saved-output report](benchmarks/reports/development/candidate-german-reaudit-2026-09-15/README.md)
binds independent audit source `e6ae50e` to the preserved 5% and full native
outputs. Both native public-mean audits pass coefficient binding, fresh model
space, objective replay and scores, including all 88,876,264 full-sample retained
rows. The 5% native objective passes ADR 0026's existing rules. The reference
public-mean witness remains indeterminate and fails replay/stationarity checks.
No estimation or stopped full reference was restarted; original failures remain.

The [completed OLS adjudication report](benchmarks/reports/development/candidate-ols-adjudication-2026-09-15/README.md)
records the one permitted tighter reference fit and independent all-row
conditional projection. Native residuals pass on all 4,361,733 retained rows
(maximum error 1.177e-10); the tighter reference still misses the existing
pointwise envelope. Sample/order, coefficients, covariance and DoF parity pass.
No native numerical defect was demonstrated. [Accepted ADR 0073](docs/adr/0073-german-ols-review-adjudication.md)
authorizes this fixed case's independent residual adjudication for the Mac/Linux
development review. Exact-candidate preservation still requires validation.
No general CG exception or tolerance relaxation is active;
original failed comparisons remain failed. No further reference estimation is
planned.

The earlier OLS 25.7% difference compares whole-action allocation with base
execution storage. Its separately admitted CG workspace is 2,191,388,408 bytes;
base plus CG is 5,032,768,432 bytes, above observed 3,570,575,392 bytes. That
comparison does not demonstrate a cap breach. Complete action accounting and
coverage remain required; a whole-action memory claim is not accepted. Mac builds
still await owner acceptance of the Xcode license. No candidate is qualified.

## Candidate preparation: packaging and fixed development controls

The [review packaging tests](benchmarks/reports/development/candidate-review-packaging-2026-09-15/README.md)
pass all 17 synthetic checks. Production staging still requires all three
binaries. The explicit Mac/Linux review mode marks Windows missing and
publication ineligible; actual candidate archives and clean installation checks
remain pending. The [fixed development case list](benchmarks/manifests/candidate_review_development_v1.json)
was established before new timing measurements. Three alternating fresh-process
pairs per affected case and the stated regression gate remain required.

## Candidate preparation: independent two-FE audit implemented

The [focused report](benchmarks/reports/development/candidate-audit-local-2026-09-15/README.md)
records the user-authorized spanning-forest witness and grounded, degree-scaled
LSMR fallback. All 35 audit/calibration tests pass at unchanged scientific
thresholds. Singleton-heavy 40k–320k audit scaling is approximately 2x per
doubling. This checker changes independent validation only. The completed
German native PPML audit and pending OLS review decision are recorded above.
Mac builds await owner acceptance of the updated Xcode license.

## Full German PPML: native completed, reference stopped by user

The [full fast report](benchmarks/reports/development/german-ppml-full-fast-local-2026-09-15/README.md)
records source `ece0db4` / isolated plugin `156f3835`. The unchanged full
exp(logrwage2) command completes in **9,454.604 s (2h 37m 35s)** with
**88,876,264 retained rows**, 2,169,026 singleton drops and 14 iterations.
Process/FIT_RC/STATA_RC, output header/hash and all public certificates pass;
zero strict-profile rejections, retries and fallbacks. Native tracked peak is
65.31 GB against the 72.69 GB selected model; process RSS peaks at 83.68 GB,
with zero swaps or scratch. Auto reports MAP-family execution with fast storage.
FE projection consumes 8,235.32 s; full phase/solver costs are retained.
This is successful native publication; the saved full output now passes the
independent audit recorded above. The old interrupted low run is not a paired
speed baseline. Disclose
the overlapping SCC transfer in the native timing.

The user stopped the full ppmlhdfe reference because it was taking too long.
The [stop receipt](benchmarks/reports/development/german-ppml-full-fast-local-2026-09-15/reference-user-stop.md)
records verified process termination, process_rc=-15/accepted=false and no final
Stata receipt or numerical exports. Elapsed process time is 17,269.615 s
(4h 47m 50s), last logged outer iteration four; this is censored duration, not a
completed benchmark or observed convergence failure. Peak RSS is 106.37 GB,
peak footprint 115.05 GB, zero reported swaps. Partial logs are preserved.
The monitor was deleted and no automatic restart is scheduled. All work in this
comparison is stopped or terminal. Full reference comparison remains unavailable;
independent native model-space acceptance now passes. The 5% historical gaps
below retain their original source-specific status.

## German 5% SCC comparison: commands complete, scientific gaps unresolved

The user authorized a bounded SCC comparison and project storage. The
[source-bound report](benchmarks/reports/development/german-five-percent-scc-2026-09-15/README.md)
records source `0660945` / isolated Linux plugin `83d27773`. The deterministic
5% worker draw yields 4,553,489 rows from 774,281 workers, preserving selected
2001–2009 histories, ordering, extremes and default singleton removal. Checksum,
roundtrip and sample job 7580024 pass. OLS uses logrwage2/conventional VCE;
PPML uses exp(logrwage2)/robust VCE. Four application threads match SCC's license;
each family reserves 16 slots/48 GiB with native fast/auto and a 40 GiB cap.
Build, launcher and initial 9,000-row scientific smokes pass.

The [continuation receipt](benchmarks/reports/development/german-five-percent-scc-2026-09-15/continuation/README.md)
preserves both original failures. OLS 7580025 fails before estimation in shared
ftools compilation. PPML 7580026 reaches its 3-hour allocation (10,801 s,
failed=100/exit=137): first native and both references finish; second native is
interrupted without a fit result. Analysis 7580041 fails on missing final
manifests before scientific checks. No completed two-pair PPML campaign is claimed.

One narrow OLS launcher fix gives each process private pinned sources and a
private compilation directory, verified before both command timers. The focused
retest 7580500 passes two concurrent bootstrap checks, both 9,000-row OLS calls,
independent score/model-space checks and complete failed=0/exit=0 accounting
(4 s, 1.190G maxvmem); shared libraries are unchanged. Empirical OLS job 7580504
finishes all four commands, then fails the comparison (failed=0/exit=1,
3,189 s, 4.764G maxvmem). The [terminal OLS report](benchmarks/reports/development/german-five-percent-scc-2026-09-15/ols-terminal/README.md)
records command times 293.338/301.054 s native and 1,315.637/1,259.514 s
reference. Both pairs have exact input/sample/order/DoF and passing b/V;
maximum absolute b/V differences are 9.9425e-13/5.9166e-19. Saved residuals
fail the pointwise envelope: max difference 6.3657e-5, 699,592 rows outside.
Independent normal-equation diagnostics pass both arms, with smaller native
scores; this alone does not settle prediction accuracy or model space.
Diagnostic 7580896 passes failed=0/exit=0 in 12 s, 853.215M maxvmem, without
re-estimation or changing thresholds. Original failure remains preserved.
Native auto reports CG in design/residual stages (221/254 iterations), both
flagging fallback use; full stage costs are retained. The large-auto-LSMR
exception is not assumed applicable. Comparison acceptance remains blocked.

Both native OLS runs expose a separate memory-model underestimate: 3.5706 GB
tracked allocation versus 2.8414 GB selected model (ratio 1.256634). Native
process peak RSS is about 4.416 GB, reference about 3.674 GB; zero swaps/scratch.
The 40 GiB cap has headroom but does not establish tight-cap admission safety.
Preserve this OLS model gap for focused repair; it is separate from the adopted
PPML bounds. Native source/artifacts and reference policies are unchanged.

Complete PPML pair audit 7580499 fails its native model-space gate after 607 s
(failed=0/exit=1, 1.841G maxvmem). Exact rows/sample/order, normalized rank/DoF,
b and V pass; maximum absolute b/V differences are 2.7700e-14/2.7228e-15.
Native public means replay the objective and pass scores (maximum 2.1903e-14),
but independent LSMR reaches 5,000 iterations/stop 7: fresh relative residual
3.3016e-6 and normal 1.3284e-8 exceed unchanged 1e-10/1e-12 thresholds.
This leaves model space indeterminate and the full comparison unaccepted;
it does not prove prediction infeasibility. Reference public-mean diagnostics
and objective adjudication are not reached. Preserve this failure; localize
the projection limitation before another expensive model-space audit, including
on the full local sample. No estimator/audit retry or tolerance relaxation.

The single complete pair takes 1,369.344 s native versus 4,904.909 s reference;
these remain diagnostic timings, not an accepted speed claim. All SCC jobs
are now terminal; the full local reference was subsequently stopped by the user.
There are no active jobs or monitors for these comparisons. The stopped SCC
campaign remains stopped; no push, installed-dist change or qualification claim.

## German PPML: memory bounds and consistency-loop repair adopted

The [checkpoint report](benchmarks/reports/development/german-ppml-checkpoint-recovery-local-2026-09-15/README.md)
records the scheduler regression: ordinary iterations loosened a checkpoint's
1e-14 requirement to 1e-12. The repair persists refinement, permits one canonical
storage-preserving recovery, and returns bounded r(430) on persistent stagnation.
All scientific acceptance tolerances are unchanged. The analytical two-FE case
improves seven→five iterations and predictor error 9.06e-9→1.95e-12.
360 PPML/separation, 215 OLS/session, 97 plugin tests, universal/ABI checks,
26 licensed fits and 18 paired commands pass. Paired scientific exports are
exact; independent scores/model-space and pinned subset comparison pass.
Low/fast/narrow medians are 2.286→2.224, 3.070→3.069 and 0.090→0.089 seconds.
Phase/RSS costs and the slower fast strict phase are disclosed. These controls
show no material regression; they are not a full-data speed claim.

The [memory report](benchmarks/reports/development/german-ppml-preflight-bounds-local-2026-09-15/README.md)
records bounded exact integer FE counts before admission, without N-sized state
or reduced headroom. Full-data rejection probes reduce the fast base estimate
184.58→72.69 GB and low/pool4 88.41→55.93 GB. Constrained-auto subset medians
improve 2.240→1.381 s; explicit-low preparation overhead is disclosed.
Full native fast admission/RSS, 88,876,264 retained rows and public PPML
certificates now pass; the full report above records the completed run.
Independent full-data validation and the pinned-reference comparison remain pending. The combined isolated
plugin is `156f3835`; installed dist remains `4187efdf`.

The old low/pool4 run was stopped after 33,956.40 process seconds, 66.28 GB max
RSS and repeated strict rejection through iteration 36. No result was accepted;
logs/scratch are preserved and its obsolete monitor was deleted. The independent
full-output comparison and mean audit are now calibrated on the 65,503-row
subset. No push or cluster work; original unrelated STATUS and untracked work
remain intact.

The full fast retry and retained-count receipts remain preserved; completion
and the sequential reference launch are recorded in the full report above.
Preserve failed launch/previous-run receipts and do not overlap the timed
reference with builds or numerical audits.

## German PPML: changing-weight scratch block transport adopted

The [source-bound report](benchmarks/reports/development/german-ppml-block-transport-local-2026-09-14/README.md)
adopts complete-row extract/patch transport through the existing bounded scratch
bank. Parent native is `0531450` / `2acecd29`; tested candidate is `fdaf3e16`.
No production capacity, weight, sample, rank, projector or certificate change.
215 OLS/session, 351 PPML, 97 plugin tests, universal static/ARM64 ABI and 26
licensed fits pass, including weak weighted graphs and memory/pool controls.

Twenty-four paired commands preserve exact scientific exports. Three-pair German
command medians fall 0.817→0.323 s (6,654 retained) and 8.109→2.245 s (65,503),
located in FE projection and strict/final work. Narrow and 129-column/pool-one
controls improve; independent scores/model-space pass, as do pinned-reference
German samples/b/V. Native peaks and scratch bytes are exact; full phase/RSS
costs and tiny variable synthetic plans are recorded. This is local evidence.
Full-row I/O increases logical sequential bytes while greatly reducing calls;
the existing 1 MiB bank is reused. The wide oracle's original width-limit error
and corrected all-column audit remain visible.

The subsequent full exp(logrwage2) retry used low memory, pool four and eight
threads and was later stopped; see the current repair section above. The isolated tested
plugin matches committed `fad7a5c`. Its validated IRLS-entry trace confirms
91,045,290 original and **88,876,264 active rows**; see full-row-check.json. The
first two IRLS iterations have completed; the second accepts a full step. The
first checkpoint matches the parent exactly. Convergence has not passed and no
full fit is published yet; see full-second-iteration.json. One one-second sample
shows changing-weight projection
in the existing parallel Schur/PCG workers. The harness preserves sample/order
and means/absorbed outputs after the timer. The pending reference runner now
verifies all 18 frozen capture files, including archive-backed ppmlhdfe rather
than assuming every source has Git metadata. See full-progress-checkpoint.json.
Run the prepared pinned reference sequentially after native completion, then
independently compare outputs. A bounded all-row public comparison script is
prepared and syntax-checked; calibrate it on existing German subset exports
after timed estimation ends before using it on the full outputs. Its preparation
receipt records the local source hash and the separate pending full-mean audit.
The preceding full run verified 88,876,264 active rows but was interrupted after
one completed IRLS iteration and 1,453.38 process seconds; no full PPML estimate
or full-data speed claim exists. Its scratch, logs and three samples are preserved.
Installed dist remains `4187efdf`; no push or cluster work. Original unrelated
STATUS additions and untracked evidence remain intact.

Roadmap: [adopted transport and remaining work](https://github.com/johannes-schmieder/fereg/issues/27#issuecomment-5672315940).

## German PPML: scratch moments adopted; full retained count verified

The [source-bound report](benchmarks/reports/development/german-ppml-scratch-moments-local-2026-09-14/README.md)
adopts bounded complete-row reads in retained scratch moments. The same Gram
partitions preserve row/FMA order, weight/reference scales and selected identities.
Low preflight covers tile capacity and nonmonotone partition counts after rank
shrinkage. Dense/one-shot OLS routes do not acquire the bank. Parent is `1f76986`
with plugin `e1c72f56`; isolated candidate is `2acecd29`. Installed dist remains
`4187efdf`. See ADR 0070 for the bounded workspace extension to ADR 0020.

214 OLS/session tests, 351 PPML tests, 97 plugin tests, two memory tests,
universal static/ARM64 ABI checks and 26 licensed native fits pass. The new
scalar oracle proves exact moments and over 50-fold fewer read chunks at 18
stored columns on 65,536/131,072 rows. Constrained-memory expectations now charge
the tiles under the same imposed limits. Warmed allocation checks pass.

Three alternating German pairs per subset retain 6,654 and 65,503 rows. Command
medians fall 1.194→0.845 s and 11.688→8.224 s (about 29%); finite solve medians
fall 0.327158→0.039424 s and 3.171349→0.390776 s. All paired scientific values
are exact, and independent score/model-space checks pass. Native allocation
peaks rise 47,520 bytes each; larger RSS median rises about 6.14 MB. Scratch and
non-time solver work are unchanged; solver fallbacks remain zero. This is local
development evidence, not a qualification or full-data timing claim.

The preceding [positive-outcome report-capacity adoption](benchmarks/reports/development/german-ppml-report-capacity-local-2026-09-14/README.md)
reduces the full low/pool1 plan from 156.87 to 68.74 GB without weakening
headroom. Zero-containing samples retain their general bound. The old full
pool4 attempt passed admission/separation and reached retained moments, but was
interrupted after 723.36 process seconds, 65.07 GB maximum RSS and no swaps.
Its scratch is preserved; no estimate or authoritative retained count was
published. The new validated IRLS-entry trace exposes that count before solving.

The full retry now passes preparation/separation and its validated IRLS-entry
trace confirms 91,045,290 original and **88,876,264 active rows**. It uses
exp(logrwage2), all observed extremes, low memory, pool four and eight threads.
See the separate full-row-check receipt in the report. The first IRLS iteration
completed, but the next changing-weight projection still used per-row block
reads/writes. A third one-second sample captured 742 worker samples there.
The owned run was interrupted after 1,453.38 process seconds, with 65.16 GB
maximum RSS and zero swaps; no full estimate was published. Its scratch and
three samples remain local. The bounded block transport adoption above addresses
this bottleneck; its full-data retry remains pending. Residual and QR row loops
remain possible later costs.
No installed-binary replacement, cluster work or push. Original separate STATUS
changes and untracked evidence are preserved.

Roadmap: [scratch-moment progress](https://github.com/johannes-schmieder/fereg/issues/27#issuecomment-5671859571).
Detailed external benchmark payloads previously failed automatic approval review;
only minimal generic roadmap updates have been approved.

## German PPML: smaller samples pass; low-memory categorical projection adopted

The [source-bound report](benchmarks/reports/development/german-ppml-low-map-local-2026-09-14/README.md)
adopts bounded categorical MAP projection in low memory, reusing existing guarded
current-weight machinery. Dense response fusion stays fast-only; low retains
scratch blocks. Projector capacity is capped by design width, and low preflight
charges group/dot and accelerator banks before allocation. Parent native is
`1e9581e` / `bc1e8909`; candidate universal plugin is `9d920101`. All 177 compiled
inputs match the source transfer. Installed dist remains `4187efdf`.

Using the selected `exp(logrwage2)` outcome, four German subsets retain 6,654,
65,503, 132,691 and 338,054 rows with exact pinned-ppmlhdfe masks/order. Slopes
agree within 1.1e-13 and covariance within 1.3e-12. Native means pass independent
objective/score/model-space audits; independent FWL covariance agrees within
5.8e-17. Reference reconstructed means do not certify model space; its reported
objectives remain the authority under ADR 0026. Rank/absorbed-DoF normalization
bookkeeping differs by one between commands, with equal total rank/residual DoF.

The 75,350-row low fit completes in 9.824 s; parent low was interrupted at
257.68 process seconds during generic weighted projection, without a fit.
Small low falls 1.439→0.931 s. At 378,124 rows, low costs 86.049 s, fast 17.914 s
and ppmlhdfe 82.414 s; low native peak is 599 MB versus fast 1,183 MB, with
48.68 MB scratch. These are single local diagnostic timings, with tracing and
a disclosed brief contention risk for the first 75,350-row comparison. They
are not universal speed claims. All completed native solver fallback counts
are zero. Phase costs, inner/outer counts and small-memory overhead are archived.

347 PPML-related core tests, 213 OLS tests, 97 plugin tests, universal static
audit and ARM64 ABI/self-test pass. Two licensed fixtures contain 26 native fits
with one/two/three FE, tiny weighted mobility links, pools 1/2/10000, FW/PW,
clustering/missingness and singleton/separation cascades. A 557 MB cap rejects
fast before workspace allocation while low succeeds. The first dummy-Poisson
oracle's loose stopping and the independent audit's insufficient scratch budget
are preserved with their resolved reruns and unchanged candidate tolerances.

Roadmap evidence: [#27 adopted low-memory follow-up](https://github.com/johannes-schmieder/fereg/issues/27#issuecomment-5671023540).

Next: inspect full German PPML memory admission and a fitting bounded pool,
then estimate and compare with pinned ppmlhdfe. The 8.37-billion wage in the
largest subset was removed by ordinary singleton pruning in both commands;
full-sample extremes remain untested. No extra sample exclusions, SCC work,
installed-plugin replacement or push occurred. Unrelated work is preserved.

## German OLS: full fit certified after QR and macOS memory repairs

The [source-bound report](benchmarks/reports/development/german-ols-qr-memory-local-2026-09-14/README.md)
adopts long-vector compensated QR reductions and repairs macOS availability
through Mach free/inactive statistics, against `0af4321`. Rank/reference-scale,
weight, covariance, certificate and memory-headroom policies remain unchanged.
The isolated universal plugin is `bc1e8909`; installed dist remains `4187efdf`.
Patch replay matches 177 compiled inputs and the added licensed fixture. There
are 71 passing OLS fit tests, 302 PPML core tests, 97 final plugin tests, two
licensed fixtures (8 OLS and 14 PPML native fits), universal static audit and
actual ARM64 ABI/self-test. No Windows/x86 scientific runtime claim is made.

The original full German OLS specification now publishes certified estimates:
91,045,290 input rows, 2,169,026 singleton drops and **88,876,264 retained**.
All 18 slopes agree with the completed owner reghdfe result within 2.569e-11;
covariance maximum difference is 5.679e-20 and relative RSS difference 7.286e-13.
Rank is 18, absorbed DoF 14,702,025 and residual DoF 74,174,221 in both results.
Finite inverse errors are about 1e-16 under the unchanged 5e-10 tolerance.

Command time is **1,708.530 s (28.48 min)** versus the saved reghdfe run's
6,699.551 s (111.66 min), a single unpaired local ratio of 3.92. Preparation
including singleton pruning is 28.717 s. Maximum process RSS is 78.48 GB,
native allocation peak 44.55 GB and scratch 13.51 GB, with zero swaps. Initial
absorption uses three CG fallbacks (1,136.138 s solve); final residual absorption
uses one (132.160 s). A one-second stack sample is disclosed. No isolated QR
performance gain or full-reference mask comparison is claimed.

The old macOS sysctl omitted inactive pages; it can also fail under a restricted
invocation. The final plan keeps its original headroom. CMG's existing conservative
bound is about 731 GB before the 36.42 GB execution reserve, so certified CG is
selected. Tightening that broad bound without weakening admission is a next P0
candidate; no CMG sizing or hierarchy changes are adopted. The native peak is
22.33% above the published base execution estimate, within the joint solver plan.

The QR-only full attempt's r(909), earlier r(459), rejected short-vector/Givens
arithmetic, first Mach layout-length failure and zero-test launches remain
preserved. The successful full run uses CG whereas the earlier r(459) used CMG;
independent power-of-two oracles isolate the QR bug, but the full runs do not
isolate its sole causality. German subsets and exact small masks remain controls.

Next: test German PPML on smaller subsets using the user-selected imputed
wage outcome `exp(logrwage2)`, retaining its observed extremes, before any full run. Universal
singleton pruning is already adopted for OLS/PPML and all memory modes; no German
PPML result is claimed. Broader #27 work and the separate action-memory/PPML repair
remain open. No cluster work, installed-binary replacement or push occurred.

Roadmap disposition: [#27 evidence and open work](https://github.com/johannes-schmieder/fereg/issues/27#issuecomment-5670448455).

## Universal singleton pruning: prepared PPML queue adopted locally

The [incremental source-bound report](benchmarks/reports/development/ppml-singleton-index-local-2026-09-14/README.md)
extends parent `73aef645edc2` to production prepared PPML, which previously used
its own repeated membership-pass loop. Every production OLS/PPML singleton
entry path now uses an amortized linear fixed-point algorithm, independently of
fast/low/auto memory, solver, covariance or supported weight policy. PPML reuses
its CSR and partition codes, maintains exact `u128` frequency counts, queues each
deletion once and emits sorted proposals. Separation-triggered cascades use the
same boundary. Concrete per-level/row/view costs are admitted before preparation;
no-drop samples skip queue and selection-mask allocation. Ledger/reason ordering,
interruption and transactional publication are unchanged.

All 302 PPML core tests and three focused licensed fixtures pass. The new fixture
includes 14 native fits, exact drop reasons and samples, rank/status, DoF, final
weight normalization and covariance, with independent dummy Poisson coefficients
and fitted means. 640 unit/frequency multiway cases match a pass oracle; cascade
scale doubling bounds membership visits by 2*D*N. All 177 compiled inputs match
the adopted main source; isolated universal plugin is `312c7546`. Installed dist
remains `4187efdf` and was not rebuilt.

In 36 local paired commands, the 36,864-row PPML cascade falls 2.123→0.298 s;
its singleton/FE-separation phase falls 2.004218→0.001000 s. Recorded slope,
variance, counts, rank, DoF, deviance and iteration/fallback fields match exactly.
Fallback counts are zero. The largest cascade's native allocation peak increases
by 46,176 bytes; other case peaks and all scratch/selected model values match.
Balanced fast/low command medians are 1.1%/1.8% slower despite slightly faster
separation; three pairs do not establish this small variation as systematic.
A one-pass singleton kernel retains 0.03 ms overhead. Initial balanced-counter
regression was rejected before adoption; all raw receipts remain local.

Fresh pinned-reghdfe OLS screens on 75,350, 151,029 and 378,124 input rows pass
in both memory modes, retaining 65,503, 132,691 and 338,054 rows with exact masks
and DoF. Maximum coefficient difference is 1.11e-11. The largest subset costs
57.512/56.901 s (low/fast) versus reghdfe's 31.613 s: a useful weak-graph control,
not a speed win. These use the parent plugin and are single diagnostic timings.

Full German OLS estimation, unresolved at this checkpoint, is now certified
by the QR/macOS follow-up above. The user selected `exp(logrwage2)` for German
PPML; no empirical PPML success is claimed. #27 priorities and the unrelated
action-memory/PPML repair work remain open. No cluster work, installed-binary
replacement or push occurred.

## AKM singleton pruning: indexed linear-time fix, full retained count verified

The [source-bound report](benchmarks/reports/development/singleton-index-local-2026-09-14/README.md)
fixes the diagnosed observation-zero scan in `fe/singleton.rs` against `681634d`.
Each level keeps an active-row XOR; queue entries become global level indices,
paying for the index within the previous allocation envelope. Work is O(D*N+L).
The original no-singleton exit, queue/deletion order, masks, weight multiplicity,
fallible allocations, worker/SPI and interruption/publication boundaries remain.
The adopted isolated universal plugin is `253bf9eb`; installed dist is unchanged.

All 33 focused shared core tests, 95 serial plugin tests and four licensed fixtures
pass. New checks compare 640 cases with the old scanner and an independent mask
oracle, cover one-to-five effects and frequencies, long cascades, all seven
reservation/capacity failures, fast/low OLS and cluster exclusions. Patch replay
matches all 176 archived compiled inputs. In 108 bounded pruning calls, doubling
singleton-heavy inputs gives approximately 2x time versus the scanner's 4x;
65,540-row cascading pruning falls 1.106749→0.001242 s with exactly equal live
allocation peaks. No-singleton performance/allocations remain unchanged.

At this checkpoint, the original full AKM command cleared native preparation
in 27.574 s, then failed closed with r(459), `prepared OLS bread inverse certificate did not pass`,
in the unchanged low-memory numerical path. No full estimates are produced.
A separately archived diagnostic plugin (`e9b2141e`) reports the unchanged
prepared sample and deliberately stops before solving: 91,045,290 original and
candidate rows, 2,169,026 singleton drops, **88,876,264 retained**, exactly matching
reghdfe. Preparation is 29.582 s; process maximum RSS is 48.16 GB. Its expected
r(1) and no-public-coefficients assertion pass. This diagnostic hook is not adopted.
The normal candidate smoke retains 65,503 rows; its 18 coefficient and V maxima
against saved reghdfe results are 1.77913e-14 and 1.37152e-17, with exact N/DoF.

The later QR/macOS follow-up above now certifies full OLS estimation under
unchanged tolerances. This earlier inverse-failure receipt remains unchanged.
All failed runs/builds,
original data/scripts and historical receipts remain. This fix leaves #27's seven
P0–P7 items and the isolated action-memory prototype open; it does not add solver
changes, agents, cluster work or a push. Unrelated STATUS additions and all 358
original untracked paths are preserved.

## P0 CMG/Schur memory lifetimes: tested prototype continuation, not adopted

The [source-bound report](benchmarks/reports/development/action-memory-cmg-schur-local-2026-09-14/README.md)
archives full patch `2e03030b` and five-file incremental patch `d02e385b` against
carrier `082e1625`, evidence `2739915` and accepted native `25bf610`. Borrowed CG
reports actual fixed-bank/matrix/coefficient capacities. CMG retains admitted
construction through setup failure and hierarchy rejection, observes lazy pool
growth, charges hierarchy/coefficient overlap during ordinary polishing, and
separates released-hierarchy final correction. Three-vector norm construction
and its retained output are included. OLS Schur now carries allocated probe,
full-attempt and CG-continuation/reference-norm peaks through the outer adapter.

All 86 distinct focused core tests and 95 serial plugin tests pass on the final
frozen source without compiler warnings; replay matches all 179 candidate inputs.
Six new tests cover oversized reservations/early exits, real failed nuisance
construction, rejected admission, one-to-four pool growth, full-width coefficient
reconstruction, retained versus released correction, and one-time outer merging.
Existing weighted independent-MAP, forced Schur continuation, weak-graph and
exact/one-byte-short admission checks pass. Initial edit assertion, incorrect
pool accessor and test-only adapter warnings remain documented. No scientific
threshold or fixture was loosened.

The implementation stays isolated; public reporting, all 178 accepted main inputs
and installed dist remain unchanged. No release build, licensed-Stata oracle,
new public cap or command timing ran, so no performance gain or whole-action
bound is claimed. Next cover warm retained LSMR, saved MAP conversion/failed CMG
coefficient attempts, specialized slopes, recovery/inference and final action
transport, then the original three-FE and weak-wide checks. Prepared PPML parent
accounting also remains open. Full pass/copy/build/physical-attempt reporting and
seven P0–P7 items remain; R1–R5 are excluded. Reviews, upstream assessment and SCC
follow P0–P7. All 97 unrelated STATUS additions and 358 original untracked paths
remain; no agent, cluster, upstream change or push occurred.

## P0 compact action-memory carrier: implemented prototype, not adopted

The [source-bound checkpoint](benchmarks/reports/development/action-memory-carrier-local-2026-09-14/README.md)
archives patch `082e1625` against evidence `d0c0e86`/accepted native `25bf610`.
It adds an eight-byte workspace-peak carrier with a distinct unmodeled state,
reuses generic/owned/direct-CG admission quantities, counts fixed borrowed-MAP
capacities, and preserves admitted versus rejected/fallback attempt envelopes.
Low-memory report merges propagate unknown coverage. Warm retained LSMR overlap,
CMG/Schur construction, specialized slopes, recovery/inference and whole-action
transport remain explicitly unmodeled; public selected-memory reporting is unchanged.

All 41 distinct focused core tests and 95 serial plugin unit tests pass on the
final frozen source; an isolated patch replay matches all 179 candidate inputs.
Tests cover closed-form residuals, exact/one-byte-short bounds, weak-chain routing,
capacity over-reservation, discarded LSMR fallback and cold/warm reuse. Review found
and guarded retained-operator overlap and deferred compact conversion for an unused
CG route. Formatter/archive-parser findings remain recorded. No release-plugin
build, licensed oracle or performance measurement ran; avoiding the earlier
weak-wide overhead remains a hypothesis. Main retains all 178 accepted native
inputs and installed dist. The prototype stays isolated, with its replayable
patch/evidence archived here rather than adopted as complete reporting.

Next finish CMG/Schur construction and retained solver/pool accounting, saved
coefficient/recovery and inference propagation, then aggregate and publish the
complete model. Verify the original three-FE joint cap and the earlier weak-wide
cost controls. P0 pass/copy/build/physical-attempt work and the seven active P0–P7
items remain open; R1–R5 are excluded. Reviews, upstream assessment and SCC follow
P0–P7. No agents, cluster work, upstream change or push occurred.

## P6 generated OLS residual tiles: rejected; P0 three-FE memory gap

The [source-bound report](benchmarks/reports/development/generated-ols-residual-tiles-local-2026-09-14/README.md)
rejects candidate `e97dee05` against accepted `25bf610`/plugin `e6978061`.
Shared generated recipe tiles preserve each prepared layout's absent-indicator
FMA semantics. All 16 core/95 plugin tests, eight independent licensed fits per
artifact and universal/static/arm64 ABI checks pass. All 25 untraced pairs retain
exact science/solver fields. Repeated AW/unit/PW command medians are
1.095→1.039 / 1.252→1.195 / 1.553→1.484 seconds; median paired gains are
5.18%/5.25%/4.44%. PW misses the predeclared 5% repeated-primary gate. Separate
original-input diagnostics locate 77.978 ms in fast raw-residual accumulation;
low construction stays about 4.2 ms. Largest control cost is low PW +48 ms/2.21%.

All paired native peaks are exact, but the original actual-three-FE FW OLS
control peaks at 50,265,168 bytes against a reported 22,983,168-byte selected model
on both artifacts. This is a baseline P0 memory-accounting gap, not an admitted
control. Other cases fit their models; PW sampled RSS rises. Both violations,
all 56 captures and the summary's failed admission assertion remain. No extra
repeats/public cap calls follow rejection. All 178 accepted source inputs are
restored; the prototype source/patch/artifacts remain isolated and archived.

Source [adjudication](benchmarks/reports/development/generated-ols-residual-tiles-local-2026-09-14/memory-gap-adjudication.md)
clarifies that the public value is a base execution model; adapters separately
admit incremental solver workspace against the usable cap. This confirms the
known P0 reporting gap, not an established breach of actual joint admission.
Next complete action/attempt memory reporting and verify the joint boundary on
the original three-FE inputs, including the older reporting prototype's known
weak-wide timing and coverage limitations. Do not inflate a multiplier to cover
the observed number.

Seven P0–P7 items remain. Next diagnose and repair that three-FE memory gap on its
original full/warm inputs before broader timing work. Then finish generated/shared
layouts, remaining PPML state/construction, P7 scheduling and P0 full accounting.
Outer Stata work is also material in the generated targets (about 0.44–0.53 s)
but is not yet attributed to a particular ado consumer. R1–R5 remain excluded;
independent reviews, upstream CMG assessment and the bounded SCC campaign follow
P0–P7. Installed dist, unrelated STATUS additions and original untracked files
remain. No agents, upstream changes, cluster work or push occurred.

## P4/P6 generated finite-predictor tiles: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/generated-predictor-tiles-local-2026-09-14/README.md)
and [ADR 0068](docs/adr/0068-retained-generated-predictor-tiles.md) adopt candidate
`e6978061` against evidence `ef38dd5`/accepted native `fb4cc5b`. Fresh/strict
retained generated predictors borrow final ordinary/topology factor codes into
128-row tiles under the existing team. Original coefficient-slot FMA order,
including zero indicators, remains exact. The fixed 1024-byte stack bank adds
no retained/N-sized allocation. Numeric precedence, ordinary frozen PPML, OLS
subtraction, rank, weight generations and independent reconstruction remain.

Nine primary pairs give FW/unit/PW command medians 1.438→1.366 / 4.029→3.691 /
1.464→1.363 seconds; median paired gains are 5.92%/8.39%/6.64%. Separate original-
input diagnostics locate 99.293 ms in three FW accumulations (113.483→14.190 ms),
with unchanged independent certification. All 23 successful untraced pairs have
exact science and non-time solver decisions. Fourteen successful controls cover
weak graphs, slopes, three FE, low memory, saved outputs and width/sample guards;
largest added cost is 6 ms on small numeric. Primary peaks are exact/admitted;
one weak-PW peak is lower by 1.380 MB, without an established cause or memory claim.

The original topology-backed/exact-duplicate control fails r(430) on both artifacts
in both warm/full fits because the final Hessian changes canonical basis. All
failures remain; explicit checks verify no coefficients are published. This is
failed positive public coverage, not a speed result, and no rank/tolerance rule
was changed. Sixteen distinct core and 95 serial plugin tests, seven independent
OLS/dummy-Poisson/Hessian/sandwich fits per artifact, universal/static/arm64 ABI
checks pass. Both artifacts admit the generated width-eight control at exactly
18,154,399 usable bytes and reject one byte below with r(909). All 54 captures,
formatting/Mata/summary-assumption failures, source and artifact identities remain.

Seven P0–P7 items remain. Next audit generated OLS raw-residual consumers and
shared slope/three-plus-FE layouts, including the unchanged canonical-basis
failure as a coverage limitation; then remaining PPML state/construction, P7
scheduling and P0 complete accounting. R1–R5 are excluded. Independent reviews,
upstream CMG assessment and the bounded SCC campaign follow P0–P7. Installed
dist, unrelated STATUS additions and original untracked files remain; no agents,
cluster work, upstream change or push occurred.

## P6 generated RHS tiles: standalone prototype rejected

The [source-bound screen](benchmarks/reports/development/generated-rhs-tiles-local-2026-09-14/README.md)
rejects shared generated RHS tiles against accepted `fb4cc5b`. Candidate `6bbc5ace`
uses borrowed ordinary/topology factor codes and a fixed 128-byte interaction mask
for both prepared layouts and low-memory blocks. All 18 focused core tests, 95
serial plugin tests, five independent dummy/Poisson/Hessian/sandwich fits per arm
and universal/static/actual arm64 ABI checks pass. Original fixture compile,
base-policy, rebased-axis, PPML omitted-slot and archive-interpreter failures remain.

Three untraced pairs preserve exact scientific exports and solver decisions:
fast AW OLS 1.118→1.129 s, low PW clustered OLS 2.224→2.231, FW PPML 1.522→1.481.
None meets the prospectively fixed 5% and 10 ms gain gate. Three separate diagnostic
pairs preserve original science; fast AW RHS saves only 6.311 ms (31.432→25.121),
and its entire baseline scope is 2.81% of command time. All three native peaks
are exact and admitted; RSS rises slightly. No extra seeds/graph timing/private
audit/public exact-cap calls follow rejection. All twelve captures and the patch
are archived; all 178 accepted inputs and installed dist are restored/unchanged.

Seven P0–P7 items remain. Next target repeated generated raw-residual/predictor
consumers, then shared slope/three-plus-FE layout, remaining PPML state, scheduling
and complete accounting. R1–R5 are excluded. Independent reviews, upstream CMG
assessment and the bounded SCC campaign follow P0–P7. No agents, cluster or push.

## P4 guarded PPML Schur checkpoints: adopted local checkpoint

The [final source-bound report](benchmarks/reports/development/ppml-schur-checkpoint-adoption-local-2026-09-14/README.md)
and [ADR 0067](docs/adr/0067-guarded-ppml-schur-checkpoints.md) adopt candidate
`f4cb7dee` against evidence `7996a5e`/accepted native `7739e8e`. Eligible admitted
wide two-FE checkpoints reuse the actual-weight probe and retained Schur graph;
a fallible transient copy preserves original-input LSMR/tighter retry/MAP fallback.
Ordinary joint LSMR, canonical raw norms/exact tolerances, rank, weights, covariance,
reconstruction and publication policies remain unchanged.

Final review added combined capped-Schur and double-LSMR-certificate rejection,
which exposed stale scalar energy in the later MAP recurrence. The final shared
entry clears all scalar banks and preserves both fallback diagnostics. The
original failed source/test and preliminary `cfeed644` measurements are archived;
that artifact was not adopted. The unchanged 2e-10 independent test is green, and
all comparisons/oracles were rerun on the corrected artifact without pooling.

Nine primary pairs give unit/FW/PW command medians 2.312→1.679 / 3.186→1.910 /
2.007→1.716 seconds; paired median reductions are 27.9%/40.1%/14.3%. Native peak
medians fall 22–30% within unchanged models. Candidate auto beats accepted explicit
MAP on all three FW inputs by 32.3–35.6%. Eleven protected controls preserve science;
maximum added time is 6 ms on three FE. Separate exact-science traces show changing-
weight probe rejection on weak/bottleneck graphs and both probes rejected on cycle/
tiny-weight bridges. All attempted work remains included; P0's physical attempt,
copy/build and complete reporting work remains open.

All 67 distinct focused core and 95 serial plugin tests, nine native and three
independent Poisson/Hessian/sandwich fits, universal static and actual arm64 ABI
checks pass. Public explicit LSMR admits both artifacts at exactly 2,429,462,126
usable bytes and rejects one byte below with r(909). A saved-two-FE/mean control
improves 2.472→1.860 seconds; all 122880 original-row outputs match at unchanged
tolerances (maximum absolute difference 3.28e-10). The final 52 main and six
supplemental captures retain both expected admission errors and source/artifact
identities; the preliminary 58 captures remain separate. Installed dist, 97 unrelated
STATUS additions and 358 original untracked paths remain. No agents, cluster,
upstream changes or push occurred.

P4's weighted-Schur comparison is resolved; seven active P0–P7 items remain. Next
continue generated/interacted block evaluation and shared slope/three-plus-FE
layouts, remaining PPML construction/state work, P7 scheduling and P0 accounting.
R1–R5 stay excluded; two independent reviews, upstream CMG assessment and the
bounded SCC campaign follow P0–P7.

## P4 weighted Schur route screen: checkpoint opportunity, no new route

The [source-bound existing-route screen](benchmarks/reports/development/ppml-weighted-schur-screen-local-2026-09-14/README.md)
compares accepted native `7739e8e`/plugin `2689faf0` under auto/MAP/CMG. PPML MAP
already calls the same weighted Schur recurrence as OLS after its pilot; no new
engine is needed. Nine independent two-FE/16-control Poisson/Hessian/sandwich
fits pass across unit/FW/PW and all three routes, including independently formed
correction factors. Nine untraced calls at N122880/K50 preserve exact discrete
science, registered b/V/objective tolerances and admitted native peaks.

Auto/MAP/CMG seconds are 2.342/2.978/4.142 dense unit, 3.289/2.878/7.203 strong FW
and 2.263/2.451/3.109 dense PW. MAP wins the strong FW screen by 12.5% and has
lower peaks, but loses both dense screens. Strict/final scopes are markedly
cheaper with MAP (0.594/0.549/0.483 s versus auto 0.933/1.610/0.985 s).
Nine separate trace calls preserve exact original-route science. Full LSMR
projection-attempt widths are 52/51/1 for unit and 52/51/51/1 for FW/PW: both
weighted strict 51-RHS first attempts miss the unchanged 1e-12 certificate at
about 1.16e-12 and the existing 1e-14 recurrence retry passes. All failed-attempt
costs remain. Original missing checkpoint trace flags and display corrections
are disclosed; aggregated solver observations are not literal attempt counts.

All 18 captures and replay scripts are archived. This is a single-seed baseline
screen, not an adopted optimization or completed P4 Schur item. Next implement a
bounded current-weight Schur checkpoint candidate using the existing recurrence,
retaining ordinary joint LSMR, original-input fallback, independent canonical
certificates and joint memory admission. Then repeat the original comparisons
and protected cases. Eight active P0–P7 items remain; R1–R5, independent reviews,
upstream and SCC retain their previously stated scope/order. No source/artifact
replacement, agents, cluster work or push occurred.

## P4 mean validity/reuse: standalone prototype rejected

The [source-bound report](benchmarks/reports/development/ppml-mean-reuse-local-2026-09-14/README.md)
rejects mean reuse against evidence `ebf038f`, accepted native `7739e8e`.
Candidate `8a85e77c` tracks paired eta/mean bounds validity through accepted/trial
swaps, invalidating before partial writes and strict-candidate scratch use.
All 58 focused core tests, 95 serial plugin tests, ten independent licensed
Poisson/Hessian/sandwich/generated/narrow fits per arm and universal/static/actual
arm64 ABI pass. Four untraced pairs and three separate diagnostic pairs preserve
exact science and solver decisions; all untraced native peaks/models are exact
and admitted. The actual workspace adds 80 fixed bytes, with no heap/vector.

Complete/FW/PW/narrow screens are 7.600→7.251 / 2.458→2.375 / 2.198→2.177 /
1.446→1.432 seconds. Complete/FW meet the time-only 2% and 10 ms gate, but direct
all-call diagnostics show only 11.724/3.613 ms saved in mean construction; narrow
saves 11.583 ms. The entire baseline mean scope is only 0.20%/0.18%/1.02% of
those command times. Most apparent wide-target gain is in unchanged finite work,
so the attribution gate fails. No extra seeds/holdout timings or private audit
follow. Narrow sampled RSS rises about 51 MiB despite its exact native peak.

The rejected patch, diagnostic identities and all 14 captures are archived;
all 178 accepted inputs and installed dist are restored/unchanged. Eight active
P0–P7 items remain. Next compare weighted Schur with actual joint LSMR/MAP/CMG
PPML workloads, then finish generated/slope/three-plus-FE layout, P7 and P0 work.
R1–R5 remain excluded. Independent reviews, upstream CMG assessment and the
bounded SCC campaign follow P0–P7. No agents, cluster work or push occurred.

## P4 complete-frame refresh: standalone prototype rejected

The [source-bound screen](benchmarks/reports/development/ppml-frame-refresh-local-2026-09-14/README.md)
rejects combined response/weight refresh against accepted `7739e8e`. Candidate
`ddb302c6` removed duplicate old-weight/response validation and applied the exact
weight-bit/all-scale identity rule to ordinary calls, retaining invalidation and
prior-response storage. Eleven core checks, 95 serial plugin tests, ten independent
Poisson/Hessian/sandwich/generated/narrow fits per arm and universal/static/native
arm64 ABI pass. All four complete-command pairs preserve exact science and solver
fields, with exact admitted native peaks.

Complete/FW/PW/narrow screens are 7.408→7.496 / 2.509→2.419 / 2.219→2.214 /
1.427→1.458 seconds (+1.19%/-3.59%/-0.23%/+2.17%). None meets the prospective
5% and 10 ms gain gate. Weight/response phases move only a few milliseconds;
most FW savings are in unchanged finite work. No extra seeds/holdout timings,
tracing or private allocation audit followed rejection. The distinct one-control
request was fixed before timing on unchanged original full/warm data.

The candidate patch and eight captures remain archived; all 178 accepted inputs
are restored. The predictor optimization remains adopted. Eight active P0–P7
items remain. Next examine mean reuse across accepted/trial swaps and strict
scratch reuse, including bounds/error invalidation, then the remaining P4/P6/P7
and P0 work. R1–R5 are excluded; the two independent reviews, upstream CMG
assessment and bounded SCC campaign remain later steps. Installed dist and
unrelated work are preserved; no agents, cluster work or push occurred.

## P4 retained numeric predictor tiles: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/ppml-predictor-tiles-local-2026-09-14/README.md)
and [ADR 0066](docs/adr/0066-retained-numeric-predictor-tiles.md) adopt bounded
numeric finite-predictor tiles against `e65d6f9`, isolated plugin `2689faf0`.
Fresh/strict retained solves reuse the existing numeric bindings and installed
team, preserving selected-column FMA order with a fixed stack bank and the
original reconstruction/certification. Ordinary frozen PPML, generic fallback,
reference scales, generation validity, low-memory storage and one-shot OLS remain.

Three-seed complete/FW/PW command medians are 7.785→7.467 / 2.585→2.436 /
2.376→2.202 seconds; median paired reductions are 5.72%/5.76%/6.33%. Separate
exact-science diagnostics locate 82/90 ms in the three weighted predictor
accumulations; unchanged reconstruction/certification remains about 3.4 ms.
All 25 untraced A/B pairs, six A/A pairs and three diagnostic pairs preserve
exact science and solver decisions. Untraced native peaks are exact and admitted;
weighted sampled RSS rises slightly. This is no memory-saving claim.

Original low-FW costs +142/+171/+3 ms remain, alongside later +151/+155 ms flags.
Six interleaved identical-binary A/A pairs range -132 to +185 ms. Six new A/B
pairs have median -24 ms/-2.62%; all nine A/B pairs have median +3 ms/+0.29%,
passing the prospectively fixed median cost gates. Adopt with the variability
and individual flags disclosed, without a low-FW speedup or zero-cost claim.
The low changed kernel saves about 2 ms, not enough to explain those swings.

Validation passes 20 distinct core checks, 95 serial plugin tests, four positive
independent Poisson/Hessian/sandwich/generated-factor fits per arm, and universal/
static/native arm64 ABI. Initial wrong test import, zero-match selector and two
archive collector failures are retained. All 178 candidate inputs match main;
the installed plugin and 97 unrelated STATUS additions/358 untracked paths remain.

Eight active P0–P7 items remain. Next reduce duplicated PPML response/weight
refresh passes while preserving exact frame identity and invalidation; then
weighted Schur comparisons, remaining generated/slope/three-FE layouts, P7 and
P0 reporting. The user excludes R1–R5 from this run. After P0–P7, two independent
agents review changes, followed by upstream CMG small-components assessment and
the bounded SCC benchmark campaign. No agents, cluster work or push occurred.

## P3/P6 low-memory OLS score retention: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/low-score-retention-local-2026-09-13/README.md)
and [ADR 0065](docs/adr/0065-low-memory-ols-score-retention.md) adopt original
projected-column scratch retention against `26d05b0`. Ordinary low robust and
clustered OLS hand a private owner from fit to inference, outside the certified
result. The copy borrows the existing I/O bank before QR and takes it afterward;
actual post-QR identities select columns for bounded row tiles. Independent raw
means and score/covariance certificates remain. Optional file/copy failures keep
the uncached path. IID, fast and shared PPML session policies are unchanged.

Thirteen untraced pairs and eight diagnostic/cap calls pass science, with exact
paired full b and maximum V difference 1.58e-15. Three same-input weak two-FE AW
repeats improve medians 0.198→0.148 seconds (25.25% paired median); score storage
falls 56.40→4.37 ms for 0.40 ms copy cost. Single slope/three-FE saved/rank-deficient
robust controls improve 15.2%/17.3%/11.0%. Wide AW 40.737→38.702 is 4.99546%, below
the declared 5% gain gate; PW 0.929→0.962 costs 33 ms/3.55% and sampled RSS rises
0.157→0.207 GiB. No untraced pair exceeds both 5% and 10 ms added cost. The original
rank-deficient saved control used IID; it is preserved and supplemented by robust
inference on the exact same data, with rank 49 selected from 51 generated columns.

The larger scratch footprint is accepted: logical peak becomes 16*N*G rather
than 8*N*max(G,2*R). The rank-deficient robust case rises 24.08→25.07 MB scratch
while native peak falls 40.59→38.20 MB. There is no general native/RSS saving claim
or new resident N-by-K copy. The weak public 9.41 MB execution base excludes
solver reservations. Current-model replay gives 75,268,168 usable bytes for LSMR
and 13,911,784 for CG; both original plugins pass both shared caps. At the smaller
cap both select CG and peak near 11.63 MB, with full science preserved and capped
versus unrestricted V difference at most 1.92e-13. Broader P0 reporting remains.

Validation passes 38 focused core checks, 95 serial plugin tests, 49 candidate
positive licensed fits, four baseline supplement fits and universal/static/native
arm64 ABI. Rank-selected/corrupt scratch, donor-buffer return after failed writes,
cleanup and independent dummy/sandwich/CGM checks are covered. The original
unsupported factor-by-continuous request fails closed on both plugins; a distinct
supported generated-interaction fixture passes both. Initial missing-import and
archive fixture-lookup errors are preserved separately. All 178 candidate inputs
match main and plugin `365bee73`; installed dist and unrelated work are unchanged.

This closes P3 low-memory duplicate residualization and P6 bounded column/scratch
handoff. Thirteen #27 items remain. Next examine remaining PPML construction,
weight/response/predictor passes and generation-valid reuse, then its weighted
Schur comparison, remaining P6/P7 and R1–R5. Complete #27 before the authorized
agents, upstream CMG assessment and small SCC campaign. No push or cluster work.

## P6 saved OLS final-target recovery: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/saved-schur-adoption-local-2026-09-13/README.md)
and [ADR 0064](docs/adr/0064-saved-ols-final-target-recovery.md) adopt the five-file
saved-Schur/recovery extension against accepted `bc5a6ae`. Public one-shot OLS
can recover requested categorical effects from its certified final target when
an accepted Schur solve omits initial coefficient products. Rejected probes keep
prior retention/routing; shared PPML defaults, weighted-operator/width guards,
continuation budgets and independent final 1e-14 output precision remain.
Intercept recovery attempts now enforce joint admission, including finishing
and SD/LSMR overlap, with the existing cheaper MAP fallback. Slopes/PPML retain
existing recovery policies; broader accounting remains open.

All 23 pairs pass full science and saved-output comparisons. Three-seed unit/AW/
PW command medians fall 3.012→0.809 / 83.397→68.675 / 3.641→2.676 seconds; median
paired gains are 73.5%/17.7%/26.8%. Native peak medians fall 54.9%/31.2%/30.9%.
Initial absorption supplies the main gains; final precision, recovery, complete
64-step attempts and CG continuations are included. Rejecting unit/PW seeds add
54/20 ms (1.80%/0.55%); rejecting AW adds 3.851 seconds/4.62%, mostly in unchanged
CG for an unestablished reason. No pair exceeds both 5% and 10 ms added cost.
The weighted cut rejects with exact science/peak and costs +10 ms/10.1%.
Earlier standalone admission +36/+43 ms flags remain historical and unexplained.

Disconnected/unbalanced/automatic-CMG saved controls improve 30–39% command time
but native peaks rise 19.97→36.54 / 19.93→26.06 / 17.50→32.87 MB. An isolated
allocator diagnostic reproduces exact candidate science and locates all these
peaks in final LSMR absorption; recovery peaks near 8.05 MB. At 35,148,872 usable
bytes, those three plus weak-d3 use the existing admitted final CG continuation
and peak at 15.2–17.3 MB. Independent full b/V and saved-output checks pass;
maximum cap/unrestricted output difference is 2.39e-12. The unrestricted memory
tradeoff is accepted with the proven bounded alternative, without a seed-specific
guard. Public execution-base reporting is still not a whole-action bound.

Validation passes 124 focused core checks, 95 serial plugin tests, 48 saved dummy
fits with eligible-Schur assertion, four weighted omission fits, exact/one-byte-
short recovery/CG/generic and original unit/PW initial public boundaries, plus
universal/static/native arm64 ABI. Initial unit/PW bounds are 827753800/831179336
usable bytes with admitted peaks. The oracle harness's 16-versus-17-coordinate
failure is preserved; independent grounded-dummy/SVD response influence extends
weak/disconnected/unbalanced oracles to the normalized intercept/full covariance.
Completed captures were reused by identity; independent LSQR/LSMR still agree.
No seed/input/tolerance was replaced. All 178 candidate inputs match main and
plugin `8bfa9fc2`; the diagnostic artifact is separate. The report archives 97
capture entries (58 command calls and 39 oracle CSVs), source and log identities.
Installed dist, 97 unrelated STATUS additions and 358 original untracked paths
remain intact. No agents, cluster, upstream update, qualification or push.

This closes only P6 final-target saved-FE recovery; 15 #27 items remain. Next
continue P3's low-memory scratch/duplicate projection audit, then generated and
sloped layouts, PPML pass/state/Schur work and R1–R5. Complete #27 before the
user-authorized two-agent investigation, upstream CMG review and small SCC run.

## P6 final-output precision: adopted on repaired memory admission

The [source-bound adoption](benchmarks/reports/development/ols-output-precision-adoption-local-2026-09-13/README.md)
and [ADR 0063](docs/adr/0063-ols-derived-output-final-precision.md) adopt the
three-file precision repair against `58bcf3a`. Isolated universal plugin
`14b396c7` binds 178 inputs. Requested residual/absorbed/combined-prediction/saved-FE
outputs use a separate final target at most 1e-14 in fast/low/projection-only fits,
independent of coefficient retention. Terminal CMG releases its hierarchy before
an admitted certified CG correction. Initial projection, reference scales, rank,
weights/sample/DoF and public tolerances remain unchanged.

All 109 focused core checks, 95 serial plugin tests, 48 saved dummy fits, four
weighted omission cases and universal/static/native arm64 ABI checks pass. Public
CG/generic exact and one-byte-short boundaries pass with admitted native peaks.
The original saved slope succeeds at 33268760 usable bytes with 28906404 observed,
using certified CG after memory-rejected final LSMR. One byte below rejects with
prior estimates/sample/order/output absence preserved; constrained saved values
match the previous unrestricted strict candidate within 1e-14. The original tiny
weighted cut passes all 30720 rows against the preserved high-precision/direct
oracles: maximum residual error 3.05e-13, full b/V and both saved FE pass.

All 19 paired commands pass candidate science, including the original thirteen
cases and six prescribed reversed follow-ups. Six baseline saved-output errors
remain independently adjudicated and excluded from accuracy-matched speed claims.
Repeated slope/weighted-cut/automatic-CMG added costs are 28/53/18 ms medians.
Unit/PW add 91/116 ms in their original pairs, largely final LSMR setup/solve.
Unrestricted slope native peak rises 28.87 to 38.54 MB; the constrained route
above fits. Other original peaks are exact except three FE +78933 bytes, low FW
+149 bytes and weak -1072016 bytes, without causal attribution. Base/scratch
models are unchanged and RSS moves both ways. This is a correctness adoption,
not a speed or general memory improvement. Earlier failed candidates/concurrent
plugin failure remain historical evidence, not fixes claimed by the serial run.

Next rebase the saved-Schur/final-target recovery prototype on this precision
baseline and repeat its original comparison. It remains unadopted; all 16 open
#27 items, broader memory reporting and specialized slope/recovery accounting
remain. The installed plugin, 97 unrelated STATUS additions and 358 original
untracked paths are preserved. No agents, SCC, upstream update, qualification
or push occurred.

## P0 generic owned-FE admission: adopted local repair

The [source-bound report](benchmarks/reports/development/generic-owned-fe-admission-local-2026-09-13/README.md)
adopts [ADR 0062](docs/adr/0062-generic-owned-fe-memory-admission.md) against
accepted numerical `f576bc5` at evidence HEAD `7ae569d`. The isolated universal
candidate `ad725932` binds 178 inputs. Owned-design construction and retained
codes/slopes/memberships/unit weights now enter joint admission. Generic MAP,
CG, accelerators and LSMR check their actual route, including explicit fallback
copies and separately admitted CG continuation after rejected LSMR. A cheap MAP
pilot need not reserve unused LSMR. Zero-control fits charge three overlapping
caller observation vectors. Numerical rules and the categorical model remain.

Four accepted-source red regressions are green. All 134 focused core checks,
95 serial plugin tests, 48 saved dummy fits, four weighted omission cases and
universal/static/native arm64 ABI checks pass. The public AW intercept/slope
fixture with zero finite controls matches independent dummy residuals. Its exact
cap leaves 806328 usable bytes and observes 730668 native bytes; one byte below
admission returns r(909) and preserves prior estimates/sample/order/output absence.
The superseded 708024-byte estimate failed the observed-peak check. Separate
allocator tracing localized the missing caller overlap; its diagnostic artifact,
failed estimate and initial build/collector corrections remain archived.

Ten fixed paired commands preserve exact scientific exports and OLS non-time
solver fields. Original slope auto/LSMR/CG changes are +2/+3/-1 ms; none crosses
the prospective 5% and 10 ms review threshold. Native peaks are exact in nine
pairs; weak saved OLS adds 530456 bytes (1.6%) for an untraced reason. Public
bases/scratch remain unchanged and RSS moves both ways. This is a correctness
repair, with no claimed speedup or general memory reduction.

Next rebase and validate the separate final-output precision repair, then resume
saved-Schur/recovery comparisons. Both prototypes remain unadopted. Specialized
slope MAP, recovery, whole-action reporting and opaque factor-storage bounds
remain separate P0 work; all 16 unresolved #27 items remain open. The installed
plugin, 97 unrelated STATUS additions and all 358 original untracked paths are
preserved. No agents, SCC, qualification, dependency update or push occurred.

## P0/P6 saved-output precision: science passes; generic admission prerequisite

The [source-bound investigation](benchmarks/reports/development/ols-saved-residual-precision-local-2026-09-13/README.md)
preserves four precision candidates and the separate
[saved-Schur failures/oracles](benchmarks/reports/development/saved-schur-residual-precision-local-2026-09-13/README.md).
Accepted numerical source remains `f576bc5`; neither the three-file precision
repair nor the archived five-file Schur/recovery optimization is adopted.

The final isolated precision artifact `76858524` binds 176 inputs. Requested
residual-derived outputs receive a separate final tolerance at most 1e-14 in
fast/low/projection-only fits. Terminal CMG releases its hierarchy before checked
CG correction. The original tiny weighted cut passes all 30,720 rows against an
80-digit grounded oracle and independent LSQR/LSMR: maximum residual error
3.05e-13, versus 1.76e-9 and 28 failing rows at the preceding 1e-12 target. Full
b/V, saved FE, rank and DoF pass at unchanged public tolerances.

All 97 focused core tests, 48 saved dummy fits, four weighted omission cases,
public exact/one-byte-short CG caps, universal build and native ABI checks pass.
The initial plugin suite passes 95/95; a final concurrent rerun passes 94/95 with
one unexpected PPML warm-allocation event. Ten fixed exact checks per accepted/
candidate binary and one complete serial 95-test suite per binary all pass.
The failed concurrent result and unresolved cause remain evidence; no test or
PPML production code was weakened. Timings start only after the serial gates.

Nineteen complete cost pairs include all thirteen planned inputs and six
prescribed reversed follow-ups. Candidate science passes throughout; independent
oracles adjudicate six original baseline output-error cases, excluded from
accuracy-matched speed claims. Median added costs are slope +27 ms/7.6%, weighted
cut +54 ms, and automatic CMG +19 ms. These are correctness costs, not speedups.
The slope final projection switches MAP→LSMR and native peak rises 28.87→38.54 MB
against an unchanged public 18.93 MB execution base. Full phases, peaks, scratch,
RSS, fallback/warning fields and all failed/superseded captures are archived.

Source inspection and four isolated accepted-source closed-form regressions
prove that generic owned-FE auto/MAP/CG/LSMR ignore the supplied joint workspace
budget. Each unrestricted residual matches the oracle, then incorrectly succeeds
at a one-byte budget. This pre-existing gap blocks precision adoption because
the new final slope LSMR workspace needs checked admission. The initial missing
test-fixture build failure is preserved; exact accepted fixture bytes produce
all four intended red assertions.

Next implement checked generic owned-design and solver/fallback admission, test
the original slope and constrained boundaries, then resume precision adoption
and the saved-Schur comparison on that repaired baseline. Do not drop slope
coverage or relax precision to evade the gap. P0 reporting, P6 and the other
#27 items remain open. All 97 unrelated STATUS additions, 358 original untracked
paths and the protected installed plugin remain preserved in this step. No
agents, SCC, qualification, dependency update or push.

## P6 prerequisite: omitted-column output publication repaired

The [source-bound repair](benchmarks/reports/development/ols-derived-output-omissions-local-2026-09-13/README.md)
adopts [ADR 0061](docs/adr/0061-ols-derived-output-omitted-columns.md) against
accepted `834703b`. OLS derived outputs now skip generated columns carrying the
existing FE-collinear or numerical-rank-deficient status and no coefficient.
Canonical slots, rank, zero public coordinates and all numerical fit/inference
and transaction policies remain. The isolated one-file candidate `fe066c02`
excludes the saved-Schur/recovery prototype and binds 176 inputs.

A closed-form regression fails at the original selector and passes in fast/low
modes after repair. The initial post-fix test's wrong wire-status assertion is
preserved and corrected to existing status 3. All 95 plugin tests, four weighted
independent omission/output oracle cases per artifact and 48 ordinary saved-output
fits pass. The exact original slope and rank-deficient saved warm/full requests
now succeed, retaining ranks 15/49 and independent native certificates. Universal
build/static and actual arm64 ABI checks pass. Public base-model gaps on those
functional requests remain P0 work.

Eight cost pairs retain exact full science/solver fields and native/base/scratch
peaks. PW adds 819 ms/21.6%, then reversed repeats change -70 and +1961 ms/44.5%.
The excess lies mainly in unchanged initial CG; its cause is not established.
Four prescribed follow-ups keep A/A separate: A/A +132/-9 ms; A/B +48 ms/1.35%
and +9 ms/0.25%. All flags remain disclosed. This is a correctness adoption with
timing uncertainty, not a speed or universal memory claim.

The [expanded saved-Schur screen](benchmarks/reports/development/saved-schur-controls-local-2026-09-13/README.md)
preserves the still-unadopted five-file prototype on `834703b`. Nine original
primary pairs pass: unit/AW/PW command medians 2.896→0.694 / 82.925→65.283 /
3.499→2.582 seconds, paired gains 76.1%/22.3%/25.8%, including rejected probes
and AW/PW continuation costs. Median native peaks fall 54.9%/31.2%/30.9%.
Fifteen core/five plugin checks, 48 independent saved fits and initial/recovery
exact/one-byte-short admission checks pass; full overlap audit and limitations
are archived. Five protected pairs have exact science and at most +5 ms.

Weak, disconnected and unbalanced saved comparisons fail baseline parity.
Grounded direct dummy/SVD and separate LSQR/LSMR oracles agree within 9e-13;
candidate residual/FE outputs all pass the original thresholds. The baseline
misses residual tolerance on 1/1/155 rows and first-FE tolerance on 216 unbalanced
rows. These three adjudicated comparisons remain failed parity evidence, outside
passing timing aggregates. The subsequent slope warm-up exposes the publication
bug repaired above; no failed input or seed was replaced.

Next rebase the saved-Schur/recovery prototype on this omission repair and finish
the original adoption comparison. The old screen's three-FE saved, deficient-rank
saved and explicit-CG saved pairs remain unexecuted; the fixed original slope
pair also needs comparison. Retain all baseline-error adjudications and timing
flags. P6 adoption/P0 whole-action reporting and other #27 items remain open.
Installed dist, 97 unrelated STATUS additions and all 358 original untracked paths
remain preserved. No agents, SCC, qualification, dependency update or push.

## P0 prerequisite: prepared direct-CG admission repaired

The [source-bound report](benchmarks/reports/development/prepared-cg-admission-local-2026-09-13/README.md)
adopts [ADR 0060](docs/adr/0060-prepared-direct-cg-memory-admission.md). Simple and
categorical direct-CG adapters now enforce the existing joint execution/CG
budget, including retained coefficient recurrence and the installed team. The
isolated candidate `fdc3e141` changes two files against accepted `dadd956` at
`de6988b`; it excludes the unadopted saved-Schur/recovery prototype. Arithmetic,
reference scales, weights/sample/rank/VCE/DoF and certificates remain unchanged.

The closed-form missing-admission test fails before the fix and passes afterward
across 36 dispatch/weight/topology/worker combinations. All 65 isolated prepared-
fit tests, 48 saved-OLS dummy fits and one/seven-worker weighted direct-CG oracle
checks pass. Public one-byte-short admission returns r(909) with prior results,
sample/order and requested-output absence preserved. Exact admission succeeds
at 296259 native bytes within 311496 usable. Both architecture builds and native
arm64 ABI checks pass. Eight fixed paired commands preserve exact scientific
exports and solver decisions; largest increase is PW +18 ms/0.5%. Explicit-CG
pairs are +12 ms/0.4% and +4 ms/1.9%. None triggers repeats. Native peaks are
exact except low FW -4608 bytes; public base/scratch models remain unchanged.
RSS moves both ways. This is a correctness repair, not a speed or memory gain.

A separate three-request diagnostic on the unadopted saved prototype records
actual initial joint checks: unit 827.75 MB and PW 831.18 MB, versus observed
378.71/581.41 MB and public bases 418.80/424.69 MB. PW rechecks CG continuation;
low FW checks CMG construction/retention. Recovery planning takes 3.334/3.250
microseconds on the full saved requests, so its direct scalar cost cannot
explain earlier 36–43 ms increases. Their cause remains unresolved; no flagged
observation is discarded. Full recovery remains 0.385/0.622 s in these separate
diagnostics, which are excluded from ordinary timing comparisons.

Next rebase the saved-Schur/recovery prototype on this repair, complete its
initial/final overlap admission review, and compare complete saved-output cost
against the repaired accepted baseline. P0 reporting/other-route accounting and
P6 adoption remain open; the five prior prototype files remain uncommitted.
Installed dist, 97 unrelated STATUS additions, original untracked paths and
separate Veneto work remain preserved. No agents, SCC, qualification or push.

## P0/P6 recovery admission tested; initial-peak diagnosis corrected

The [source-bound recovery report](benchmarks/reports/development/saved-recovery-admission-local-2026-09-13/README.md)
records an unadopted five-file extension to the saved-Schur prototype. Plugin
`247f889a` binds 176 inputs; both architecture builds/native arm64 ABI, 13 core
recovery tests, five plugin saved tests and all 48 independent saved-OLS oracle
fits pass. Each intercept-recovery route has a checked incremental reserve.
An over-budget CG/LSMR attempt can still use the existing admitted MAP fallback;
valid retained coefficients need no unused Schwarz reserve. PPML/slopes retain
their existing policies. One/four-worker exact-boundary and failed-product tests
pass. The public cap fixture rejects one byte below admission with r(909),
preserves prior estimates/sample/output identities, and succeeds at the exact
cap: 362557 native bytes versus 381696 usable. This is scoped boundary evidence.

The earlier score-cache-overlap diagnosis is corrected: ordinary robust/clustered
inference consumes and drops its score design before recovery. A separate exact
original-PW allocator diagnostic reaches its 581.41 MB whole peak beforehand;
recovery peaks at 140.70 MB and adds 13.78 MB above entry. Recovery takes 0.697 s,
with exact full science and saved values. This diagnostic is not pooled into
command comparisons. The public 424.69 MB base omits the initial solver reserve;
remaining initial-peak reporting and complete overlap accounting are P0 work.

Twelve incremental cost pairs include six controls/screens, two triggered seeds
and four bounded A/A/reversed-order follow-ups. All preserve exact science,
solver fields, native peaks, models and scratch. The original accepting unit
case slows +36 ms/5.3% and reversed +43 ms/6.1%. One rejecting seed adds 333 ms,
mostly in earlier CG, but its reversed pair is -7 ms. A/A moves +23/-11 ms.
The accepting-case cost signal remains unresolved; no admission speedup or
combined-prototype adoption is claimed. PW/low observed/model gaps remain.

The five changed source files remain uncommitted; accepted numerical baseline
is `dadd956`. Next resolve initial-solver peak reporting/admission overlap and
localize the accepting-unit cost before primary saved-Schur adoption comparisons.
Historical receipts, installed dist, unrelated Veneto work and original untracked
paths remain preserved. #27 is unfinished; no agents, SCC, qualification or push.

## P6 original PW screen resumed; recovery admission remains open

The [repaired-baseline screen](benchmarks/reports/development/saved-schur-probe-local-2026-09-13/repaired-baseline/README.md)
reapplies the archived two-file saved-Schur prototype against accepted `dadd956`.
Plugin `18427c19` binds 176 inputs, passes both architecture builds/native ABI,
the focused guard test and all 48 independent saved-output oracle fits. The
identical baseline plugin's 48-fit oracle receipt is retained from the CG repair.

The exact original PW missing/two-way-cluster/partial-saved pair now completes:
3.498→2.518 seconds (−28.0%), native peak 841.84→581.41 MB (−30.9%), input near
0.081 s in both. Full b/V/residuals are exact, saved FE maximum difference is
6.05e-10 within unchanged tolerances, and samples/status/DoF/certificates pass.
The probe accepts at 0.00794698, but its full Schur attempt reaches the 64-step
cap and continues with CG: candidate 138 = 8 probe + 64 Schur + 66 CG iterations,
versus baseline 66. Both fallback flags remain set. Reported command/peak includes
that discarded attempt and final recovery; this is one pair, not adoption.

Both peaks exceed the unchanged 424.69 MB model. This screen's proposed
score-cache-overlap diagnosis is corrected by the newer source/allocator audit
above: the ordinary inference cache is already dropped and the whole peak
precedes recovery. Recovery admission and constrained rollback now have focused
evidence, while initial-peak reporting and an incremental cost signal remain
open. The combined prototype remains unadopted; accepted numerical baseline
remains `dadd956`, installed dist and unrelated Veneto changes are preserved.

## P6 prerequisite: negative CG energy independently audited

The [source-bound repair](benchmarks/reports/development/cg-energy-audit-local-2026-09-13/README.md)
is adopted under [ADR 0059](docs/adr/0059-borrowed-cg-negative-energy-audit.md),
against accepted numerical `257f4ce` at evidence HEAD `d4d7e97`. Candidate plugin
`e831a657` binds 176 inputs; only borrowed symmetric CG changes. A previously
fatal small negative Hestenes subtraction now requires an independent scaled,
compensated check of current and pending weighted energy. Larger/inconsistent
steps still fail; original recurrence stopping, reference scales, FE and strict
reconstruction certificates remain. The calculation adds no heap workspace.

The exact original PW missing/two-way-cluster/partial-saved worker now succeeds
in 3.510 seconds. Separate tracing confirms RHS 51's two-ulp overshoot and zero
independent remaining energy. This is a correctness repair; the failed baseline
is not a timing denominator. The final 45-row closed-form fixture fails on the
isolated accepted core and passes with the repair. All 57 focused Rust tests,
48 saved-OLS dummy fits per artifact and both independent Poisson fixture arms
pass. Both architecture builds and native arm64 ABI checks pass. Initial fixture
expectation/overflow failures, a missing-oracle setup error and collector-field
corrections remain disclosed; no existing test/public tolerance was loosened.

All twelve protected pairs preserve exact full science, sample/status/DoF,
solver decisions, native peaks, models and scratch. Largest absolute increase
is saved unit 2.890→2.938 seconds (+1.7%); dense increases 5 ms/14.7%. No increase
crosses max(10 ms,5%); no extra seeds were triggered. The saved-unit fallback
flag and 59 iterations remain exact. RSS moves both ways. Existing low/saved
observed/model gaps remain; repaired PW reaches 841.84 versus 424.69 MB. These
are unresolved P0/P6 gaps, not memory qualification.

Next reapply the archived saved-Schur prototype against this repaired baseline
and resume the original PW screen, then prove overlapping final-recovery/
fallback admission and constrained budgets before adoption and primary repeats.
The prototype remains unadopted. Other #27 items remain open; finish those before
the two-agent investigation, upstream CMG review and authorized later small SCC
campaign. Installed dist, unrelated Veneto work and historical receipts remain.
No agents, SCC, qualification or push occurred.

## P6 saved two-FE Schur: archived prototype and baseline failure

The [source-bound probe](benchmarks/reports/development/saved-schur-probe-local-2026-09-13/README.md)
tests ordinary OLS permission to omit its initial coefficient batch only after
the existing weighted Schur probe accepts. Rejected probes retain coefficients;
other weighted-linear callers keep their default requirement. Baseline numerical
source is `257f4ce` at evidence HEAD `75ad177`. Prototype plugin `002a3089` binds
176 inputs, passes both architecture builds/native arm64 ABI, the focused core
guard test and 48 independent saved-output dummy-oracle fits per artifact.
Eligible weighted two-FE oracle fits demonstrate actual new Schur dispatch.

The original unit saved-both-channel screen falls 2.897→0.650 seconds, with
native peak 839.05→378.71 MB. Input time stays about 0.075 s; native falls
2.784→0.538 s and initial absorption supplies the savings. Full b/V and saved
values pass original tolerances; sample/status/DoF remain exact. This is one
pair, not adoption or a cumulative speed claim. Its baseline exceeds the public
418.80 MB model; its candidate fits, but complete recovery admission is unproven.

The original degree-two AW operator rejects Schur: contraction 0.0106146 exceeds
0.01. Both commands spend about 84 seconds, with exact b/V/fit/certificate/saved
exports and unchanged 3363.96 MB native peak, above the 1675.12 MB public model.
The candidate adds the charged eight-step probe. The harness initially assumed
every nominal strong graph must select Schur; that assertion failure and its
correction remain preserved. No original AW pair was replaced or rerun.

The original PW missing/two-way-cluster/partial-saved baseline fails r(459)
before the candidate runs. Exact noisy-worker replay identifies a CG Hestenes
negative-energy failure during absorption. Two direct replays pass; the cause
of that worker/direct difference remains unestablished. A separate accepted-
baseline scalar diagnostic (plugin `200370b2`) reproduces the error on RHS 51,
the explicit constant: previous 139583.428571428522, decrease
139583.428571428580, remaining −5.820766e−11 versus floor 3.099375e−11. This is
a two-ulp overshoot. The diagnostic changes no arithmetic or error policy and
passes both architecture builds/native ABI. All failures and setup mistakes
remain in the report; no failed command is counted as a pass.

The P6 prototype is archived and removed. Its prerequisite energy repair is
now adopted in ADR 0059 above. Resume the exact original PW screen against that
repaired baseline, resolve overlapping final-recovery/fallback memory admission
and constrained budgets, and complete the prescribed protected/repeated
comparisons before adopting the saved-Schur optimization.
P6 and the remaining #27 work stay open; no target/seed substitution, agents,
SCC, qualification or push. Installed dist and unrelated Veneto work remain.

## P6 selected-output mask: evaluated and rejected

The [isolated report](benchmarks/reports/development/selected-output-mask-local-2026-09-13/README.md)
rejects the standalone three-file recovery mask change against accepted numerical
`257f4ce` (evidence HEAD `25b1bb3`). Plugin `79ff38b2` binds 176 inputs, passes
both architecture builds/arm64 ABI, all 17 topology/recovery tests and 48
independent explicit-dummy saved-output fits per artifact. Unit/AW/FW/PW,
one/two FE, fast/low, partial/combined outputs and rollback pass.

The three prescribed screens are 0.207→0.207 / 0.329→0.315 / 0.409→0.412 seconds
for unit saved, AW low saved and PW two-FE clustered partial saved. None meets
the fixed command/native-peak threshold. Full b/V, sample/status/DoF/FE/cluster
and saved values are exact. Unit/AW native peaks are unchanged and admitted.
PW falls 398.11→394.59 MB for an untraced reason, but both exceed the unchanged
107.15 MB model; this is a P0 gap, not memory acceptance. PW sampled RSS rises.
The separate AW profile is 0.310→0.313 seconds with unchanged native peak and
no isolated recovery-allocation timer. No additional seeds or protected
performance campaign ran after rejection. The full patch and receipts are
archived and all 176 accepted build inputs restored; no production source
changes remain from either P5 or this mask experiment.

P6 saved outputs remains open. The next substantive comparison is fast two-FE
saved output without retaining all initial projection coefficients, using the
existing accepted-final-target recovery. Include final recovery/fallback cost
and resolve its overlapping memory accounting; one-FE Auto already omits that
initial coefficient batch. The remaining P0/P3/P4/P6/P7 and R1–R5 work precedes
the authorized agents, upstream CMG review and later small SCC campaign.
Installed dist and unrelated Veneto work remain unchanged. No push.

## P5 centered tiles: evaluated and rejected for native adoption

The [final QR/publication report](benchmarks/reports/development/centered-tiles-probe-local-2026-09-13/qr-public/README.md)
concludes the bounded centered one-FE evaluation against accepted numerical
`257f4ce` (evidence HEAD `436fe2a`). Stable current-frame means, streamed finite
moments, existing rank-revealing scratch QR, original-row residuals and centered
robust/clustered scores were implemented and tested in the archived prototype.
They are not adopted. The conditional PPML extension is declined because there
is no acceptable OLS winner; R3 may revisit a materially different mechanism.

The earlier actual-dispatch screens saved 40–43% native peak. Bounded parallel
Gram/FE certificates reduced initial overhead, but full-rank IID still missed
the prescribed memory/time tradeoff (0.573→0.631 seconds, +10.1%). Those stages
and their failures remain immutable in the same report tree.

The near-capacity QR repair was then built as plugin `c8817a0b`. Its first public
unit-IID candidate returned r(498): precommit required fast mode to report zero
scratch. A separate diagnostic confirmed that cause and previous e(b) rollback.
An experimental origin flag and exact final-axis/file-bound check repaired this
reporting boundary without changing ordinary fast/low checks. Final plugin
`840092dd` binds 182 inputs and passes both architecture builds/arm64 ABI.
Thirty-five centered core and 46 plugin tests pass (81 distinct). An unavailable
Cargo launcher and two report-comparison fixture failures remain disclosed.

All five prospective public QR controls now pass science but violate the command
cost envelope. Near unit/AW/FW/PW commands are 0.034→0.051 / 0.047→0.063 /
0.046→0.062 / 0.046→0.065 seconds (+34–50%). Wide deficient AW is 1.366→1.898
(+38.9%, +532 ms). Native peaks fall 30–41% and fit unchanged models; AW/wide
sampled RSS rises. Two separate profiles confirm excess fit cost; wide score
construction savings do not offset it. Full b/V and independent certificates
pass, with exact sample/rank/status/DoF/FE/cluster structure. Scratch capacities
are 2.23/40.11 MB; actual I/O traffic is unmeasured. Full overlapping admission
proof remains incomplete. Cost rejection stops adoption before broader controls,
repeat primary seeds or a PPML extension; no failing result is discarded.

The rejected route, report extension and prototype-only modules were removed
from main after the complete patch/source/artifact/numeric evidence was archived.
All 176 accepted baseline build inputs matched before isolating the independent
P6 selected-output candidate in three files: `fe/topology_slope_map.rs`,
`fe/mod.rs` and `ols/prepared_categorical_data.rs`. That unadopted candidate only
forwards the requested combined-output mask to the existing recovery finisher;
its earlier independent evidence remains in the scope/saved-output stage.
Next test and measure it separately, then continue remaining #27 P0/P3/P4/P6/P7
and R1–R5 items. P6 saved outputs remains open. Complete #27 before the two-agent
investigation, upstream CMG assessment and later small SCC campaign.

Installed dist, the 97 unrelated STATUS additions and all 358 original untracked
paths remain preserved. No agents, SCC, qualification or push occurred.

## P3 cluster-state reuse: evaluated; certificate prototype rejected

The [source-bound report](benchmarks/reports/development/cluster-certificate-local-2026-09-13/README.md)
rejects representative-tuple certification against accepted `257f4ce`. Dense OLS,
sparse OLS and sparse PPML first-screen commands fall 0.318→0.308 / 0.172→0.170 /
0.363→0.358 seconds: none meets the prospective joint 5%/10 ms threshold. No
additional primary seeds ran. All nine pairs (three screens, four controls and
two separate public profiles) preserve exact scientific exports and solver
fields. Largest protected slowdown is low AW saved-output OLS +3 ms/4.1%.
The sparse OLS profile saves 6.4 ms in preparation; that does not overturn the
command-screen rejection. All three primary native peaks are unchanged/admitted,
while sampled RSS rises. Two control peaks differ for untraced reasons.

The small-slope and weak OLS controls exceed their unchanged models on both
artifacts: 1,388,568 versus 1,382,472 bytes, and 27.49/27.91 versus 13.64 MB.
These are recorded P0 gaps, not memory acceptance passes. Three focused unit
tests, both independent OLS/Poisson interaction oracle arms, universal static
checks and native arm64 ABI checks pass. Initial compilation and collection/checker
failures remain disclosed. All 179 accepted inputs are restored, installed dist
is unchanged, and unrelated Veneto work is preserved.

The [consumer audit](benchmarks/reports/development/cluster-certificate-local-2026-09-13/consumer-audit.md)
completes the bounded cluster-state reuse evaluation: final codes/subset orders
already serve their production and certificate consumers; the identified extra
categorical factorization alternative is rejected. Low-memory QR overwrites
within scratch, so direct inference handoff would be incorrect. That broader
P3 item remains open. Next prototype stable one-FE centered tiles through the
existing moment/rank boundaries, preserving reference scales and QR fallback;
P0 and other P4–P7/R1–R5 work remain. No agents, SCC, qualification or push.

## P3/P4 exact wide covariance pivots: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/ppml-pivot-cache-local-2026-09-13/README.md)
adopts exact upper-row maximum reuse in PPML Jacobi eigensystems above 64
identified dimensions against `d9aa8c0`. The existing eigenvalue vector holds
temporary maxima within its modeled lifetime; only affected rows are rescanned.
Pivot/tie order, rotations, stopping, clipping/rank policy and independent
certificates remain unchanged. There is no new heap allocation, retained state,
worker team or SPI call. [ADR 0058](docs/adr/0058-exact-ppml-jacobi-pivot-cache.md)
records the boundary.

All 19 planned pairs pass: nine primary pairs over three seeds, nine protected
controls and one separate private audit. Unit K65/FW K129/PW K257 command medians
fall 1.637→1.597 / 3.148→2.264 / 14.784→4.072 seconds; median paired gains are
2.4%/28.0%/72.3%. Adoption rests on the repeated K129/K257 gains. Their inference
medians fall 0.810→0.230 / 8.609→0.827 and publication 0.347→0.055 / 3.239→0.320
seconds. Baseline instrumentation attributes most eigensystem time to exhaustive
pivot search; separate standalone whole-inspection timings are not pooled with
command evidence.

Every scientific CSV is byte-identical, with exact sample/rank/status/DoF,
cluster/separation/PSD reports and discrete solver fields. Native peaks are exact
in 18/19 pairs, including all primaries, and every peak is admitted. Three-FE
peak rises 24,715 bytes to 38.510 MB against a 58.377 MB model; its allocation
source is untraced. All modeled phases are exact. Sampled RSS moves both ways;
no memory saving is claimed. Largest protected slowdown is small K2 +2 ms/7.4%,
within the review envelope. The private audit falls 19.344→8.623 seconds while
retaining zero events/live/peak growth over 16 iterations, stable capacities and
fresh/warm equivalence. Retry/fallback counts are zero throughout.

Forty-nine distinct core tests, 22 PPML plugin tests, both independent dummy
Poisson/Hessian/sandwich fixture arms and the original wide profiled-Poisson/
full-CGM oracle pass. The latter retains b/V errors 3.97e-15/2.87e-16. Isolated
universal/static and actual arm64 ABI checks pass. A read-only identity-key
mistake and initial collector's interleaved-test-output undercount are disclosed;
neither required a fit/test rerun. Candidate plugin is
`c64ead0763d662a8f7bd1c74f1058426ea03f90c871ff387e76a6fe935b2e3da`, with all
176 inputs verified. Installed dist, 97 unrelated STATUS additions and 358
original untracked paths remain intact. No agents, SCC, qualification or push.

This closes the bounded covariance experiment, not the remaining P3 cluster-state
reuse/duplicate residualization or broader P4/P5/P6/P7 and R1–R5 work. Next audit
final-sample cluster ordering consumers and low-memory score reconstruction, then
the remaining construction/state and centered-tile work. Complete #27 before the
authorized two-agent investigation, upstream CMG review and later small SCC run.

## P4/P6/P7 wider retained moments: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/ppml-wide-moments-local-2026-09-13/README.md)
adopts fixed two-tile standardization above 64 active columns against accepted
numerical `9ec47b8` (evidence-only `374d257` does not change that source). Each
existing Gram-row partition uses at most 64 weighted left and 64 standardized
right values: 1024 stack payload bytes, no heap, retained state or N-by-K copy.
Every output keeps its exact observation/FMA order, selected mapping, weights
and reference scales. The up-to-64 cache, scratch/unscaled/single-column paths,
one-shot OLS, QR, rank and strict current-frame policies remain unchanged.
[ADR 0057](docs/adr/0057-wider-retained-dense-moment-tiles.md) records the boundary.

The complete original protocol now passes on repaired artifacts: 17 new pairs,
including original PW K257 warm-up/full requests, nine primary pairs, seven
controls and a separate private audit. Earlier incomplete probe pairs are not
pooled. Unit K65/FW K129/PW K257 command medians are 2.308→1.678 /
4.479→3.213 / 16.106→15.171 seconds; median paired gains are 29.2%/28.0%/5.4%.
Finite-solve medians fall 0.730→0.310 / 1.247→0.405 / 1.282→0.756 seconds,
with lower strict/final work. Every primary pair improves. K257 inference and
publication dominate its remaining time; their costs are retained in totals.

Every scientific CSV is byte-identical; samples, rank/status/DoF, cluster and
separation tables, solver choices and certificates agree. Native peaks/models
are exact in all 17 pairs and admitted; primary deltas remain 364.49/287.97/
253.09 MB. PW sampled RSS rises 0.334→0.381 GiB; no memory saving is claimed.
Largest absolute control increase is unchanged complete K50 +89 ms/1.2%; low
FW adds 30 ms/3.3%. No control crosses the review envelope. The private audit
falls 21.923→19.398 seconds and retains zero events/live/peak growth over 16
iterations, stable capacities and fresh/warm equivalence. Retry/fallback counts
are zero throughout. Public and private timing aggregates remain separate.

Sixty-two distinct Rust checks and both independent dummy-Poisson/Hessian/
sandwich oracle arms pass. One matching integration test was initially outside
`--lib` filters and explicitly completed after timing; no failing numerical test
or fit was rerun. Isolated universal/static and actual arm64 ABI checks pass.
Candidate plugin is `d1e8efc290851ace10f917ac3cb630415bfaa975a2da588dead4a85b87fa2abc`,
with all 176 source/ado/fixture inputs verified. Installed dist and the 97
unrelated STATUS additions/358 original untracked paths are preserved. No agents,
cluster, qualification or push occurred.

This closes the bounded wider-moment evaluation. P6's width exercise combines
these 63/64/65/128/129/257 finite-column tests with the existing weighted-Schur
15/16/64/65 actual-RHS controls; rank policy is unchanged. Broader P4/P6/P7,
P5 and R1–R5 remain open. Next inspect the now-dominant K257 covariance/
eigensystem work, then remaining construction/predictor/state and centered-tile
opportunities. Complete #27 before the authorized two-agent investigation,
upstream CMG review and later small SCC campaign.

## P4/P6 published covariance inspection: adopted local repair

The [source-bound report](benchmarks/reports/development/ppml-psd-validation-local-2026-09-13/README.md)
repairs final PPML PSD-report validation against accepted `fd6b970`. The validator
inspects the published covariance's own eigensystem, retaining the existing
bound on further repair and all report/rank/tolerance checks. It no longer
substitutes diagnostics from a newly re-clipped comparison matrix. Production
CGM, inference and all numerical session/moment code remain unchanged;
[ADR 0056](docs/adr/0056-published-ppml-covariance-validation.md) records the boundary.

The original PW K257/N7680 warm-up fails r(430) on baseline and passes candidate.
A closed-form 65-dimensional Householder spectrum independently reproduces the
invalid rejection; forged reports and indefinite matrices still fail. A NumPy
profiled-Poisson/full-CGM oracle matches the original sample, identified and
reported ranks, nested FE and cluster structure, with maximum b/V errors
3.97e-15 / 2.87e-16. The N30720 full request has exact scientific exports in both
arms. Its common +576-byte modeled-phase offset is recorded separately as a
pre-validator dynamic embedding input discrepancy; its allocation source was
not traced. Native peak is unchanged/admitted. The initial oracle's incorrect
identified-versus-reported rank assertion is preserved and corrected to the
existing normalized-constant contract, not a new rank policy.

Forty-seven core tests, 33 PPML plugin tests and three independent licensed
fixtures on both arms pass, with explicit r(0)/sentinels. Isolated universal
build/static audits and actual arm64 ABI checks pass. Eleven paired cost
comparisons preserve exact b/V, discrete science, native peaks and modeled
budgets, all admitted. FW K129/exposure/missing saves 337/340/345 ms across
three seeds: median command 4.740→4.400 seconds (7.2% paired), publication
0.68449→0.34734. Inference remains unchanged. The largest absolute control
increase is complete K50 +230 ms/3.1%, in unchanged finite/strict work;
saved-output +11.5% is +3 ms. No control crosses the review envelope. K129 and
AW OLS sampled RSS rise; no memory saving is claimed. All PPML retry/fallback
counts stay zero. Cold functional calls are separate from timing evidence.

Candidate plugin is `411ed113101399adda2c893d0b3e51fa16cbc96e230871ec568e8b346a689126`;
176 source/ado/fixture inputs are bound by the report. An excluded initial
wrong-Cargo-directory diagnostic build, baseline failures and both harness
corrections remain disclosed. Installed/dist and unrelated Veneto evidence are
preserved. No agents, cluster, qualification or push occurred.

A [supplemental control correction](benchmarks/reports/development/ppml-psd-validation-local-2026-09-13/control-correction/comparisons.json)
clarifies that `weak_d3` means degree three with two FEs, not three FEs. The
original pair and all 11-pair archives remain unchanged. An added explicit
low-mobility three-FE pair passes 0.083→0.082 seconds with exact science and
admitted peak (18.453→18.452 MB, not exactly equal). A first missing-g3 warm-up
setup failure and an overstrict collection peak-equality assertion are preserved.
There are now twelve successful cost pairs, with the extra control separate.

Next resume the original 17-pair wider-moment protocol against this repair,
including the unchanged PW K257 warm-up and full request. The wider prototype
remains absent/unadopted; broader #27 P4/P6/P7 and other open items remain open.
Complete #27 before the authorized two-agent investigation, upstream CMG review
and later small SCC campaign.

## P4/P6 bounded request transport: adopted local repair

The [source-bound report](benchmarks/reports/development/request-transport-local-2026-09-13/README.md)
adopts [ADR 0055](docs/adr/0055-bounded-local-request-transport.md) against accepted
`22b5106`. Above 4,000 metadata bytes, OLS/PPML retain the unchanged framed
request in one fixed caller-local macro and pass a bounded byte descriptor to
the same native call. Exact length/NUL/UTF-8/framing and existing handshakes
precede observation reads. A 16 MiB metadata cap, extra truncation-detection
byte and separate SPI terminator capacity keep the read bounded. The buffer
is released after parsing; all numerical core inputs remain unchanged.

The final isolated plugin `7b7c6664...` passes 94 plugin tests, 26 static tests,
universal static/native arm64 ABI checks and the ordinary independent OLS/PPML
oracles. A new long-name fixture passes OLS AW robust and PPML FW robust/PW
clustered independent calculations, malformed/over-cap reads and rollback.
The old binary's r(1003) is expected transport-failure evidence, not a science
pass. Final review found and fixed the initial candidate's SPI terminator bound
and truncating-reader test model; its failing reproducer and eight initial
pairs remain archived separately. A fixture registration error and Python
launcher failures are preserved; corrected runs pass without weaker checks.

Eight final pairs preserve exact b/V and discrete science/solver fields. The
largest slowdown is complete PPML 7.354→7.500 s (+2.0%); none crosses the
prospective max(10 ms, 5%) review envelope. Native peaks are exact on seven
pairs and 4,302 bytes lower on FW K129; all are admitted. Sampled FW/weak RSS
rises, so no general memory saving or command-speed gain is claimed.

The exact original PW K257 full fit now succeeds at N30720/seed104729. Its
N7680/seed104730 warm-up reaches native code but fails the existing covariance
PSD-report consistency check with r(430), on both transport candidates. This
is a newly exposed separate numerical validation issue; it is not silently
accepted, bypassed or replaced with a smaller passing target. Next localize
and repair that check independently, then resume the original wider-moment
protocol. The wider optimization remains absent/unadopted and broader #27
items remain open. Installed dist, all 97 unrelated Veneto additions and prior
untracked evidence remain intact; no agents, cluster, push or qualification.

## P4/P6 wider moment probe: request transport needs repair first

The [source-bound probe](benchmarks/reports/development/ppml-wide-moments-probe-local-2026-09-13/README.md)
archives an unadopted extension against accepted `22b5106`. Two fixed 64-value
tiles preserve exact Gram/RHS row/FMA order above 64 active columns with no new
heap or observation-sized state. Sixty-two isolated tests pass, including exact
scalar moments at 128/129/257 columns and 1/4/16 workers; both narrow independent
Poisson/sandwich oracle arms and universal static/native arm64 ABI checks pass.

First-screen unit K65 and FW K129 commands fall 2.269→1.636 and 4.726→3.458
seconds, with exact b/V and admitted unchanged native peaks. These single pairs
are promising but insufficient for adoption. The PW K257 target fails on the
unchanged baseline's warm-up with r(1003), before measurement. No repeats,
protected performance controls, private audit or candidate K257 public fit ran.

Noisy replay/trace localize the failure to framed request transport at the
plugin invocation. A separate 270-variable short call reaches native code;
1,024/4,096-byte dummy options reach native validation, while 8,192+ bytes fail
inside Stata with `options too long`. The exact universal limit is not claimed.
All failures and diagnostic log hashes are preserved, including an initial
invalid `_dup` probe. The numerical prototype is archived and reversed; all
152 accepted native hashes are restored. Installed dist and Veneto work remain.

Next repair bounded metadata/request transport independently, preserving one
substantive native call, main-thread SPI, canonical framing and publication
atomicity, then resume the full wider-tile protocol against that repaired
baseline. Do not replace the unavailable PW target with a smaller passing case.
Other #27 work remains open; agents, CMG assessment and SCC remain after #27.

## P4/P7 retained dense moments: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/ppml-moments-local-2026-09-13/README.md)
adopts [ADR 0054](docs/adr/0054-retained-dense-moment-row-standardization.md)
against accepted native `6ad8b20`. Scaled retained dense moments at up to 64
active columns standardize each observation's selected suffix once per existing
Gram-row partition. The fixed local cache preserves exact row/FMA order and
uses no heap allocation, retained partial bank or observation-sized state.
Wider, scratch, unscaled and single-column partitions retain their previous
loops. Rank, scales, certificates, generations and one-shot OLS policy remain.

Sixteen pairs / 32 commands include nine primaries across three seeds, six
controls and one separate private audit. Complete/FW/PW command medians fall
9.540→7.440 / 3.180→2.446 / 2.944→2.253 seconds, with median paired gains
22.0%/24.0%/24.4%. Every primary improves. Finite and strict solving fall while
projection/preparation stay close. All native peaks are exact and admitted;
sampled RSS moves both ways, including higher PW RSS. The largest protected
increase is K=65 +10 ms/1.4%. The private audit falls 7.817→5.429 seconds and
retains zero events/live/peak growth, stable capacities and fresh/warm equivalence.

Full b/V and discrete science/solver fields are exact. Sixty-two isolated Rust
tests cover exact scalar moments, 1/4/16 workers, 1/7/16/63/64/65 columns, extreme
scales, selected storage, nonfinite rejection, retained QR, strict rollback and
stale generations. Both independent dummy Poisson/sandwich oracle arms and
universal static/native arm64 ABI checks pass. All 152 native inputs match the
measured isolated artifact. A rejected scratch-fixture NaN, incomplete diagnostic
sampler and duplicate-VCE wide-control warm-up failure are preserved and explained.
Only the malformed wide control was rerun; its corrected pair passes.

Next evaluate bounded standardization reuse beyond 64 columns and in scratch
consumers, then remaining P4 construction/predictor/state work and other #27
packages. Broad P4/P7/P6 items remain open. Complete #27 before the authorized
two independent agents, upstream CMG assessment and small SCC campaign. Installed
dist, unrelated Veneto work and historical receipts remain intact; no agents,
SCC, push or qualification ran.

## P4 fresh raw reference norms: evaluated and rejected

The [source-bound report](benchmarks/reports/development/ppml-raw-norms-local-2026-09-13/README.md)
evaluates raw norm reuse against accepted `6ad8b20`. A frame compatibility guard
allows fresh one-FE construction to omit its duplicate raw norm scan and copy
original-input norms from the same successful projection. Raw means, reference
scaling, initial/final certificates and rank policy remain. The prototype is
archived and reversed; all 152 accepted native input hashes are restored.

Fourteen pairs / 28 commands include nine primaries over three seeds and five
controls. Complete/FW/PW median paired command changes are −0.9%/−1.8%/+1.4%:
none reaches the prospective 2% gain. Projection falls in all nine primary pairs,
by 16–129 ms, but unchanged finite/strict timing variation prevents adoption.
No extra seeds or private allocation audit were run after the repeated screen
failed. All native peaks are exact and admitted; RSS moves both ways. Largest
protected increase is weak two FE +6 ms, below the review envelope.

Full b/V and discrete scientific/solver fields are exact; independent certificates
pass. Thirty isolated Rust tests and both independent dummy Poisson/sandwich
oracle arms pass, including original-input hypot norms, changing/extreme weights,
warm-zero returns and output nonmutation. Universal static/native arm64 ABI checks
pass. No test failure or tolerance change occurred. Installed dist and unrelated
Veneto work remain intact; no agents, SCC, push or qualification ran.

Accepted native remains `6ad8b20`, including selected within-norm reuse. Next
localize the larger remaining finite/strict matrix, moment and predictor costs;
raw-reference reuse is rejected as a standalone step, with possible combined
layout work left as a hypothesis. Other #27 packages and R1–R5 remain open.
Complete #27 before the authorized agents, CMG assessment and small SCC campaign.

## P4 fresh-design within norms: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/ppml-fresh-norms-local-2026-09-13/README.md)
adopts [ADR 0053](docs/adr/0053-ppml-fresh-design-norm-reuse.md) against accepted
native `ccc1f33`. Fresh fast one-FE PPML construction selects the same successful
projection's control norms through existing generated-column metadata. It omits
per-value norm arithmetic during compaction and two K-sized scratch allocations.
Raw reference norms/means, initial certificates, final checks and rank policy
remain; multi-FE, low/generic/slope and alternative divisors keep their calculation.

Fifteen pairs / 30 commands include nine primaries over three seeds, five controls
and one separate private audit. FW/PW median paired command gains are 2.4%/5.1%,
with projection medians 0.4704→0.4444 and 0.3883→0.3437 seconds. The complete
unweighted target has no repeatable command gain (+0.02% paired), despite lower
projection cost. Unchanged finite/strict/preparation variation is preserved and
not wholly attributed to norm reuse. The largest protected increase is +1 ms;
no public pair exceeds the prospective regression envelope. The private audit
adds 65 ms/0.7%, retaining zero events/live/peak growth and stable capacities.
Native peaks are exact and admitted on all pairs; RSS moves both ways.

Full b/V and discrete scientific/solver fields are exact; independent certificates
pass. Twenty-nine isolated Rust tests and both independent dummy Poisson/sandwich
oracle arms pass, as do universal static/native arm64 ABI checks. The new test's
overstrict cross-projector diagnostic comparison fails identically on accepted
baseline; only that new comparison was corrected, preserving its fixture,
scientific envelope, global helper and runtime contracts. All 152 native inputs
match the measured isolated artifact. Installed dist and unrelated Veneto work
remain intact; no agents, SCC, push or qualification ran.

Next inspect the duplicate fresh raw-reference norm pass and remaining matrix/
moment/predictor work. Other P4 items, P0 accounting, P3/P5/P6/P7 and R1–R5 remain
open. Complete #27 before the authorized independent agents, upstream CMG review
and small SCC campaign.

## P4 fresh-input certificate removal: evaluated and rejected

The [source-bound report](benchmarks/reports/development/ppml-fresh-input-local-2026-09-13/README.md)
evaluates a standalone fresh one-FE entry against accepted `ccc1f33`. Fresh raw
responses/design blocks skip the initial certificate, project once and independently
certify against original reference norms. The first sweep consumes the unchanged
iteration budget; retained warm residuals keep iteration-zero checks. The prototype
is rejected and its exact source/patch/artifacts remain archived. All 152 restored
native hashes match accepted `ccc1f33`; no prototype remains in active source.

Sixteen pairs / 32 commands include nine primaries, five controls and two low-FW
rechecks. Complete/FW/PW command medians are 9.683→9.751 / 3.223→3.209 /
2.979→2.821 seconds, with median paired changes +0.7%/−0.8%/−4.7%.
The only command target exceeding the prospective gain threshold has a 126 ms
median reduction in unchanged finite solving versus about 4 ms in projection.
A material complete-command gain attributable to the changed work is unproven.
The low-FW control first costs 72 ms; rechecks save 96 ms and cost 29 ms.
Every observation remains, without treating changing direction as a root cause.

Full paired b/V and discrete scientific/solver decisions are exact. Native peaks
are exact and admitted, with no memory gain. Twenty-eight isolated candidate
tests and both independent full-dummy Poisson/sandwich oracles pass, including
parallel weighted group means, fresh/prior equality, warm-zero and iteration
budgets, current-frame norms, stale-rank and transaction/generation checks.
Universal static/native arm64 ABI checks pass. No private allocation audit ran
after rejection; no new allocation-free claim is made.

This completes the bounded P4 fresh-input/current-frame norm evaluation: same-call
fused norm reuse is adopted and standalone fresh-certificate removal is rejected.
Next evaluate fresh-design norm construction and remaining matrix/moment/predictor
passes, then the remaining #27 packages. The two independent agents, upstream CMG
assessment and small SCC campaign remain after #27. Installed dist, all 97 unrelated
Veneto lines and prior evidence remain intact; no push, agents, cluster or qualification.

## P4 same-call projection norms: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/ppml-norm-reuse-local-2026-09-13/README.md)
adopts [ADR 0052](docs/adr/0052-ppml-same-call-projection-norms.md) against repaired
`6b7d0fe`. Successful fused one-FE PPML projection copies its certified trailing
control norms into retained storage when frame/MAP weight divisors match.
Multi-FE handoffs and alternative divisors retain the stable current-frame scan.
No last-result accessor, norm-vector transfer or observation-sized allocation is
introduced. Raw reference scales, warm certificates and strict rank policy remain.

Fifteen pairs / 30 commands include nine primaries over three seeds, five controls
and one separate private audit. Complete/FW/PW command medians fall
10.317→9.689 / 3.380→3.231 / 3.035→2.933 seconds; median paired gains are
8.5%/4.5%/4.5%. All primary pairs improve. The large first-screen preparation
difference is preserved and not attributed to norm reuse. Projection/strict
phases fall; native peaks are exact and admitted throughout. Sampled RSS moves
both ways, including higher explicit-MAP RSS; no memory saving is claimed.
The largest protected command increase is explicit MAP +5 ms. The private audit
falls 8.661→8.027 seconds and retains zero allocation events and live/peak growth.

Full paired b/V and discrete scientific/solver fields are exact; independent
certificates pass. Twenty-three isolated candidate tests include changed-weight,
iteration-zero, capacity and failure checks; the stale-rank oracle now exercises
both matching and alternative divisors. Both independent dummy Poisson/sandwich
oracle arms and universal static/native arm64 ABI checks pass. All 152 native
input hashes match the measured isolated plugin a4d4a33a... . Initial scale-field
compilation error and every measurement are disclosed; no tolerances changed.

The subsequent fresh-certificate evaluation is rejected above. Fresh-design norm
construction, remaining P4 passes and other #27 packages remain open. The two independent agents, upstream CMG assessment and small SCC
campaign remain after #27. Installed dist, all 97 unrelated Veneto lines and
historical receipts remain intact; no push, agents, cluster or qualification.

## P4 current-weight within norms: correctness repair

The [source-bound repair and cost report](benchmarks/reports/development/ppml-current-norms-local-2026-09-13/README.md)
fixes stale within-column norms in fused PPML response/design projection against
`c6ad4c4`. The raw reference norms and projected values refreshed, but strict
rank/score checks could consume within norms from older weights. A two-row-group
closed-form oracle gives a current within/raw squared ratio about 5.34e-15 versus
2.67e-7 with the stale numerator, across the unchanged 1e-9 FE-collinearity cutoff.
The old retained strict solve accepts rank one while a fresh solve gives rank zero.
The finalized test requires the existing strict prior-basis rejection, unchanged
basis generation and no current output; explicit basis refresh permits rank zero.

The repair uses the existing stable current-frame norm scan after fused matrix
compaction, with retained scratch and no observation-sized allocation. This
covers fused one/two/three-FE categorical checkpoint consumers. Raw scales, warm
projection, finite/QR policies, generations and strict transaction rules remain.
This is a correctness repair, not a performance gain or a public-estimator
counterexample claim.

Eleven pairs / 22 commands preserve exact full b/V and discrete science; all
independent certificates pass. The first public screen costs +4.8% for complete
one FE, +5.0% for FW/exposure and +4.6% for PW/offset/clustering. Two additional
FW seeds confirm a +5.3% median paired cost across three pairs. Native peaks are
exact and admitted throughout; sampled RSS moves both ways. Protected increases
are weak +4 ms, low FW +12 ms (1.5%) and explicit MAP +2 ms. The separate private
audit costs +12.4% but retains zero allocation events and zero live/peak growth.

Twenty-one distinct tests pass from a candidate-only Cargo target, including
strict rollback, identical-frame reuse, generation invalidation and the new
reproducer. Both artifacts pass the independent full-dummy Poisson/Hessian/
sandwich oracle. A shared Cargo target reused the baseline test executable after
switching copied source trees; those test results are excluded. A forced release
recompilation produces the exact measured candidate bytes (313e85aa...), with
universal static and native arm64 ABI checks passing again. The workflow now
records source-snapshot target isolation. Other scaffold/test-expectation
failures and all timing observations are preserved in the report.

The subsequent same-call norm reuse is adopted above. Fresh-input certificate
removal, further P4 passes and the remaining #27 work are open. The two independent agents,
upstream CMG assessment and small SCC campaign remain after #27. Installed
dist, all 97 unrelated Veneto lines and prior evidence remain intact; no push,
agents, cluster work or qualification occurred.

## P2 low-memory fresh blocks: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/low-one-fe-local-2026-09-13/README.md)
adopts [ADR 0051](docs/adr/0051-low-memory-fresh-block-projection.md) against
accepted native `4e18c22`. Low-memory raw outcome/regressor blocks use the
existing fresh absorption entry. The projected outcome consumes its one-column
storage; certified within norms are reused only with matching frame/projector
weight normalization. Explicit alternative divisors retain their original norm
calculation. Raw reference scales, QR, warm residuals, retained solvers, saved
products, scratch layout and admission remain intact.

Seventeen pairs / 34 measured commands include nine primary comparisons across
three seeds, seven controls and one separate trace. Narrow AW command medians
fall 0.369→0.336 seconds (8.9% median paired gain) and native peak 95.62→87.75 MB,
exactly one 7,864,320-byte outcome column (8.2%). Unit and PW-cluster command
gains are 4.6% and 2.6%; all primary pairs improve. Candidate primary peaks fit
their unchanged models; the AW baseline exceeded its model. Existing low-FW
interaction (8.77 vs 6.70 MB) and weak (22.87 vs 9.41 MB) gaps remain P0 work.
Sampled primary RSS falls, but saved/weak controls rise; no universal RSS claim.

Full paired b/V, sample/status/DoF and solver decisions are exact. Independent
certificates and saved values pass. Thirty-two distinct Rust tests, both arms
of the explicit-dummy/frequency-expansion Stata oracle, universal static audits
and the native arm64 ABI/self-test pass. No protected command slows in the
selected single pairs. The trace locates the gain in native fitting; input,
preparation and IID covariance work remain close. The report preserves the
initial missing-PATH Rust launch and all measured observations.

This completes the remaining P2 fresh-block evaluation. Next inspect PPML
fresh-input/current-frame norm reuse and remaining repeated passes, then the
remaining #27 inference, centered-tile, output/generated-column, scheduling and
research items. The two independent agents, upstream CMG assessment and small
SCC campaign remain after #27. Installed dist, all 97 unrelated Veneto STATUS
lines and pre-existing evidence are preserved; no push or cluster work occurred.

## P2 constant-weight specialization: evaluated and rejected

The [source-bound screen](benchmarks/reports/development/constant-weights-local-2026-09-13/README.md)
compares an isolated one-file prototype against accepted `4e18c22`. It detected
constant weights during existing validation, omitted normalized weight arithmetic,
and preserved original slices/scales for solver handoffs and raw certificates.
No target met the prospectively required 5% and 10 ms command gain. Four target
pairs changed −2 ms/−0.8% (narrow AW), −13 ms/−2.1% (wide AW), +5 ms/+1.2%
(FW cluster) and +3 ms/+1.0% (weighted Schur). The wide native phase saved only
1.7 ms; most apparent command savings were outside the modified kernel.

All six pairs / twelve commands preserve exact full science and solver decisions.
Two focused tests, universal static/native arm64 ABI checks and all runtime
certificates pass. Primary native peaks are unchanged and admitted; sampled RSS
moves in both directions, and the weak control retains its old model discrepancy.
No extra timing seeds or independent full-estimation oracle campaign ran because
the prospective screen rejected adoption. The source, patch, plugin identities
and all observations are preserved. After verifying the archived 152-file source
manifest and exact patch, only the prototype was reversed; native source remains
accepted `4e18c22`. Constant specialization is evaluated-and-rejected, not claimed
impossible in other workloads or platforms.

The subsequent low-memory fresh-block adoption is recorded above; other #27 work remains.
The authorized two independent agents, upstream CMG assessment and small SCC
campaign remain after #27. Installed dist, all 97 unrelated Veneto STATUS lines
and pre-existing evidence remain intact; no push, agents, cluster jobs or
qualification occurred.

## P2 categorical one-FE initial projection: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/categorical-one-fe-local-2026-09-13/README.md)
adopts [ADR 0050](docs/adr/0050-categorical-one-fe-initial-projection.md) against
accepted native `7663a87`. Fast automatic OLS with one intercept-only categorical
effect and no saved-FE retention now reuses the existing simple one-FE initial
projector and OLS norm scheduling. It omits only the redundant raw certificate;
final independent certification and original reference scales remain. Warm
residuals, explicit solvers, slopes, multi-FE, low memory and PPML keep their
existing paths. No observation-sized allocation or alternate projector is added.

Seventeen pairs / 34 measured commands include nine primary pairs, seven controls
and one separate trace. Command medians fall 0.846→0.693 seconds for unit IID,
0.310→0.274 for AW robust and 0.521→0.478 for PW two-way clustering: median
paired gains 18.0%, 11.3% and 8.6%. The trace locates savings in initial absorption
(0.06000→0.02004). Primary native peaks are effectively unchanged and admitted;
weighted sampled RSS rises 0.392→0.502 / 0.394→0.491 GiB without an established
cause. Largest protected command increase is slope +2 ms. Existing low-memory
and weak model underreports remain open; no resident-memory gain is claimed.

Full paired b/V, samples, status/DoF and solver decisions are exact. Four focused
Rust tests and both arms of the independent interaction-dummy Stata oracle pass,
including all OLS weight kinds, 1/7 workers, frequency expansion, rescaling,
collinearity, raw-scale projection and warm/retained paths. All 152 native input
hashes match the isolated measured plugin `ea63bae2...`; universal static/native
arm64 ABI checks pass. Initial scaffold/toolchain and oracle-code assertion
failures are preserved and explained in validation, without weakening science.

The expanded P0 reporting prototype was verified against its archived 152-file
manifest and exact thirteen-file patch, then removed from active native source.
It remains archived and unadopted; this is not closure of its accounting gaps.
Next evaluate constant-weight specialization and fresh low-memory one-FE blocks,
then remaining #27 packages. Only after #27: the authorized two independent
agents, upstream CMG assessment and new small SCC campaign. Installed dist, all
97 unrelated Veneto STATUS lines and pre-existing evidence remain intact.
No push, new cluster work or qualification occurred.

## P0/R1/P7 bounded LSMR batches: adopted local checkpoint

The [adoption report](benchmarks/reports/development/lsmr-batch-adoption-local-2026-09-13/README.md)
supersedes the held-prototype decision at `cd298d5` without altering its immutable
72-pair investigation. The private adapter windows independent RHSs only above
64 columns and 4× the installed team, sharing one weighted solver. Narrow batches
retain upstream transport. Shared categorical OLS/PPML memory estimates now charge
recurrence, dense construction, scatter, output overlap and Schwarz stack/pool
state; see [ADR 0049](docs/adr/0049-bounded-lsmr-batches-and-memory.md).

Repeated wide weak OLS native peak falls 403.31→351.32 MB (12.9%) with +1.5%
median paired command cost; wide weak PPML falls 182.18→169.43 MB (7.0%) with
+0.5% command cost. Adoption rests on memory savings, not speed. A prospective
ordinary-command follow-up adds 12 pairs / 24 calls: six identical-binary A/A
controls and six A/B comparisons, interleaved over three seeds in both orders.
A/B differences are −6/−8/−17/+3/−16/−23 ms, all within the original review
envelope; A/A ranges −19 to +18 ms. Earlier traced PPML increases of 21/36/38 ms
remain unexplained and preserved. No causal narrow speedup is claimed.

Full science and non-time solver/retry decisions are exact. The preceding 67
LSMR-filter and 77 memory-filter tests (134 distinct), three independent licensed
fixtures in both arms and nine source-bound allocation probes remain applicable.
The reviewed source differs only by a corrected test comment; its universal plugin
is byte-identical to the measured `c9db0dce...` candidate and static/native arm64 ABI
checks pass. The report binds all 152 final native inputs and seven changed files.
A discriminating 327680-byte admission test rejects the old over-budget LSMR
construction and selects certified CG. Three unconverged allocation probes remain
allocation evidence only. The ill-conditioned smooth 65-column oracle and earlier
low-FW/saved-AW weak certificate failures remain failures, not passing controls.

Next repair actual attempted/fallback peak reporting: public OLS selected-model
bytes still omit solver increments. Opaque factor/stack bounds and full construction
counters remain open; this is not universal memory-bound qualification. Continue
remaining #27 packages before the authorized two-agent investigations, upstream
CMG assessment and new small SCC campaign. Installed dist, the 97 unrelated Veneto
STATUS additions and all pre-existing local evidence remain intact. No SCC,
qualification or push occurred.

## P0 attempted-memory reporting: expanded prototype, not adopted

The [expanded source-bound report](benchmarks/reports/development/solver-memory-reporting-expanded-local-2026-09-13/README.md)
records the archived thirteen-file native prototype against adopted `7663a87`,
superseding the initial eight-file prototype's coverage statement without changing
that historical receipt. Its exact patch was later removed from active source
before the P2 categorical one-FE experiment above. CMG construction/rejection/error estimates now survive into
fit reports, including retained hierarchy/coefficient overlap with CG polishing.
Transient/retained borrowed LSMR records estimates without a supplied memory cap
and uses the installed team; generic intercept-only LSMR preserves failed attempts.
Fast/low inference score reconstruction now contributes its separate solver peak.
The public selected-memory scalar and observed/model ratio remain unchanged.

All 152 native inputs are bound to an isolated universal candidate plugin,
`8b6f3ffaa711581367de389c9341d47ef57d2810c76188048852b10083b5b4d5`.
Static/native arm64 ABI checks, plugin type-check, 128 distinct focused Rust
tests and six licensed oracle invocations pass. The CMG budget test rejects
one byte below its modeled retained-state-plus-polish overlap and accepts the
exact amount, preserving residuals/coefficients and final-report persistence.
The independent Stata dummy oracles cover weighted inference, CMG/LSMR,
1/7 workers, saved effects and low pools 1/5/10.

The fifteen original comparison pairs retain exact full science/non-time solver
fields. CMG low/wide command medians are 0.128→0.128 / 0.214→0.210 seconds;
weak PPML is 0.412→0.413. Wide weak OLS is 1.006→1.054 (+4.8% median paired),
with +49/+9/+51 ms individual changes and the last crossing max(10 ms, 5%).
Six prescribed follow-up pairs keep identical-binary A/A separate from A/B:
A/A differences −9/+20/+21 ms; A/B −4/+13/+52 ms. The +52 ms loss again crosses
the envelope. These are 21 pairs / 42 calls total, with no observation excluded.
A/A variation does not explain the recurring losses. Do not adopt or count this
reporting prototype as a gain; its missing coverage and timing cost remain open.

Next complete slope/signed-cover and generic CG/MAP/owned-design accounting,
saved-effect recovery attempt/overlap/admission, retained coefficient overlaps,
and construction/discarded-work counters. Localize the weak wide cost before
publishing a complete action model. Public underreports remain visible, including
54.93 MB observed versus 1.045 MB modeled on small weak OLS. Separate Veneto
changes, historical receipts and installed dist remain intact. No push, new
cluster work, broad qualification or independent-agent phase occurred.


## P0/R1 borrowed LSMR transport: adopted local checkpoint

The borrowed categorical LSMR adapter now delays its overwritten candidate
allocation until the upstream solve ends, releases RHS transport before output
materialization, and releases demeaned/unrequested coefficient banks before
independent certification. The original input and requested recovery coefficients
retain their scientific lifetimes; see
[ADR 0048](docs/adr/0048-borrowed-lsmr-transport-lifetimes.md).

The [report](benchmarks/reports/development/lsmr-transport-local-2026-09-12/README.md)
binds seventeen successful pairs / 34 measured commands against `15bd21d`.
Three-seed automatic weak-graph native peaks fall 30.37→27.31 MB at N30720/K10
and 469.17→403.31 MB at N122880/K65 (10.1%/14.0%). Command medians are
0.060→0.060 and 0.997→1.013 seconds; the wide median paired overhead is 0.9%.
This is a measured memory improvement, not a speed claim. Full successful
science and non-time solver decisions remain exact. Generic explicit fast
LSMR, strong Schur and complete IID control peaks remain unchanged. The PPML
checkpoint control adds 10 ms/3.9%, with exact science and retry diagnostics.

Sixty-two focused Rust tests and both arms of two explicit-dummy Stata oracles
pass, including retained batches, saved coefficients, raw-scale certificates
and constrained routing. Two other weak requests fail identical r(459) guards
on both artifacts: low FW cluster is uncertified; saved AW coefficient recovery
is not certified/aligned. All seven timed failed calls and the separate
untimed duplicate-solver warm-up failure are preserved. Those failures remain
unresolved and do not count as passing oracles.

The [P0 audit](benchmarks/reports/development/lsmr-transport-local-2026-09-12/memory-audit.md)
proves the OLS model's missing per-RHS/coefficient-space recurrence accounting
with six independent allocation tests and four pinned-within batch probes.
Reporting separately omits topology solver increments. Row permutation,
scatter, batch output overlap and Schwarz dense/setup/apply lifetimes also need
reconciliation; inspect the existing more detailed PPML model before sharing
it, including its unproven nested-concurrency bound. Public model excesses persist:
27.31 versus 13.39 MB and 403.31 versus 268.55 MB on these primary cases.
The small weak control exposes about 55 MB versus 1.045 MB. P0 is not closed.

Adopted native inputs match the measured universal plugin; static/native arm64
ABI checks pass. Installed dist, 97 unrelated Veneto STATUS additions and all
pre-existing untracked paths remain preserved. No SCC, qualification or push.
Next correct and validate joint memory accounting/reporting, then address the
remaining #27 work and preserved certificate failures. Finish #27 before the
authorized independent agents, upstream CMG assessment and new small SCC campaign.

## P3/P7 cluster-boundary setup: adopted local checkpoint

OLS ordered clustering and PPML paired-independent reductions now share a bounded
search for each sorted cluster's end. It preserves every chunk/boundary, empty
chunk, row/reduction order and allocation; see
[ADR 0047](docs/adr/0047-bounded-cluster-boundaries.md). It removes repeated scans
through giant clusters without splitting their numerical reductions.

The [report](benchmarks/reports/development/cluster-boundaries-local-2026-09-12/README.md)
binds twenty pairs / forty measured commands against `fdf4624`. Repeated skewed
OLS command medians fall 0.430→0.401 seconds (6.5% paired, 27–37 ms savings).
PPML inference falls 0.06969→0.03532, but total command medians only fall
2.914→2.893 (1.1% paired, variable 7–47 ms savings). Adoption rests on OLS's
repeatable complete-command gain. Primary native/scratch peaks are unchanged.

All science, samples, saved outputs and non-time solver decisions are exact.
The initial PPML two-way control slows 43 ms/12.5%, localized to unchanged solve
phases; three fresh rechecks improve 19/53/17 ms. The original result is retained,
and the source of timing variation is not established. Other controls stay within
the review envelope. Weak OLS retains a pre-existing memory-model discrepancy.
Twenty-five focused Rust tests and both strengthened 65,536-row Stata/Mata oracle
arms pass; the first OLS oracle was enlarged after review because it did not cross
a chunk boundary. Adopted native inputs match the measured plugin exactly;
universal static/native arm64 ABI audits pass. Installed dist, unrelated Veneto
work and historical evidence remain intact; no SCC, qualification or push ran.

Next address P0's observed/model memory discrepancies, then remaining P2/P3/P4
passes and P5 centered-tile work, carrying winners through P6/P7. R1–R5 remain
open. Finish #27 before the authorized independent agents, upstream CMG review
and new small SCC campaign.

## P3/P6 interaction coding: adopted local checkpoint

Categorical FE/cluster interactions now use one shared exact dense tuple mapping
when its full u32 table fits the displaced physical-row ordering bytes. Sparse
or overflowing products retain comparison sorting without partial allocation.
Original validation, lexical codes, sample/weight/singleton semantics and numerical
operations remain; see [ADR 0046](docs/adr/0046-bounded-interaction-coding.md).

The [report](benchmarks/reports/development/interaction-codes-local-2026-09-12/README.md)
binds 27 pairs / 54 measured commands against `9c2bcc5`: nine primary, fifteen
protected, one pilot and two public profiles. Repeated OLS command medians fall
0.439→0.393 seconds for interacted clusters (10.5% paired), and 0.370→0.345 for
interacted FE/clusters (6.8%, with 24–26 ms savings). Separate traces locate those
gains in preparation. PPML falls 0.383→0.370 (3.4%), but its recorded preparation
time barely changes; that smaller gain is not wholly attributed to this timer.

Full science, samples, saved values and non-time solver diagnostics are exact.
Primary native peaks are unchanged within their models; the largest protected
command increase is unchanged IID +4 ms/0.7%. Graph controls retain pre-existing
memory-accounting gaps, including weak OLS 30.6→30.9 MB against a 13.6 MB model;
this does not close P0. Forty-one distinct focused Rust tests and both independent
Stata/Mata OLS/Poisson oracle arms pass. The final comment-reviewed universal
plugin is byte-identical to the measured candidate; static/native arm64 ABI checks
pass. The first comment-layout rebuild mismatch is retained in validation.

Installed dist, unrelated Veneto changes/evidence and historical receipts remain
intact. No SCC, qualification or push occurred. Next evaluate the duplicate
cluster-boundary scans under P3/P7, then remaining score reconstruction and the
P0/P2/P4–P7/R1–R5 work. Finish #27 before the authorized two-agent investigation,
upstream CMG review and new small SCC campaign.

## P3 shared subset score passes: adopted local checkpoint

Scratch-backed OLS covariance can now feed all subset accumulators from one
original-row traversal when the entire compensated bank, range metadata and
prefix scratch fit one displaced physical-row ordering budget. Planning is
all-or-none; larger products retain the accepted per-subset route. Group/merge
order, independent per-group bread, weights, PSD, rank and publication contracts
remain unchanged; see [ADR 0045](docs/adr/0045-batched-low-memory-cluster-scores.md).

The [report](benchmarks/reports/development/cluster-subset-batch-local-2026-09-12/README.md)
binds 26 pairs / 52 measured commands against `c37686b`, including original and
revised pilots, six original primary pairs, nine controls, six fresh many-way
pairs, a separate profile and two memory rechecks. The original two-/three-way
10% target was missed (0.7%/5.9% paired gains). A separately specified follow-up
repeats five-way gains of 20.4% at N30720/K4 and 17.4% at N122880/K10 with AW,
with command medians 0.054→0.043 and 0.441→0.365 seconds. Adoption rests on those
many-way results; ordinary two-way gain remains unconvincing.

All full scientific exports, saved values and non-time solver diagnostics are
exact. Primary native/scratch peaks are unchanged. Worst protected command
slowdown is one-way +5 ms / 2.0%. Weak controls still exceed their pre-existing
memory models. A two-way native-peak increase changes direction in rechecks;
the source of variation remains unestablished, and no model-bound pass or
universal RSS reduction is claimed. Bank bytes and fewer score traversals are
source-derived counts, not measured physical I/O.

Thirteen distinct focused Rust tests and both arms of an independent full-dummy/
Mata covariance oracle pass, covering unit/AW/FW/PW, full/deficient rank, all
31 five-way masks, exact admission bounds and fallible reservations. Initial
oracle failures (DoF rank, omitted PSD repair, eigenvector orientation) and the
slower initial prototype are preserved. The reviewed universal plugin is
byte-identical to the measured revised artifact; static/native arm64 ABI checks
pass. Installed dist, unrelated Veneto changes and historical receipts remain
intact. No SCC, qualification or push occurred.

Next: remaining P3 interaction construction/order reuse, skew scheduling and
score residualization; P0 memory/counter reconciliation and P2/P4–P7/R1–R5 remain
open. This completes the bounded low-memory subset-sharing experiment, not #27.
Finish #27 before the authorized independent agents, CMG review and new small
SCC campaign.

## P3 low-memory cluster scores: adopted local checkpoint

Low-memory OLS now reads eligible cluster score subsets sequentially, using
compensated group scores only when their complete table and presence flags fit
inside the displaced N-row ordering budget. Larger/unrepresentable Cartesian
products retain tuple sorting. Original within-group row order, lexicographic
group order, per-group bread and independent covariance arithmetic remain;
see [ADR 0044](docs/adr/0044-sequential-low-memory-cluster-scores.md).

The [report](benchmarks/reports/development/cluster-grouping-local-2026-09-12/README.md)
binds thirteen pairs against `2243b4d`: six primary comparisons, six controls and
one separate pilot. Complete-command paired gains are 16.4% for low FW one-way
clustering (0.307→0.259 s) and 48.2% for low PW two-way clustering (1.870→0.964 s).
Primary native/scratch peaks are unchanged; full scientific exports, saved
outputs and non-time solver diagnostics agree exactly. Every bounded control
improves, including weak, high-cardinality fallback, small, fast, saved and no-FE
cases. Single-pair control gains are not repeatability claims.

The weak control retains a substantial pre-existing observed/reported model
discrepancy: 29933828→29827588 native bytes versus 13460424 modeled. Primary and
other control bounds hold. PW sampled RSS rises 0.169→0.192 GiB; no general RSS
saving is claimed. A sample-hash-verified cache replay explains fewer repeated
tile requests, but is explicitly source-derived rather than measured disk I/O.

Nine distinct focused Rust tests and both arms of the full-dummy Stata b/V oracle
pass, covering unit/AW/FW/PW, full/deficient rank, precise table bounds, overflow
fallback and allocation failures. Universal static/native arm64 ABI audits pass;
the reviewed rebuild is byte-identical to the measured prototype. Installed dist
and unrelated Veneto work/evidence remain intact; no SCC or qualification ran.

A separate sequential-histogram prototype is not adopted: its ordering kernel
improves, but nine command pairs show under-2% noisy gains with changes in
unchanged input work. Both experiments and all attempts are archived. Existing
simple cluster codes and covariance/certificate order sharing were already in
place. Next P3 work is combining subset consumers within bounded score storage,
interaction construction/order reuse, skew and remaining score reconstruction.
P0 model/counter reconciliation, P2/P4–P7 and R1–R5 remain open. Continue #27
before the authorized two-agent investigation, CMG review and small SCC campaign.

## P3 inference ownership: adopted local checkpoint

Ordinary fast robust/clustered OLS now packs score values only up to 32 MiB,
then uses the existing retained strided route when the finite solve permits it.
This also selects existing stable raw moments and fused residual construction.
Shared weighted-linear/PPML policy, IID, low memory, rank, memory admission and
public contracts remain unchanged; see
[ADR 0043](docs/adr/0043-ols-inference-strided-cache-policy.md).

The [report](benchmarks/reports/development/ols-inference-ownership-local-2026-09-12/README.md)
binds 29 pairs / 58 measured calls against native baseline `f4cebe7`. The final
comment-reviewed source rebuild is byte-identical to the prototype plugin.
Three-seed command medians fall 1.312→0.803 seconds for robust missing,
0.368→0.253 for PW robust and 0.539→0.409 for FW clustering; paired gains are
38.8%, 31.4% and 24.1%. Primary native peaks fall about 25–26%, and every observed
action peak fits the unchanged model. These gains include reference/residual
passes; strided score certification costs more and covariance time stays similar.

Samples/status/DoF, FE/cluster tables and solver decisions agree exactly. Full
b/V differences are at most 3.34e-16 / 7.20e-21, with unchanged independent
certificates/tolerances. Newly eligible weak, no-FE, K65 and interacted two-way
cluster cases pass. No protected control exceeds max(10 ms, 5%); complete IID
is +12 ms / 2.1%, small/narrow +1 ms. QR profiles improve 4.0% while retaining
about 0.12 seconds of score reconstruction in both arms; that work is not removed.

[Validation](benchmarks/reports/development/ols-inference-ownership-local-2026-09-12/adoption/validation.json)
records eighteen distinct focused Rust tests and an explicit-dummy full-covariance
Stata oracle on baseline/candidate across unit/AW/FW/PW, robust/clustered and full/
deficient rank, with r(0) sentinels. Universal static/native arm64 ABI checks pass.
The initial pilot diagnostic-assertion failure remains disclosed in immutable
preliminary evidence. No full profile, SCC, Windows or qualification ran;
installed dist and unrelated Veneto work remain preserved.

P3's packed/strided lifetime comparison is adopted. Next audit actual repeated
cluster-state/order construction and score consumers, then evaluate useful tiling,
skew scheduling and low-memory duplicate work. P0/P2/P4–P7, R1–R5 and the later
agent/CMG/new-SCC phases of the active goal remain open.

## P1 candidate-run and mixed-source loading: adopted local checkpoint

Against `5411b9b4d37aab1a856b12431db84ecfce1c6f22`, shared OLS/PPML input loading
batches numeric peers of strings and long retained candidate runs at N≥16,384.
Runs under 64 rows stay scalar, chunks stay bounded at 65,536, and strings keep
original encounter order. PPML offsets/exposures use the same reader. No new
observation-sized storage, excluded-cell reads, classifier, memory-admission,
solver/inference or publication change is introduced; see
[ADR 0042](docs/adr/0042-candidate-run-numeric-loading.md).

The [report](benchmarks/reports/development/candidate-input-local-2026-09-12/README.md)
binds isolated plugins and 29 completed pairs / 58 measured calls. Three-seed
command medians fall 0.615→0.445 seconds for restricted AW OLS, 0.528→0.430 for
FW OLS with string clustering, and 1.643→1.539 for restricted PW PPML with offset.
Paired gains are 27.6%, 18.9% and 6.3%. OLS input medians fall 0.390→0.217 and
0.210→0.113 seconds with stable fit/inference. PPML has no separate input timer;
its unchanged finite work also varies, so the whole gain is not attributed to
loading. Complete-data adjustment-only pairs show no reliable command gain.

All scientific exports, samples/status/DoF, certificates, iterations, retries,
fallback decisions and saved outputs match exactly. All measured action peaks
fit unchanged models; primary peak medians are unchanged. One AW pair uses 0.7%
more native peak. Worst graph-control slowdown is weak robust +8 ms / 3.7%;
complete numeric, fragmented restrictions, small/narrow, low-memory and saved
controls remain inside the predeclared review envelope. No RSS gain is claimed.

[Validation](benchmarks/reports/development/candidate-input-local-2026-09-12/validation.json)
records 43 focused plugin tests, three candidate licensed fixtures and the new
explicit-dummy/Poisson/missing-code oracle on baseline, with r(0) sentinels.
Universal static and native arm64 ABI audits pass. A PPML control input-cache
name collision stopped setup before estimation; all four controls passed with
distinct identities afterward. Initial unit expectation and collector corrections
are disclosed. Installed dist, unrelated Veneto work and historical evidence
remain untouched. No SCC, push or qualification ran.

P1 loading is adopted; adjustment-only speedup remains unsupported. Continue P3
inference ownership/lifetime comparisons and the remaining P0/P2/P4–P7 work.
The full active goal (including later independent agents, CMG assessment and the
new small SCC campaign) remains open; P1 completion does not complete #27.

## P1 PPML ado preparation: adopted local checkpoint

Against `8be388e280eb64af5de9830204a63bee8de3b33d`, remove three nested PPML
sort-preservation scopes and the optional ado completeness marker/markout/count.
Candidate and weight-expression handling, authoritative native row/domain rules,
pre-admission classification and all publication/rollback checks remain intact.
[ADR 0041](docs/adr/0041-ppml-ado-preparation.md) records the decision.

The [report](benchmarks/reports/development/ppml-ado-preparation-local-2026-09-12/README.md)
binds the same native plugin in both arms and 25 pairs / 50 measured calls.
Primary complete-command paired medians improve 2.4% (complete one FE), 1.1%
(FW/exposure/missing) and 0.4% (PW/offset/two-way cluster). Outer Stata medians
fall 0.271→0.101, 0.092→0.052 and 0.074→0.042 seconds. Full native work varies;
these are modest command gains, not native-kernel gains. All full scientific
exports and solver/retry/fallback decisions are exact. Primary native peaks
are unchanged and all action peaks fit their unchanged models.

The low-memory control initially slows 957→1085 ms. Six retained follow-ups
plus that original yield a −1.0% median paired change, with variation mostly in
unchanged native finite/strict work; no low-memory speedup or precise external
cause is claimed. Other graph controls slow by at most 6 ms. Four distinct
candidate licensed fixtures and the baseline native-preparation fixture pass
with r(0) sentinels; 17 transaction and five registration tests pass. Tests cover
independent Poisson science, all missing codes, FW/PW, string clusters, singleton
ordering, saved outputs, original row/sort metadata and injected failure exits.
The [validation](benchmarks/reports/development/ppml-ado-preparation-local-2026-09-12/validation.json)
retains the corrected initial weighted-varlist draft failure. Installed/dist and
separate Veneto work remain untouched. No SCC or qualification ran for this step.

Continue #27's remaining preparation/loading, pass, inference, centered-tile,
output and scheduling investigations; the five weak-route allocation events also
remain open. The active user goal now explicitly authorizes completing #27,
then two independent agent investigations and their evidenced optimizations,
then assessment of upstream CMG small-component changes, followed by a new small
SCC benchmark report (about ten tasks, roughly 3–5 hours each, smaller datasets
across graph families and primarily fixest/pyfixest/reghdfe comparisons). This
is later work, not a restart of either stopped historical campaign. No push is
authorized. P0–P7 and R1–R5 completion must be evidenced, not inferred from the
completed checkpoints.

## P4 stable reference passes and scratch: adopted local checkpoint

Against `d32f05e45519aaee5c5d44bc87caddb4f68482f3`, two isolated changes are
adopted: reuse the retained identity bank for frozen-solve QR scratch, then
schedule current-weight reference norms/means by column for fast PPML with one
categorical FE at N≥65,536, eight stored columns and multiple installed workers.
Each column preserves original row order, scaled norms and compensated means;
there is no observation-sized cache, stale-weight reuse or solver/rank-policy
change. See [ADR 0040](docs/adr/0040-ppml-stable-reference-pass-scheduling.md).

The [report](benchmarks/reports/development/ppml-reference-passes-local-2026-09-12/README.md)
and [identities](benchmarks/reports/development/ppml-reference-passes-local-2026-09-12/identities.json)
bind three plugins and 36 paired comparisons / 72 measured calls. Scheduling
versus the allocation-only baseline improves public primary command medians
11.800→10.179 seconds (complete one-FE robust, N983040/K50/16 threads),
3.741→3.342 (FW/exposure/missing, N245760/K50/four threads), and 3.546→3.075
(PW/offset/two-way cluster, N245760/K50/16 threads). Median paired gains are
13.7%, 10.7% and 14.8%. Existing projection and strict/final timers locate the
savings. Preparation/inference remain near baseline; primary native peaks are
identical across arms at about 2.523 GB, 633.5 MB and 645.3 MB. All
observed native action/profile peaks remain within unchanged models. RSS is
reported separately, including an FW median increase; no RSS gain is claimed.

All scientific exports, sample/status/DoF, actual/reported solver iterations,
strict retries, fallback decisions and saved outputs are exact. Weak/dense/
bottleneck/cycle/disconnected/unbalanced, small/narrow, low-memory, restricted,
saved/separated and row-threshold controls pass. Worst scheduling slowdown is
55→62 ms on disconnected (+12.7%, +7 ms), below the declared review threshold.
An allocation-only low-memory flag (878→963 ms) did not reproduce in two
independent-seed repeats; the original remains archived.

The scratch fix removes the observed one allocation per frozen solve. Real
16-iteration audits change 16→0 events for fast PW and low-memory FW one-FE
cases, with passing whole-iteration allocation certificates. Subsequent FW/PW
scheduling profiles also record zero. The weak-graph audit changes 21→5 events
and **still fails** that operational certificate, despite stable live/peak
memory and exact science. The failed broad assertion and narrowed adoption
claim are explicitly recorded; the five residual events remain open.

[Validation](benchmarks/reports/development/ppml-reference-passes-local-2026-09-12/validation.json)
records 148 distinct focused Rust tests, three licensed allocation-only fixtures,
a new N65536/K8 full-dummy Poisson/Hessian/sandwich oracle against the final
plugin, five registration checks and universal static/native arm64 ABI audits.
No full scientific profile, SCC, Windows or qualification work ran. Installed
dist and unrelated Veneto status/evidence remain preserved; nothing was pushed.

Next: localize the five weak-route allocation events, then measure remaining
raw RHS/stride and within-norm passes or P3 inference ownership. Current-frame
norm reuse, skipping fresh-input checks, broader generated-column/low-memory
coverage and P5 centered tiles remain hypotheses. Scheduling does not itself
remove a mathematical reference-statistic pass.

## P4 PPML one-FE projection: adopted local development checkpoint

Against `0889721a7c781618dd9f132642a85d065e34afad`, fast PPML with one categorical
intercept effect now enters the existing reusable categorical projector and
fused changing-response/retained-design projection. It certifies after the first
group sweep, preserves zero-iteration certification of useful warm candidates,
and allocates no accelerator matrices. Current-weight raw reference norms/means,
rank/QR policy, final-Hessian synchronization, independent certificates and
strict RAII rollback remain unchanged. No automatic family region, memory
admission or separate reconstruction behavior changes; see
[ADR 0039](docs/adr/0039-ppml-one-fe-retained-projection.md).

The [report](benchmarks/reports/development/ppml-one-fe-local-2026-09-12/README.md)
and [identities](benchmarks/reports/development/ppml-one-fe-local-2026-09-12/identities.json)
bind 23 paired comparisons / 46 measured calls: nine public primary pairs,
ten protected pairs, three separate private allocation profiles and one pilot.
Three-seed command medians fall 14.027→11.724 seconds for complete one-FE robust
PPML at N983040/K50/16 threads, 4.422→3.708 for N245760/K50/four-thread FW with
exposure/missing data, and 4.223→3.572 for N245760/K50/16-thread PW with offset and
two-way clustering. Median paired gains are 16.3%, 16.0% and 15.7%. Projection
medians fall 3.555→2.001, 1.201→0.730 and 1.203→0.681 seconds; strict/final
medians fall 5.046→4.281, 1.292→1.102 and 1.243→1.078. Preparation and inference
are largely unchanged; overlapping timers and independent medians do not sum.

Reported outer/inner counts, step halvings, strict retries and fallback decisions
are unchanged. Actual MAP iteration totals fall 5→3 and 6→4 because certified
warm solves can require zero projections; the runtime count charges at least one
per solve. All samples/statuses/DoF/weight/cluster/separation/receipt identities
are exact; b/V differ at roundoff scale (max 5.99e-17 / 1.28e-20), and independent
certificates and saved outputs pass. Unaffected weak/dense/bottleneck/cycle/
disconnected/unbalanced and low-memory routes retain exact science. Worst
protected slowdown is cycle 57→64 ms (+12.3%, +7 ms), below the predeclared
max(10 ms, 5%) review threshold; small/narrow and requested-output controls improve.

Primary native peaks rise 3.4–3.5% (84.16 MB large, 21.04 MB weighted) because
the existing group/RHS workspace now serves one FE. All observed action and
profiled phase peaks remain inside unchanged models. Real allocation audits in
both arms record 16 events in 16 iterations, without live/peak growth: retained
capacities and fresh/warm equivalence pass, but the whole-iteration
allocation-free certificate remains false. Event sizes/call sites remain open;
neither mock-probe tests nor passing science imply that operational certificate.

[Validation](benchmarks/reports/development/ppml-one-fe-local-2026-09-12/validation.json)
records 140 focused Rust tests, five licensed fixtures with r(0) sentinels and
five profile-registration checks. New independent weighted group-mean and
full-dummy Poisson/Hessian/sandwich oracles cover unit/FW/PW and robust/clustered
inference. Universal static audits and native arm64 ABI tests pass; no complete
scientific profile, SCC, Windows or qualification run occurred. The installed
plugin and unrelated Veneto status/evidence remain preserved.

The subsequent reference-pass/scratch checkpoint above supersedes this
checkpoint's allocator and scheduling resume point. Its original
[cost audit](benchmarks/reports/development/ppml-one-fe-local-2026-09-12/cost-audit.md)
remains source-bound evidence; current-frame norm reuse and the broader pass
experiments remain open.

## P1 PPML numeric ownership: adopted local development checkpoint

Against `b19a1cfac038dcc6fbf015cfbfaf9f51022dafb4`, production PPML now constructs
the admitted numeric row ledger and weight reports without a throwaway compact
copy of outcome/model sources. It consumes offset/exposure storage through stable
compaction within the candidate-capacity bound. Raw original-axis sources and raw
pweights remain available for separation rebuilds. Allocation-free preflight and
exact shape agreement, original-row errors, canonical weight/exposure semantics,
solver/session/reconstruction code and existing memory admission remain unchanged.
[ADR 0038](docs/adr/0038-ppml-numeric-selection-ownership.md) records this boundary.

The [report](benchmarks/reports/development/ppml-owned-preparation-local-2026-09-12/README.md)
and [identities](benchmarks/reports/development/ppml-owned-preparation-local-2026-09-12/identities.json)
bind isolated universal plugins and 23 completed pairs / 46 measured calls:
nine public primary pairs, ten protected pairs, three private audit/profile pairs,
and one separately reported exploratory pilot. Three-seed command medians fall
15.395→14.408 seconds for N983040/K50/16-thread complete one-FE robust PPML,
4.683→4.534 for N245760/K50/four-thread FW with exposure/missing data, and
4.591→4.452 for N245760/K50/16-thread PW with offset/two-way clustering. Median
paired gains are 6.4%, 4.5% and 3.3%. Existing preparation-phase medians fall
1.615→0.880, 0.351→0.227 and 0.358→0.221 seconds; this phase also includes IRLS
initialization. Inference and all iteration/solver/fallback decisions are exact
apart from timing. Every b/V/sample/status/DoF/weight/cluster/separation/certificate
and requested saved-output comparison passes exactly.

The removed complete-case source-copy payload is 408,944,640 bytes, derived from
source shape rather than hardware traffic. Overall native peak stays about
2.438 GB for the large complete case; weighted peaks fall about 2.21 MB because
the loaded adjustment no longer remains live beside its canonical offset. Later
session initialization dominates the preparation phase peak. Private allocation
audits pass, with all observed action peaks within unchanged models. Sampled RSS
and all phase/peak/fallback results are retained; no 409 MB fit-peak reduction is
claimed. Weak/dense/bottleneck/cycle/disconnected/unbalanced, small/narrow,
restricted, low-memory and saved-separation controls retain exact science; worst
slowdown is 1 ms. Low-memory FW improves 0.925→0.843 seconds.

[Validation](benchmarks/reports/development/ppml-owned-preparation-local-2026-09-12/validation.json)
records 136 focused Rust tests, four licensed Stata fixtures with explicit r(0)
sentinels, analytic Poisson/FW-expansion/PW-scaling oracles, and universal static
audits plus arm64 ABI/self-tests. One weighted extraction attempt was excluded
after a shared benchmark do-file edit corrupted Stata's incremental reading;
per-run worker copies/hashes now prevent that race, and both arms were rerun.
The installed/dist plugin, separate Veneto STATUS additions and historical
evidence are preserved. No SCC, push or qualification occurred.

P1 numeric ownership is adopted. The source audit identifies PPML's triple
sort-preservation wrappers and input markout/count as removal candidates, but
does not adopt ado changes. Batched adjustment/restricted-source loading,
post-admission parallel classification, generation-copy reduction and P0 counters
remain open. Next P4 work should isolate fresh one-FE RHS/norm/mean construction,
warm projection and strict/final-Hessian passes while preserving reference scales
and weight-generation validity. The large candidate's projection/finite/strict
scopes remain substantial; these scopes overlap and must not be summed. P3 OLS
inference remains a separate measured opportunity after the earlier Schur gains.

## P2 weighted OLS Schur: adopted local development checkpoint

Against baseline `39d1a2a876b92dcc7f8032be792ee171cb85cfdd`, the OLS planner now
admits supplied weights through the existing three-RHS probe and weighted Schur
engine. The probe uses actual normalized weighted degrees, statistics and
operator applications. It preserves the 0.01 contraction guard, eight charged
probe steps, 64-step full attempt, shared continuation budget, raw reference
scales and independent final certificates. The 16--64 actual RHS / 16,384-row
fast-mode envelope, saved-FE exclusion and explicit solver semantics remain.
[ADR 0037](docs/adr/0037-weighted-ols-schur-admission.md) records the development
admission. PPML session/reconstruction files and separate Veneto work are intact.

The [report](benchmarks/reports/development/weighted-schur-local-2026-09-12/README.md)
and [source/artifact identities](benchmarks/reports/development/weighted-schur-local-2026-09-12/identities.json)
bind isolated universal plugins and 39 completed pairs / 78 measured commands.
Three-seed command medians fall 6.623→1.244 seconds for N983040/K50/four-thread
AW two FE, 0.780→0.237 for N245760/K50/16-thread FW, and 1.042→0.490 for PW
two-way clustering. Median paired gains are 80.8%, 69.6% and 53.0%. Separate
traces reduce initial absorption 5.407→0.466, 0.530→0.056 and 0.598→0.065 seconds,
including probes of 45.1, 8.0 and 6.8 ms. No natural full-solve fallback occurs.

The large AW target's native peak rises 12.4%, from 1.156 to 1.300 GB within
its unchanged 1.675 GB model. FW and PW native peaks fall; all per-case native,
modeled and sampled RSS results are retained. Unchanged low and small/narrow
controls have observed/model discrepancies of +10.0% and +27.9% in both arms;
these remain P0 modeling work, not qualified memory evidence.

All numerical comparisons and original certificates pass, with maximum full
b/V differences 2.67e-15 / 3.55e-14. Unit-weight weak/dense/bottleneck/cycle/
disconnected/unbalanced controls retain exact science and solver decisions,
with at most 7 ms slowdown. Tiny weighted cuts reject Schur (0.0583 contraction)
and keep exact CMG science. Their repeats have median paired cost +3.8%; an
individual +16 ms / 6.1% case and the traced 2.7 ms rejected probe are disclosed.
The remaining traced cost lies mostly in unchanged legacy absorption; its
precise cause is unestablished. Small/narrow repeats are 14→13 ms medians after
an initial 11→15 ms observation. Saved outputs remain exact; width/low controls
preserve the existing eligibility boundaries.

[Validation](benchmarks/reports/development/weighted-schur-local-2026-09-12/validation.json)
records 60 prepared-fit and three additional Schur tests, four distinct licensed
Stata fixtures with a fifth invocation for added rank checks, binary audits and
actual arm64 ABI/self-tests. Weighted dummy, frequency-expansion, AW/PW rescaling,
finite/FE collinearity, 1/4/16-thread, memory-budget, forced-continuation and
transaction checks pass. Two builds from an incorrect working directory were
excluded before comparisons; correctly isolated artifacts were used throughout.
The installed/dist plugin is unchanged. No SCC, push or qualification occurred.

P2 weighted probe/admission, bounded continuation and width comparisons are
adopted. Simple weighted one-FE already uses direct projection; complex one-FE
pass reduction and separate constant-weight specialization remain open. Continue
with #27's P4 one-FE/pass work and remaining P1 PPML materialization, preserving
the allocation-free preflight and reconstruction repair; P3 inference remains
a measured bottleneck after the absorption gains. P0 modeling/counter gaps and
the weighted-cut routing cost remain explicit follow-up work.

## P0/P1 OLS preparation: adopted local development checkpoint

Roadmap [#27](https://github.com/johannes-schmieder/fereg/issues/27) supersedes
#26 and #16--25 for optimization priorities. The baseline for this work is
`e93fe437f1d8855e1c3aa47ad03026eb23a9942d`, including the committed PPML
reconstruction repair. Its separate evidence and the Veneto status edits below
are preserved; no cluster job or installed/dist plugin was changed.

All six OLS preparation branches now consume loaded source buffers through
one ownership-aware boundary: unweighted/AW/FW/PW; IID/robust/one- and multiway
cluster inference; simple, interacted, slope and saved absorbed terms. The
existing row classifier, original-row domain precedence, raw weight kinds and
values, cluster missingness before singleton deletion, true categorical
singleton fixed point, final coding and candidate-capacity bounds are retained.
Wide preparation reuses the requested worker team after SPI loading. PPML
preflight/materialization and all solver/inference/transaction rules remain
unchanged. [ADR 0036](docs/adr/0036-native-ols-input-preparation.md) and the
[coverage/cost report](benchmarks/reports/development/ols-owned-preparation-local-2026-09-12/README.md)
record this extension.

[Source/artifact identities](benchmarks/reports/development/ols-owned-preparation-local-2026-09-12/identities.json)
bind isolated universal baseline/candidate plugins. The
[41 paired comparisons](benchmarks/reports/development/ols-owned-preparation-local-2026-09-12/comparisons.json)
include 82 successful measured local calls. Three-seed command medians fall
from 1.870 to 1.294 seconds for unweighted robust sparse-missing N983040/K50,
1.197 to 0.641 for AW IID missing, 0.652 to 0.527 for FW clustering at N245760,
0.490 to 0.352 for complete PW robust, and 0.638 to 0.510 for an interacted FE
with two-way clustering. Separate traces reduce AW preparation from 0.598 to
0.042 seconds while fit/inference remains about 0.204 seconds.

All pairs have exact full b/V, sample, status, DoF, fit, cluster and certificate
results and solver decisions apart from timings. Native peaks agree in 35/41
pairs, including every primary pair; restricted preparation falls from 129.1
to 118.3 MB. Weak/dense/bottleneck/cycle/disconnected/unbalanced controls retain
science, with at most 7 ms slowdown. Small/narrow repeats remain 7 versus 9 ms;
this disclosed regression is within the predeclared 10 ms review envelope.
The original 1.973-second AW candidate input-loading outlier remains in the
primary data; three separate rechecks are 0.614--0.623 versus 1.172--1.194.

[Validation](benchmarks/reports/development/ols-owned-preparation-local-2026-09-12/validation.json):
160 core tests, 40 plugin tests, 29 independent Rust oracle/preparation tests,
six distinct licensed Stata fixtures plus a constrained-memory rerun, universal
binary audit and actual arm64 ABI/self-test pass. The 543870912-byte constraint
rejects fast with r(909) and admits low/auto with matching science. The Python
boundary screen remains 25/26 on both baseline and candidate: an unchanged
prepared-view dynamic-dispatch assertion fails in `ols/prepared_fit.rs`.
No test is weakened or reported as passing when it failed.

P1 OLS ownership/weights/clustering/complex terms/team reuse is adopted for local
development. P0's source-derived copy costs and representative routes are
recorded; hardware traffic and full setup/build counts remain unmeasured.
PPML post-admission ownership, adjustment loading/ado work and restricted/mixed
SPI batching remain open. Next priorities remain P2 weighted absorption and
P4 one-FE PPML passes, then measured inference ownership and stable centered
tiles under #27. No new automatic solver region, SCC, Windows, qualification,
push or release is implied.

## PPML reconstruction fix: completed development validation

The requested Veneto repair changes the retained weighted-linear session's
reconstruction check to an operation-count rounding bound and constructs
canonical PPML predictors directly from the working response and residual.
The original response-relative diagnostic and final publication tolerance
remain unchanged, as do all other scientific checks and the one-shot OLS path.
[Numerical contract](docs/ppml_reconstruction.md) records the bound and
independent rational oracle, including corrupted and nonfinite negative cases.

[Local evidence](benchmarks/reports/development/veneto-ppml-reconstruction-2026-09-12/local-tests.json) records
94 passing Rust tests. The rebuilt universal macOS plugin passes binary audit,
actual arm64 ABI/self-test, and four licensed fixtures: OLS and PPML smoke,
PPML scientific contract, and weighted inference. Strict Clippy remains blocked
by six pre-existing lints in unchanged OLS/FE files, with none reported in the
changed files.

The [repair report](benchmarks/reports/development/veneto-ppml-reconstruction-2026-09-12/README.md) records
successful full Veneto fits on all 3,159,720 observations at four/fourteen
threads: 133.324/101.878 seconds, both with 15 outer and 589 inner iterations.
Slopes differ from the preserved ppmlhdfe reference by at most 4.59e-12;
SE and full slope covariance comparisons pass. Both final score/Hessian checks
pass, with identical samples and slightly better reported objectives. Native
allocation peaks are 5.958/7.374 GiB; these single runs are diagnostic, not a
general scaling claim.

SCC job **7536374** completed with scheduler `failed=0`, `exit_status=0` and
application/job sentinels. It reused the Linux plugin built and smoke-tested
in job **7536367**, whose later controller path typo stopped before full-panel
estimation. Both attempts remain documented. The
[validation](benchmarks/reports/development/veneto-ppml-reconstruction-2026-09-12/scc-validation.json) and
[source manifest](benchmarks/reports/development/veneto-ppml-reconstruction-2026-09-12/source-manifest.json)
bind the results to snapshot
`659924a603a3b750a7478cad608d95022166b5a83956e066d9b9e29698420b96`;
all 484 source/fixture files match the final working tree. The source and
portable validation evidence form the requested development checkpoint. No
qualification or broad benchmark restart occurred.

## Incomplete OLS compaction: preceding implementation

The user-requested development checkpoint collects the adopted OLS sources,
fixtures, benchmark tooling and publishable numerical evidence. The
[checkpoint inventory](benchmarks/reports/development/adopted-ols-checkpoint-2026-09-12.json)
verifies that production and fixture bytes match the retained local validation.
It records the path-bearing captures preserved outside Git and the configurable
SCC directory settings. No estimator benchmark or qualification was rerun for
this packaging step. Historical source identities and receipts are unchanged.

Unweighted IID preparation now consumes the authoritative row classifier's
drop mask and compacts owned columns independently. Wide inputs classify
16,384-row blocks on the installed requested team, preserving original-row
error order. Buffer reuse stays within the prior candidate-axis capacity
bound. Complete inputs retain their shortcut; weighted, complex categorical,
robust/clustered and PPML preparation keep their existing paths. No solver,
sample, tolerance, inference or transaction rule changes.

The [compaction report](benchmarks/reports/development/fixest-compaction-local-2026-09-12/README.md)
records 53 completed local invocations: 51 successful estimates and two
explained all-singleton negative controls. For 983,040 rows, 50 controls and
about 0.1% missing, three-seed previous/current medians are 1.461/0.926 seconds
at four threads and 1.230/0.613 at sixteen, improvements of 36.6% and 50.2%.
The original four-thread input-time outlier remains in the summaries; an
extra pair does not reproduce it. Complete-data one FE remains essentially
unchanged at 0.585/0.580 seconds versus fixest's 0.472, leaving a 22.9% gap.
Weak, dense, bottleneck, cycle, disconnected and unbalanced holdouts differ
by at most nine milliseconds and preserve all numerical results.

All 20 final candidate/previous pairs have byte-identical full b/V exports,
exact samples/statuses/DoF/fit/certificate tables and solver diagnostics
except timings. All seven fixest comparisons pass. Native allocation peaks
agree in 18 pairs; the two heavily filtered retained-pair holdouts fall from
220,448,053 to 120,419,044 bytes. Both versions return Stata code 2001 when
the initial filtering recipe leaves only singletons; those original failure
receipts remain intact.

[Validation](benchmarks/reports/development/fixest-compaction-local-2026-09-12/validation.json)
records 37 core preparation tests, five sample tests, 40 plugin action tests,
eight licensed Stata fixtures, universal binary audit and actual arm64
ABI/self-test. New coverage includes cross-block error order, retained value
bits/capacity, singleton cascades, and independent sparse/dense/restricted
wide dummy regressions at 1/4/16 threads.
[Replayable source identities](benchmarks/reports/development/fixest-compaction-local-2026-09-12/identities.json)
bind the retained plugin SHA-256
`43377269d0bbfd7dc1bc2a964e117d9f282e123350820252289e3f6b76a92fae`.

Remaining complete-data one-FE component medians are 0.303 seconds input,
0.210 native and 0.069 outer Stata work. Further parity work must address
loading or native estimation passes. Ownership/column compaction for weighted
and robust/clustered paths is a separate next experiment; small/narrow gaps
also remain unresolved. No SCC, remote job, qualification, commit, push or
release action was taken.

## Native OLS input preparation: preceding implementation

The local follow-up moves full-candidate numeric missing detection from ado
into Rust, removes the unused OLS sort-preservation wrapper, and installs
wide IID preparation on the requested worker team for independent source
completeness checks. The C bridge only transports SPI values and the SDK
missing boundary. Restricted/string loading, authoritative sample rules,
solver routes, tolerances, all numerical formulas and transaction guarantees
remain unchanged. [ADR 0036](docs/adr/0036-native-ols-input-preparation.md)
records the development extension to ADR 0013's loading procedure.

The [input report](benchmarks/reports/development/fixest-input-local-2026-09-12/README.md)
records 134 completed local measurements, including 54 rotated primary calls
at three seeds. At 983,040 rows and 50 controls, previous/current/fixest
medians are 0.916/0.776/0.806 seconds for one FE at four native threads,
0.690/0.582/0.454 for one FE at sixteen, 0.878/0.763/0.841 for degree three
at sixteen, and 0.836/0.712/0.695 for degree six at sixteen. Current fereg
narrowly beats fixest on four-thread one FE, leads degree three by 9.3% at
sixteen threads, and trails degree six by 2.4%. One FE at sixteen threads
still trails by 28.2%. Small/narrow one-FE cases also retain gaps.

All 39 final candidate/previous pairs have byte-identical full b/V exports,
exact samples/statuses/DoF/fit/certificate tables and solver diagnostics
except timings. All 25 final fixest comparisons pass. Weak, bottleneck,
cycle, disconnected, unbalanced and dense holdouts retain their performance
in these bounded local checks. Modeled memory budgets agree; native peak
allocation is unchanged except an unexplained 819,240-byte decrease in one
narrow weak holdout, which is not claimed as a memory improvement.

[Validation](benchmarks/reports/development/fixest-input-local-2026-09-12/validation.json)
records 35 core preparation tests, 40 plugin action tests, eight licensed
Stata fixtures, universal binary audit and actual arm64 ABI/self-test. The
new fixture checks all 27 Stata missing codes in five storage types, weights,
clusters/strings, exclusions, singleton deletion, row/sort-state preservation
on direct failure exits, and independent wide IID dummy regressions at
1/4/16 threads. Existing TS, transaction, saved-output, memory-mode,
weighted/robust oracle and PPML smoke fixtures also pass.
[Source identities and replayable patches](benchmarks/reports/development/fixest-input-local-2026-09-12/identities.json)
bind the retained plugin SHA-256
`93412e7c7654deac201f5f8c1bdc5af62aece7ebf4347180df0acebe3d227470`.

An 8,192-row loading chunk gave no consistent benefit and is rejected;
65,536 rows remain. Current sixteen-thread one-FE component medians are
0.301 seconds input, 0.207 native and 0.073 outer Stata work. Further
one-FE gains need fewer native data/layout passes or a demonstrably useful
loading overlap, not just more workers for the now-small completeness check.
A separate three-seed sparse-missing probe improved from 2.846 to 1.174
seconds after removing the input traversal cliff, but still exposes costly
row-wise native classification/copying when a small fraction of rows drops.
That report identified owned column compaction as the next experiment;
the implementation and new evidence are recorded above.
No SCC, Linux/Windows science, qualification, commit, push or release action
was taken. Historical receipts remain unchanged.

## Wide-OLS scheduling: preceding implementation

The local follow-up retains three OLS scheduling changes: disjoint Schur
recurrence updates above seven workers, parallel three-column probe setup,
and fixed group partials for wide one-FE/Schur certificates. The shared
PPML recurrence crossover, existing weak-graph solver routes, all acceptance
maxima, tolerances, iteration/fallback rules and memory budgets are unchanged.
The diagnostic two-norm changes summation order; it does not control
acceptance on these routes. No new workspace is allocated.

The [scheduling report](benchmarks/reports/development/fixest-scheduling-local-2026-09-12/README.md)
records 140 completed local measured calls, including 54 rotated primary
comparisons at three seeds. At 983,040 rows and 50 controls, sixteen-thread
previous/current/fixest medians are 0.951/0.881/0.838 seconds on degree three
and 0.875/0.835/0.704 on degree six: 7.4% and 4.6% improvements, leaving gaps
of 5.1% and 18.6%. At four threads current fereg remains 32% and 24% faster
than fixest on these designs. One-FE medians are 0.947/0.687 seconds at 4/16
threads; the small four-thread slowdown did not recur consistently in three
additional paired checks. All weak/bottleneck/cycle/disconnected/dense
holdouts pass and retain their performance within the bounded local evidence.
The separate eight-thread crossover checks improve and beat fixest.

All 35 final candidate/prior pairs have byte-identical full b/V exports,
samples, statuses, DoF, fit/certificate tables, acceptance maxima, routing,
iterations and fallback. The largest diagnostic norm relative change is
7.3e-14. All 24 final fixest comparisons pass. Primary peak allocated bytes
are exactly unchanged; one unchanged narrow weak holdout has an unexplained
3.8 MB lower reported peak, which is not claimed as a memory improvement.
[Validation](benchmarks/reports/development/fixest-scheduling-local-2026-09-12/validation.json)
records 34 focused Rust FE tests, four licensed Stata fixtures (including the
weighted/robust explicit-dummy oracle at 1/4/16 threads), universal binary
audit and actual arm64 ABI/self-test. The retained plugin SHA-256 is
`20289553c62d5280ece81a07dc39abeecaa573430c34a280cfb3c7cc1b2df763`.
[Source identities and replayable patches](benchmarks/reports/development/fixest-scheduling-local-2026-09-12/identities.json)
bind the uncommitted source; this is development evidence, not qualification.

Independent one/four-variable Schur jobs and RHS-parallel scalar reductions
were slower and are removed. Retain the batched graph operator unless a
better layout wins a controlled comparison. The remaining roughly 0.45–0.46
seconds of input/outer Stata work matters especially for one FE; further
native data/normalization passes and staging/input work are the next local
targets. No SCC campaign was submitted or resumed, and no commit, push or
release action was taken. Historical receipts remain unchanged.

## Numeric passes: preceding implementation

The local follow-up retains tiled numeric RHS construction and independent
raw-residual updates, parallel initialization of already reserved storage,
and parallel wide-source finite checks within an installed worker pool.
It changes four OLS source files; the preceding Schur route, C bridge and
Stata transaction are unchanged. General validation before pool installation
stays serial. No tolerances, sample/weight rules, rank, DoF or covariance
formulas change, and no additional observation-scale allocation is introduced.

The [numeric-pass report](benchmarks/reports/development/fixest-passes-local-2026-09-12/README.md)
records 139 measured calls, including 54 rotated primary comparisons at three
seeds and 44 final holdout/profile/repeat calls. Relative to the retained
Schur baseline, complete-command medians fall another 5–10%. At 983,040 rows
and 50 controls, current/fixest medians are 0.951/0.798, 1.326/1.927 and
1.222/1.578 seconds for one FE and degree-three/six graphs at four threads.
At sixteen threads they are 0.704/0.470, 0.960/0.835 and 0.878/0.692.
The tested wide weak-graph holdouts retain their performance. A 6 ms narrow
holdout difference did not recur across three additional paired seeds.

All 35 final current/prior pairs have identical exported full b/V files,
fit/certificate tables and solver diagnostics except timings. All 22 final
candidate/fixest comparisons pass. The final source passes 7 RHS tests,
60 prepared-fit tests and four licensed Stata fixtures, including the
50-control weighted/robust explicit-dummy oracle, saved outputs, memory
modes and PPML smoke. The universal macOS build passes binary audit and the
actual arm64 ABI/self-test. This is development evidence, not qualification.
[Source identities, patches and validation](benchmarks/reports/development/fixest-passes-local-2026-09-12/identities.json)
bind the retained plugin SHA-256
`26aca86c83c1cc7426a8de6601b1d8ee632acc43e3e5aacc9f0c3ea3a3ed0a2b`.

The report retains a 1.337-second one-FE outlier, largely located in unchanged
input loading; it did not recur in two paired checks, but its cause is not
established. Two external RSS samples miss the native peak and are flagged;
no new RSS ranking is claimed. Native peak allocated bytes and modeled fast
budgets are exactly unchanged across the 18 primary pairs.

The main remaining cost is about 0.45 seconds of input/outer Stata work at
sixteen threads. Callback hoisting and C-side tiled copying did not establish
a useful improvement and were rejected. Next local investigations are smaller
wide-model scheduling, remaining native passes, and reuse of private Stata
staging storage without weakening sampling or transaction guarantees. No
SCC campaign was submitted or resumed, and no commit, push or release action
was taken. Historical reports remain unchanged.

## Guarded OLS Schur route: preceding implementation

Johannes approved ambitious local work to approach fixest while preserving
weak-graph strengths. [ADR 0035](docs/adr/0035-guarded-wide-ols-schur.md) admits a
guarded Schur route for fast, unit-weight, two-FE OLS at 16–64 actual RHSs and
at least 16,384 observations. An eight-step three-column probe selects the
full-width solve; a 64-step cap and ordinary certified CG continuation share
the iteration budget. Both workspaces must fit the joint memory budget.
Other route classes retain the previous planner. The one-FE initial projection
now reaches its exact direct path, and fresh certified norms move into OLS.
Parallel group preparation/reconstruction and deterministic OLS norm reductions
remove repeated work without weakening scientific checks.

The [September 12 report](benchmarks/reports/development/fixest-gap-local-2026-09-12/README.md)
preserves 148 completed measured invocations, including 54 rotated final
primary calls at three seeds, 25 final holdout/narrow calls and six final
profiles. At 983,040 rows and 50 controls, four-thread medians are 1.404/1.297
seconds for degree-three/six versus fixest's 1.916/1.548 and baseline fereg's
7.630/6.962. Sixteen-thread medians are 1.009/0.936 versus fixest's 0.835/0.686.
One-FE time falls to 1.034/0.769 at 4/16 threads; fixest remains faster at
0.795/0.447. The weak and dense holdouts improve; the difficult cycle rejects
Schur and incurs about 3 ms of probe work. Narrow controls retain exact full
b/V parity. These are local development measurements, not universal graph
parity or release evidence.

Independent full dummy-model solves adjudicate native fixest discrepancies
on bottleneck and disconnected graphs. The final candidate matches the
independent coefficients/RSS/DoF. A native fixest stopping discrepancy and a
separate connected-component DoF convention difference remain explicit
failed native-parity comparisons; neither is turned into an equal-accuracy
speed claim. The earlier [September 11 report](benchmarks/reports/development/fixest-gap-local-2026-09-11/README.md)
remains unchanged, including the prototype weakness explained by the corrected
recurrence/certificate scaling.

[Validation](benchmarks/reports/development/fixest-gap-local-2026-09-12/validation.json)
records 33 borrowed-FE and 58 prepared-OLS tests, seven licensed Stata fixtures,
and the normal universal macOS build with binary audit and actual arm64
ABI/self-test. That report's measured plugin SHA-256 is
`ffa5537a0da609888de55ad122973140f86f48700a3368fb2f3098a785d072ac`.
[Source manifests and replayable patches](benchmarks/reports/development/fixest-gap-local-2026-09-12/identities.json)
bind the uncommitted source snapshot to base `d44e36bfa5ad7`; the base SHA
alone does not identify these changes. The [local harness](benchmarks/local_fixest_gap/README.md)
contains the follow-up reproduction entry points. No SCC job was submitted,
resumed or changed for this investigation; the stopped systematic campaign
stays stopped. No commit, push or release action was taken.

The numeric-pass follow-up above addresses the next RHS and raw-residual
work. Coefficient-space Irons–Tuck and batch missing detection were slower
and were not retained in this preceding investigation.

## Veneto benchmark: stopped and reported

Johannes stopped the Veneto campaign on September 12. Array **7529406** was
cancelled and the `finish-scc-veneto-benchmark-pdf` monitor deleted. The
[cancellation receipt](Veneto_Benchmark/results/scc/campaign-stopped.json)
confirms it left the queue: tasks 1--10 completed, tasks 11--12 were interrupted,
and tasks 13--24 never started. No campaign continuation is authorized.

The single [11-page PDF](Veneto_Benchmark/output/pdf/Veneto_Benchmark.pdf)
contains vector runtime/RSS figures across 1/2/4/8/14/28 affinity cores,
cold and warm measurement tables, numerical comparisons, PPML failure analysis
and provenance. Main figures/counts use **110 calls from ten complete tasks**;
14 finished calls from interrupted tasks are supplementary only. Two commands
were cancelled mid-estimation and are not counted as native estimator failures.
The [delivery receipt](Veneto_Benchmark/results/scc/report-delivery.json)
binds the PDF, all-page visual review, source scripts and analysis checks.
Production consumed 18.98 node-hours including cancelled tasks (17.57 in
complete tasks); setup/smoke and queue waiting are additional.

Fereg OLS matches reghdfe slopes, full slope covariance and observation samples
in all ten complete-task fits. At equal one-to-four allowed processors, cold
command times are 10.6--12.5 times shorter, with higher RSS. Supplementary
14/28-core OLS fits also pass b/V/sample checks. Independent graph analysis
finds 41,901 components, explaining most other commands' approximately 1.1%
OLS SE excess. Areg reports e(converged)=0 at 50 APM iterations in every observed
call; tested postprocessing corrects the frozen collector's numeric-zero
classification without altering original receipts.

Feppml has nine native r(430) failures and one successful cold 14-core fit in
the complete tasks; a supplementary 28-core warm fit also fails. The passing
fit matches ppmlhdfe b/V/sample, but does not establish reliable performance.
Quiet logs suppress the precise native error, so initial/ordinary/strict/final
certificate paths cannot be distinguished from these receipts. Frozen-source
inspection confirms multiple r(430) paths; no specific root cause is claimed.
Pyfixest has eight weighted-FE demeaning failures and two timeouts in complete
tasks: its 10,000-iteration inner cap is distinct from the 25-iteration outer
Poisson cap. Gglm reaches the 2,400-second cap in all ten complete tasks.
These two unsuccessful PPML arms consume 65.0% of measured command time/bounds.

[Stopped analysis](Veneto_Benchmark/results/scc/stopped-analysis/audit.json)
and [failure evidence](Veneto_Benchmark/results/scc/stopped-analysis/failure-audit.json)
preserve full comparisons, accounting, supplementary checks, data geometry and
exact-source references. All 3,159,720 PPML outcomes are strictly positive,
ruling out zero-outcome separation. Daily pay spans a 56.4-million-to-one range;
24.0% of workers are observed at multiple firms. These suggest numerical
difficulty but do not prove the failing iteration's conditioning. No estimator
was rerun, no solver/tolerance/sample was changed, and no production source was
edited for the stopped-campaign report. The subsequent user-requested logged
reproduction is a separate follow-up, described below.

The frozen candidate is **d44e36b**, preceding the newer local OLS changes.
Each task reserved 28 E5-2680v4 cores and 84 GiB, but Stata/MP was licensed for
**four processors**; native Rust and R/Python thread controls are separate.
Only one cold call exists at every setting, with one complete warm repetition
through eight cores. Different affinities used two physical hosts. The report
retains these limits, native accuracy disagreements and failed memory bounds.

The panel has 1,133,933 workers and 125,707 firms after common singleton pruning
in 1999--2001. [Sample identities](Veneto_Benchmark/results/sample.json) and
[independent small-panel checks](Veneto_Benchmark/results/independent-validation.json)
remain preserved. This is VWH source-family data, not an exact KSS replication.
The [SCC authorization](Veneto_Benchmark/results/scc/data-authorization.json)
and [transfer verification](Veneto_Benchmark/results/scc/data-transfer-verification.json)
cover six derived files staged in the owner-only run directory. Raw source data
remain local. SCC smoke 7529248 and acceptance gate 7529405 passed before
production. Local pilot/stopped receipts and the earlier nine-task preliminary
audit remain unchanged. The [SCC protocol](Veneto_Benchmark/scc/README.md)
describes rebuilding the stopped report from preserved aggregate evidence.

Johannes requested Telegram delivery and a separate PPML investigation after
the report. The [Telegram receipt](Veneto_Benchmark/results/scc/telegram-delivery.json)
records successful delivery of the unchanged PDF. The
[diagnostic addendum](Veneto_Benchmark/results/scc/ppml-diagnostic/analysis.md)
locates the reproduced failure in the initial weighted-regression predictor
reconstruction check, before any PPML update. Original-binary job **7535359**
captures the native error at four threads. Logging-only job **7535389**
identifies reconstruction as the sole failed certificate at four and fourteen
threads: response-relative errors are 2.357e-14 and 1.486e-14 against a
1.421e-14 cutoff, while term-scaled errors are below machine epsilon. The six
other initial-regression checks pass. The measured cancellation ratio is 392.
This is numerical fragility in reconstructing a small response from larger
opposing terms; fourteen cores is not a reliable workaround.

The [diagnosis receipt](Veneto_Benchmark/results/scc/ppml-diagnostic/diagnosis.json)
verifies all 2,028 frozen source files, with one logging-only modified file,
and binds the measured certificate fields to both diagnostic fits. The new
jobs use the same input and scientific settings on a Gold-6226R; their timings
are not benchmark observations. Controller log-name and build-directory
errors are preserved separately in the addendum. Final build/ABI checks,
application sentinels and scheduler failed=0/exit=0 pass for job 7535389.
All diagnostics are terminal; total reserved resource use is about 0.50 core-hours.
Production code and the frozen binary remain unchanged. The focused repair
target is stable combined-predictor construction and a justified reconstruction
certificate, followed by an independent cancellation fixture and bounded
full-panel numerical validation. Downstream PPML convergence after such a
repair has not been tested. The campaign remains stopped.

## Systematic benchmark campaign: stopped and reported

Johannes stopped this campaign on September 11. Array **7522398** was cancelled
and its monitor deleted. The [final cancellation receipt](benchmarks/reports/development/systematic-production-2026-09-11/cancellation-confirmed.json)
confirms the array is absent from the scheduler: tasks 1--78 completed, task 79
was interrupted, and tasks 80--237 never started. No continuation or checkpoint
resubmission is authorized.

The [discussion report](benchmarks/reports/systematic-stopped-2026-09-11/discussion.pdf)
and [figure atlas](benchmarks/reports/systematic-stopped-2026-09-11/figure-atlas.pdf)
analyze the collected partial evidence. The atlas retains individual vector
panels assembled with LaTeX `subcaption`. The [coverage receipt](benchmarks/reports/systematic-stopped-2026-09-11/stopped-coverage.json)
records 826 accepted registered comparison groups, plus two off-grid pilot
groups; 100 OLS configurations have six valid warm repetitions. PPML has five
accepted pilot groups and no complete repetition grid. Finished calls from
cancelled task 79 remain in the raw observations but are excluded from strict
aggregates. The [evidence archive](benchmarks/reports/systematic-stopped-2026-09-11/evidence.zip)
contains numerical receipts, provenance, accounting and analysis CSVs. Collected
raw logs are retained separately in the ignored production receipt `work/`
directory and on SCC. The [delivery receipt](benchmarks/reports/development/systematic-production-2026-09-11/report-delivery.json)
binds the eight-page discussion, 45-page atlas and checked evidence archive;
all pages were visually reviewed and all 132 panel PDFs remain vector figures.

Fereg's clearest advantage is difficult sparse FE versus reghdfe. Its main
weaknesses are the gap to fixest on well-connected designs, limited gains beyond
8--14 threads in several designs, and high RSS on sparse three-FE and the largest
degree-three case. In the latter case the automatic CMG-to-LSMR/Schwarz switch
coincides with setup increasing from 0.21 to 12.01 seconds; this is an observed
profiling lead, not a causal comparison of forced solvers. Tiny coefficient
discrepancies on the largest degree-two design remain unresolved and excluded.
Production consumed 96.3 node-hours (2,695 reserved core-hours); setup, smoke and
pilot work are additional. The grid is incomplete and ordered by model/design,
so it supports no balanced overall ranking. Estimator code and frozen scientific
thresholds are unchanged.

The requested OLS/PPML comparison harness is implemented under
[`benchmarks/systematic/`](benchmarks/systematic/README.md), with 304 unique
configurations, six warm repetitions and one cold diagnostic. It compares
the requested Stata estimators and pinned R/Python fixest versions on eight
designs, up to 3,932,160 observations. Its figures are individual vector PDF
panels assembled with LaTeX `subcaption`, including time, RSS and paired ratios.
The candidate remains clean source revision `d44e36b`; estimator code is unchanged.

The [development checkpoint](benchmarks/reports/development/systematic-2026-09-10/checkpoint.json)
and adjacent compact application/scientific/accounting receipts record the SCC
work in run `20260910-systematic-d44e36b`. Setup job 7520628 passed on a 28-core
node. Probe 7520678 passed its application checks. Smoke job 7520690 completed
all eight OLS designs with passing independent coefficient/SE checks; PPML
passed those checks for one FE and dense two FE, then its independent oracle
failed to converge on other designs. Focused retest 7521074 did not resolve the
oracle failure. The unrelated R metadata-rounding correction passed its OLS
retest. Scientific differences in native comparator output remain recorded.

Johannes authorized further investigation on September 11 before production.
Diagnostic job 7521509 completed successfully and traced three
failing cases with and without column scaling;
the [submission receipt](benchmarks/reports/development/systematic-oracle-investigation-2026-09-11-submission.json)
binds its inputs, controller and 28-core/30-minute resource request.
the [diagnosis and traces](benchmarks/reports/development/systematic-oracle-2026-09-11/diagnosis.json)
show an inner LSMR normwise accuracy floor from unequal column norms. Column
equilibration reaches the unchanged outer score threshold in six/seven iterations
for all three cases. The independent oracle now applies that correction; estimator
settings, inputs and acceptance tolerances are unchanged. The diagnostic heartbeat
was deleted. Array 7522234 (tasks 1--8) validated the final harness on both models,
all eight designs, two seeds and four/28 requested cores, with four groups packed
per task and a one-hour limit. All 32 candidate fits passed independent coefficient
and SE checks and pinned reghdfe/ppmlhdfe comparisons; all eight tasks have accepted
accounting. The [final smoke acceptance](benchmarks/reports/development/systematic-final-smoke-2026-09-11/acceptance.json)
preserves native comparator discrepancies and three gglm initialization errors.
These errors do not establish failure of the larger target fits. Its isolated
controller is `controller-final` and its output is `validation-final`.

The production controller adds same-node/task and partial-group recovery guards;
all 32 comparisons replay with unchanged outcomes, and 14 local evidence/figure
tests pass. Numerical workers, data generation and oracle are unchanged from the
accepted smoke. The runtime is frozen with fingerprint `04b5e8cf00ae6947…` under
`controller-production`. [Pilot array 7522274](benchmarks/reports/development/systematic-pilot-2026-09-11/submission.json)
was accepted with tasks 1--5, two groups per task, 28 E5-2680v4 cores,
8 GiB/core and a 12-hour limit. All ten groups and 55 arm receipts are complete,
with accepted scheduler accounting and application completion markers. The
[pilot acceptance](benchmarks/reports/development/systematic-pilot-2026-09-11/acceptance.json)
records ten successful candidate fits, nine passing pinned-reference comparisons
and one ppmlhdfe command timeout. Native pyfixest iteration failures, gglm
initialization failures/timeouts and accuracy discrepancies remain recorded.
Peak estimator RSS was 6.87 GiB; scheduler maxvmem was 13.004 GiB. The longest
task took 5,054 seconds. The pilot heartbeat was deleted after terminal accounting.

[Production array 7522398](benchmarks/reports/development/systematic-production-2026-09-11/submission-1.json)
was submitted with tasks 1--237, at most 12 concurrent tasks, 28 E5-2680v4 cores,
3 GiB/core and a 12-hour cap. It packs 2,120 remaining comparison groups; eight
registered pilot groups are reused under the same frozen runtime identity.
Measured packing targets eight hours and checkpoints unstarted groups before
the walltime cap. Native failures remain terminal benchmark outcomes and do not
trigger estimator-option changes. That continuation plan was superseded by the
user's stop instruction. Only 28-core SCC nodes were used and task walltime was
capped at 12 hours. Local checks exercise evidence acceptance and figure
assembly, never estimator benchmarks.

The [first 78 terminal tasks](benchmarks/reports/development/systematic-production-2026-09-11/first-78-validation.json)
have accepted scheduler accounting and 818 validated complete comparison groups.
Task 51 checkpointed after 20,626 seconds: ten groups completed and six never
started. Its [preserved checkpoint](benchmarks/reports/development/systematic-production-2026-09-11/checkpoint-51.json)
and [prepared two-task plan](benchmarks/reports/development/systematic-production-2026-09-11/prepared-checkpoint-51-plan.json)
are preserved as historical evidence; the prepared plan was never submitted and
will not be resumed. The frozen controller still matches its inventory. Final
accounting contains 79 started tasks, with nonzero cancellation status on task
79. Never-started tasks have no accounting record.

## OLS performance and LSMR correctness

This checkpoint collects the within-norm and wide raw-moment scan optimizations,
guarded wide finite solve, low-memory LSMR preparation reuse, certified mixed
LSMR/CG reporting, and the published parallel-repeatability repair below.
Johannes authorized committing the accumulated work on main and pushing it.
The production and controller file hashes match the tested integration
snapshot; the immutable receipts retain their original source identities.
Prior sections describe the development state when each receipt was recorded.

The approved patch is published in the within fork as
[`1ac3522`](https://github.com/johannes-schmieder/within/commit/1ac3522a251c2622dbeab451415924f9ebe117b9),
a fast-forward of its existing `fix/weighted-categorical-assembly` branch.
Both fereg runtime dependencies, Cargo.lock and the reference lock now use
that exact commit under [ADR 0034](docs/adr/0034-lsmr-parallel-reduction-order.md).
The [published-pin integration receipt](benchmarks/reports/development/ols-lsmr-repeatability-pin-local-2026-09-10.json)
binds the actual Cargo checkout, source snapshot and rebuilt plugin.
Earlier work and evidence are preserved in this checkpoint. The installed
plugin has not been replaced.

The upstream implementation changes only two indexed LSMR norm/dot sums:
one existing 4096-element chunk per reduction leaf fixes their arithmetic
tree independently of worker stealing. Thresholds, per-chunk arithmetic,
public APIs, allocations, routing, iteration budgets and independent
certificates are unchanged. No local dependency override or copied solver
internals remain in fereg. The reviewed patch and prepublication receipt are
preserved under `benchmarks/patches/` and `benchmarks/reports/development/`.

The published-pin build passes all 11 selected captures with exact science
against the reviewed patch, unchanged exact self-repetition and complete
pinned reghdfe checks. This includes the four original N100,000 seven-thread
failures: balanced two FE, balanced three FE and mixed three FE in fast
memory, plus mixed three FE in low memory. The serial low-memory case and
six K29/pool-size/weight/one-FE controls also pass. Weighted automatic and
explicit-Schwarz saved-output fixtures and transaction rollback pass.

Focused validation of the published pin passes: 40 upstream LSMR tests,
57 prepared OLS tests, 31 shared PPML tests, 11 independent prepared-fit oracle
tests, 31 allocation-contract tests and the exact dependency-pin test.
Both macOS slices build and pass binary/native ABI audits; clippy, formatting
and diff checks pass. Allocation inventory v17 changes only the reviewed
Cargo.lock and core dependency wiring; all source allocation entries and
hard boundaries match v16. No unrelated dependency versions changed.

The prepublication receipt records the intentional last-bit changes against
the old dependency and the source-extracted kernel benchmark. Seven-thread
median paired ratios were 0.986 for the norm and 0.992 for the dot; serial
ratios were 1.011 and 1.013. Those component measurements were not rerun for
pin integration and are not command-speed evidence. Failed baseline captures
remain failed evidence. The frozen F0 exact-extraction policy is unchanged.
No RSS, F0, SCC, Windows or release qualification was run.

This integration is complete. The separate explicit-diagonal LSMR certificate
failure and remaining performance milestones stay open.

## Prior certified LSMR/CG report aggregation

The next tracker #26 correctness issue is fixed under
[ADR 0033](docs/adr/0033-certified-lsmr-cg-batch-reporting.md), with the
[source/artifact-bound receipt](benchmarks/reports/development/ols-mixed-report-local-2026-09-10.json).
The baseline is the September 9 retained-LSMR candidate below. Only
`ols/prepared_fit.rs` changes in production relative to that baseline; all
work remains uncommitted on main.

Low-memory automatic fits can combine a certified LSMR batch and a certified
CG continuation when their requested/effective LSMR preconditioners agree
and the CG report carries its fallback flag. The aggregate reports requested
`auto`, used `cg`, and fallback true in either batch order. Maximum iterations
and certificate norms, summed timing/retry fields and ordered warnings retain
their prior conventions. Failed certificates, incompatible requests or
preconditioners, unflagged CG and other mixed engines remain rejected.
Solver choice, numerical work, tolerances, budgets, memory lifetimes and the
public result wire are unchanged.

The original weighted automatic fixture now passes against Stata explicit
dummies, including covariance, fitted values, residuals, saved-FE normalization
and reconstruction at pool sizes 1/5/10. Its data, commands and numerical
assertions are unchanged; only the expected aggregate solver metadata and
terminal name were updated. The same baseline still fails with `r(459)`.
The N100,000 three-FE low-memory serial reproduction now passes complete pinned
reghdfe comparison and exact self-repetition; maximum coefficient difference
from the reference is below 2.3e-15. The seven-thread reproduction gets past
the report error but fails exact repetition on coefficients at about 2.5e-17.
That capture remains failed evidence, separate from the fixed report error.

All six previously successful controls retain exact scientific equality,
including low/fast memory, pool sizes, one/seven threads and probability
weights. Focused checks pass: 57 prepared OLS tests, 31 shared PPML tests,
11 independent prepared-fit oracle tests, 31 allocation-contract tests,
clippy, formatting and licensed transaction rollback. The new report test
fails before the fix and passes after it for all six batch orders; rejection
tests preserve invalid-input atomicity. Both macOS plugin slices build with
Rust 1.85.1 and pass binary/native ABI audits. Inventory v16 changes only the
reviewed production tokens; allocation sites and hard boundaries match v15.

This is a correctness fix with no performance claim. Peak RSS, F0, SCC,
Windows and release qualification were not run; nothing was pushed or installed.
The next correctness work is parallel exact-repeat behavior and the separate
explicit-diagonal LSMR certificate failures. Broader M1a setup lifetimes and
other performance milestones remain open.

## Prior low-memory LSMR preparation reuse

The bounded categorical projection portion of tracker #26 M1a is implemented
under [ADR 0032](docs/adr/0032-low-ols-lsmr-preparation-reuse.md), with the
[source/artifact-bound receipt](benchmarks/reports/development/ols-lsmr-reuse-local-2026-09-09.json).
The baseline is the prior wide raw-moment candidate below. Three production
files change relative to that baseline: the borrowed FE adapter, its private
module export and prepared OLS integration. All work remains uncommitted on main.

Eligible low-memory categorical fits lazily prepare one LSMR operator and
preconditioner for the response and all regressor projection batches. The
sample, finalized FE design, weights and solver configuration remain fixed.
Weighted preparation uses one fallibly reserved owned weight copy. Every batch
retains its original RHS scale, iteration budget and independent certificate.
The operator is released before QR/VCE/output recovery and before any CG
continuation; after a continuation, subsequent batches use the existing
one-shot route. Fast/generic/sloped/output paths and report-merging policy are
unchanged. This does not complete all of M1a.

Five alternating untraced pairs per target, N=30,000, three categorical FEs,
29 controls, one native thread, low memory:

| Pool size | Baseline median seconds | Candidate median seconds | Median paired reduction |
|---|---:|---:|---:|
| 10 | 0.459 | 0.445 | 3.6% |
| 2 | 0.519 | 0.460 | 11.5% |

Reductions use paired ratios, not ratios of displayed medians. Every pair
matches scientific fields exactly and passes the complete pinned reghdfe
comparison. Five-pair fast controls have median candidate/baseline ratios
1.043 for the same three-FE design and 1.025 for one FE; individual timings are
noisy. Separate traces show projection setup falling from 18.0 to 6.25 ms at
pool 10 and from 63.5 to 6.44 ms at pool 2. Successful operator constructions
fall from four/sixteen to one per projection; the separate final-residual
LSMR call remains one-shot. Independent operator-application counts and RSS
were not measured.

All nine diagnostic cases pass exact baseline/candidate and self-repetition
checks plus complete pinned references, including analytic/probability
weights, seven threads, K10 and pool-size controls. Focused checks pass: 30
borrowed FE tests, 55 prepared OLS tests, 31 shared PPML tests, 22 independent
Rust oracle tests, 31 allocation-contract tests, nine harness tests, clippy
and formatting. The explicit Schwarz-LSMR licensed fixture checks weighted
coefficients, covariance, residuals, saved FE normalization and reconstruction
against Stata explicit dummies at pool sizes 1/5/10; transaction rollback also
passes. Both macOS plugin slices build with Rust 1.85.1 and pass binary/native
ABI audits. Inventory v15 records the reviewed allocations and preserves the
hard one-shot/transaction boundaries.

The five older N100,000 multi-FE failures reproduce on both the source-bound
baseline and final candidate. Three seven-thread fast cases fail exact
repetition; two low-memory cases fail with inconsistent solver reports. A
fresh trace localizes the low-mode failure: the response certifies with LSMR,
a regressor batch fails the tighter LSMR certificate and continues to certified
CG, and the existing report merger rejects the mixed engines. No tolerance or
report-policy change is retained. The initial constant-tail hypothesis did
not explain that failure, and its speculative report-merging edit was removed
before the final rebuild and measurements.

The weighted output fixture's automatic and explicit-diagonal variants also
fail on both baseline and candidate; the tighter explicit-diagonal attempt
fails too. These are preserved as failed evidence, separate from the passing
Schwarz fixture. Report aggregation is addressed by the current work above; the parallel
exact-repeat and explicit-diagonal certificate failures remain open.
Remaining performance work includes fast/generic/output setup lifetimes,
redundant RHS transport and measured serial VCE work. No F0, SCC, Windows,
release qualification, push or installed-plugin replacement was performed.

## Prior wide raw-moment scans

Tracker #26's next selected optimization is implemented under
[ADR 0031](docs/adr/0031-wide-raw-moment-column-blocks.md), with the
[source/artifact-bound receipt](benchmarks/reports/development/ols-raw-moments-local-2026-09-09.json).
The baseline is the guarded wide-solve candidate below, including the earlier
within-norm optimization. Production changes only `ols/prepared_fit.rs` relative
to that baseline; the work remains uncommitted on main.

Wide raw-moment caches scan eight adjacent columns per block, sharing matrix
loads and weight reads. Each column retains the prior arithmetic and row order
for maxima, scaled norms and compensated means. Blocking requires enough
blocks for the worker team; raw matrices at most 64 columns retain their
existing fused algorithm. There is no new heap allocation. The five fixed-size
accumulator arrays use 320 bytes per active block; inventory v14 preserves
allocation sites and the hard estimation/transaction boundaries.

Five alternating pairs per target, N=100,000, one FE, fast memory:

| Controls | Native threads | Baseline median seconds | Candidate median seconds | Median paired reduction |
|---|---:|---:|---:|---:|
| 64 | 1 | 0.437 | 0.352 | 18.8% |
| 64 | 7 | 0.186 | 0.178 | 4.3% |
| 65 | 1 | 0.464 | 0.371 | 20.4% |
| 65 | 7 | 0.228 | 0.185 | 12.6% |

Reductions use the median of paired ratios, not the ratio of the displayed
medians. Every pair matches all scientific fields exactly and passes its
complete pinned reference comparison. Five-pair unchanged K50 controls have
median ratios 1.000 and 1.00694 at one/seven threads. Separate K65 traces put
raw moments at 0.157 to 0.0643 seconds with one thread and 0.0297 to 0.00967
seconds with seven threads. Their phase timings are not paired speed claims.

All 16 additional diagnostic scopes reproduce the source-bound baseline
exactly and pass complete pinned reghdfe comparisons, including low memory,
K128, no FE, analytic/frequency/probability weights, exact collinearity and
absorbed slopes. The independent K65 group-elimination/SVD oracle passes;
coefficient error is below 1.2e-15. Licensed fixtures pass wide weighted saved
outputs against explicit dummies, PPML against Stata Poisson, fast/low parity
and transaction rollback. Focused checks pass: 54 prepared-OLS tests, 335 PPML
tests, 11 independent prepared-fit oracle tests, 31 allocation-contract tests,
seven diagnostic-harness tests, clippy and formatting. Both macOS plugin
slices build with Rust 1.85.1 and pass the binary/native ABI audits.

This is focused local macOS development. Peak RSS and independent operator
counts were not measured; no F0 matrix, SCC, Windows or release qualification,
push or installed-plugin replacement was performed. Earlier multi-FE repeat
failures and the separately recorded repository-checker failures remain open.
Retained LSMR setup reuse against this improved baseline is addressed by the
current work above.

## Prior guarded wide OLS solve

Johannes approved extending the guarded finite solve above 64 candidates.
[ADR 0030](docs/adr/0030-wide-guarded-ols-finite-solve.md) and the
[source/artifact-bound development receipt](benchmarks/reports/development/ols-wide-cholesky-local-2026-09-09.json)
record the implemented change. The baseline is the prior norm optimization
below; its plugin is unchanged from the earlier width receipt.

Wide fast fits now use disjoint Gram-row partitions and attempt the existing
guarded Cholesky solve. Weak/rank-deficient pivots and failed inverse
certificates retain QR; fresh score, FE, bread and reconstruction checks keep
their existing bounds. Wide moment construction needs no partial bank. Fast
preflight charges an additional `32*K^2 + 128*K` bytes for the live Cholesky
workspace, independent of thread count. At-most-64 arithmetic and low-memory
QR remain unchanged.

Five alternating warm pairs per target, N=100,000, one FE, K=65, fast memory:

| Native threads | Baseline median seconds | Candidate median seconds | Median paired reduction |
|---|---:|---:|---:|
| 1 | 6.965 | 0.429 | 93.8% |
| 7 | 2.447 | 0.207 | 91.6% |

Reductions use paired ratios. Five-pair K64 controls show no material
regression and exact science. A separate one-thread trace puts the finite
solve at 6.708 seconds before and 0.0575 seconds after. FE/pilot iterations,
solver choices and fallback reporting are unchanged in every accepted
comparison; independent operator-call counts were not collected.

All 16 diagnostic scopes have complete pinned reghdfe comparisons. They cover
K64/K65 at N100,000 plus K128, no FE, low memory, analytic/frequency/probability
weights, exact collinearity and absorbed slopes at N10,000. K64 and low-mode
science remain exact. K65 paired coefficient differences are below 2.8e-15;
all changed numerical results pass the registered OLS envelopes and unchanged
absolute certificates. Six independent group-elimination/SVD oracles pass.
The development comparator records covariance determinant and pilot
contraction as numerical diagnostics while keeping policy decisions exact;
the original F0 evaluator and its `extraction_exact` policy are unchanged.

Final-source checks: 52 prepared-OLS, 21 finite-kernel, 49 memory, 31 PPML,
40 plugin-estimation and 22 independent Rust oracle tests pass, as do 31
allocation-contract tests, seven base-harness tests and seven final wide
comparator tests. Wide unit cases include weighted full rank, near deficiency,
exact deficiency, thread determinism, overflow and memory preflight. Licensed
fixtures verify wide weighted saved outputs against explicit-dummy regression,
wide PPML against Stata Poisson and fast/low results, and transaction rollback.
Both macOS plugin slices build with Rust 1.85.1; binary and native ABI audits
pass. Inventory v13 records three reviewed production files with unchanged
allocation-site inventories and hard transaction/one-shot boundaries.

Limits: local Apple Silicon/Stata 19 development only; no F0 screen, SCC,
qualification, Windows execution, release, push or installed-plugin
replacement. Peak RSS was not measured. The K128/weighted timings are unpaired
diagnostics. The next measured bottleneck is the existing wide raw-moment
scan: about 0.153 seconds of a 0.319-second prepared fit at one thread/K65.
That bottleneck is addressed by the current work above. Earlier multi-FE
exact-repeat failures remain open.

The broad repository checker still fails on five obsolete STATUS headings
already absent at `c975bc5` and an ignored captured source snapshot counted as
a second status document. [The bounded check](benchmarks/reports/development/ols-wide-checker-preexisting-2026-09-09.json)
records these pre-existing findings; the checker and historical snapshot were
left unchanged. Focused scientific, build and allocation checks pass.

## Prior arithmetic-preserving OLS performance

Local development result from clean `main` at
`c975bc5c6e2aa5ff4be159225447d31f0fe20de6`:
[source/artifact-bound receipt](benchmarks/reports/development/ols-arithmetic-local-2026-09-08.json).
The retained source overlay changes only `ols/prepared_fit.rs` in production.

Wide parallel within-norm scans now process eight adjacent columns with
worker-local stack accumulators. Each column retains the scalar scaled-sum
arithmetic and row order. Blocking requires enough blocks for the existing
worker team; serial and narrow parallel paths retain their prior scans.
There is no new heap allocation, solver policy, tolerance, protocol or public
syntax change. The one-FE wide norm phase fell from about 10.2 ms to 3.7 ms
in separate traced runs.

Five alternating baseline/candidate warm pairs per target, N=100,000, K=50,
fast memory and seven native threads:

| Target | Baseline median seconds | Candidate median seconds | Median paired improvement |
|---|---:|---:|---:|
| No FE | 0.167 | 0.157 | 6.5% |
| One FE | 0.141 | 0.128 | 9.2% |

The improvement column uses the median of paired ratios, not the ratio of
the two displayed medians. All target pairs match science exactly and have
complete pinned reghdfe comparisons plus an independent group-elimination/SVD
oracle. Coefficient differences from that oracle are below 5e-16. Five-pair
narrow guards and three serial controls show no median regression above 5%.
Earlier width experiments, regressing narrow dispatch, and builds with invalid
source/artifact binding are preserved as excluded or rejected evidence.

The separate 38-scope diagnostic has 26 accepted baseline scopes and 25
accepted candidate scopes; all 28 complete baseline/candidate captures match
exactly. Several multi-FE scopes fail exact self-repetition, including an
additional candidate scope with a prior baseline trace failure. Four original
reference attempts lack complete accepted records. These remain unavailable
evidence. A supplemental wide one-FE guard matches saved FE and residual
outputs exactly and has a complete pinned comparator; it does not replace the
original unavailable weak-two-FE saved-output comparison.

Final-source checks pass: 51 prepared-OLS tests, 22 independent oracle tests,
40 plugin estimation tests, 31 shared PPML tests, 31 allocation-contract tests,
six diagnostic-harness tests, formatting and clippy. Five licensed Stata
fixtures cover transaction rollback, fast/low memory, constrained preflight,
PPML auto/explicit and saved-output behavior. Both universal-plugin slices
were built in an isolated target directory; binary and actual ABI checks pass.
Allocation-source inventory v12 changes only the reviewed production tokens
and error literal in `prepared_fit.rs`; allocation sites and hard F0 seams are
unchanged. No scientific baseline/candidate differences were accepted.

Supplemental K=64/65 checks are recorded in
[the width receipt](benchmarks/reports/development/ols-width-local-2026-09-08.json).
The current production files and both plugin hashes match the original
source-bound artifacts. All six one-FE scope pairs (one/seven threads, fast
K64/K65 and low K65) match science exactly and pass the complete pinned
reference comparison. Three independent SVD oracles pass, with coefficient
differences below 2.5e-15; seven harness tests pass. The diagnostic runner now
accepts a separate manifest and generates its requested control width without
altering the original manifest or receipt.

The earlier width receipt established the old K64/K65 discontinuity and exact
science for the norm optimization. Its timings are sequential diagnostic
captures. The subsequent approved wide-solve implementation and new evidence
are recorded above; neither historical receipt was rewritten.

## PPML auto admitted for development

ADR 0029 now admits the implemented automatic policy:

- Exactly two or three categorical intercept FE dimensions select LSMR when
  fast execution and the complete ordinary/checkpoint workspace fit the
  generation-zero memory plan.
- Other designs, absorbed slopes, low-memory execution, allocation audit, or
  modeled specialized-workspace veto select MAP. Auto never selects CMG.
- A modeled veto occurs before solver construction and is not a fallback.
  Actual allocation/interruption/data/internal errors remain fatal.
- Requested, selected, concrete, effective, checkpoint, retry and fallback
  reporting is preserved. Auto uses the same certified engine as explicit LSMR.

Admission: benchmarks/reports/development/ppml-auto-development-admission-2026-09-08.json.
This is development acceptance, not formal qualification, a successful
reclassification of the failed calibration campaigns, or a claim that every
scope was rerun on the repaired source.

## Scientific adjudication

All21 hard actual-auto scopes on9 original datasets have independent
coefficient binding, model-space, objective, finite/per-FE score, rank/DoF,
covariance and default/tighter stability evidence.

- 18 scopes pass the unchanged numerical comparisons using complete pinned
 reference records. Earlier complete rows in failed campaigns are prospective
 scientific witnesses only, not accepted timings or campaign passes.
- Task17 has a fresh optimum-accuracy proof on the repaired Linux capture:
 coefficient radius4.882e-12 and covariance radius1.762e-12. All5 complete
 reference repetitions have17 coefficients and50 covariance cells proven
 outside their existing envelopes; other compared science passes.
- Tasks7/8 share the many-level N75000/K50 dataset with no complete reference
 row. Both pass independent optimum inclusion. The task7 coefficient and
 covariance bounds are6.345e-12 and1.793e-12. Reference error is not claimed.

The apparent four additional VCE discrepancies were truncated-record artifacts.
The new strict reference parser rejects incomplete matrices/records, invalid
stripes, unsuccessful fits and invalid objective pairs. It does not treat
missing evidence as a numerical mismatch. ADR0026 finite/domain/Poisson-identity
and no-worse objective rules apply without weakening coefficients
(rtol1e-7/atol1e-10) or VCE (rtol1e-6/atol1e-10).

All9 preserved SCC dataset hashes were reverified read-only; the reference
closure matches original setup. Original input/source and audit bindings are
in ppml-auto-adjudication-completion-2026-09-08/work/aggregate-bindings.json.
No projected reference state is used as an additional objective witness.

## Common checkpoint-cycle repair

The Linux K0 N75000 many-level two-FE tighter fit cycled: a certified warm
checkpoint could reconstruct the older canonical predictor, moving eta by
1.599e-10 against the requested1e-10 profile gate despite a passing score.

A fit-owned checkpoint target now tightens100x after a certified unchanged-basis
profile rejection, bounded below by1e-14 without loosening an already-tighter
target. The refined target also governs final-Hessian synchronization. The
next actual checkpoint counts the extra work as a retry. Existing RAII,
warm-state invalidation, two consecutive deviance-plus-score convergence,
canonical rank and exact final q_i*mu_i certificates remain mandatory.

The formerly300-second timeout now takes1.873seconds at both default and
tighter outer tolerance. Strict/final eta movements are8.42e-12/7.29e-16.
Default b/V/objective/rank/DoF are unchanged. Repaired difficult2FE and task17
default results are identical to baseline; sparse3FE b/V differences are
1.05e-16/3.69e-19, with unchanged objective/rank/DoF and passing certificates.
Different hosts prohibit matched performance claims for those guard timings.

## Source, jobs and artifacts

Baseline main commit:7c1fc09207864b41f851c0cb58c2dcbe80b1e8be.
Frozen dirty snapshot:da29adb7f6061a933dccbedbae923d96957361f3aa495f75780be8a05755362e.
The sole subsequent compiled-production overlay is
crates/fereg-core/src/ppml/prepared_irls.rs SHA
977522f1b878c2f96b5147f85fd927e739cb65b5111a9062f25d5f3ff50d3c93.
Documentation, adjudicator and fixture changes are separately identified.

- 7490248: actual-auto Linux smoke, task17, pass.
- 7490294/7490295: baseline actual-auto arrays;16 successful scope receipts.
- 7490361: independent direct-ring reaudits repair auditor failures4/7/8;
 original failures remain recorded.
- 7490499: trace-only diagnosis, no numerical policy change.
- 7490653: repaired K0 task3, successful fits and independent audit.
- 7490705: repaired guard scopes1/11/17; all scheduler/application/audit passes.
- 7490776.1–4: strict reference attempts all timeout1200seconds,
 failed0/exit1, application124, no fit sentinel. None accepted.

Every substantive job reserved28cores. No job is currently pending for this
goal and the completed campaign monitors were deleted.

Baseline Linux plugin:12f17d5873a0092e2ac6baee6b3ced2732504550c2983e5ada4a2d1a07f3450a.
Repaired Linux plugin:01af450c8fa6d841584661608fa45db41b0f9afb14cd7750eeaa9a3f51bc5124.
Repaired universal Mac plugin:47e578679f81ba9f0ffbb087c94864019427fa15c1f1ecac7ecc3bac5791caee.
The isolated Mac build used installed target libraries; only unavailable rustup
provisioning was omitted. Standard compilation, binary audit and actual ABI
loader tests passed. Formal exact-source qualification remains separate.

## Validation and handoff

- 745 repaired-core tests;18 focused strict-checkpoint tests;2 constrained-memory
 tests; formatting and core all-target Clippy passed.
- 71 focused auto Python tests,10 accuracy tests,8 optimum-adjudication tests,
 16 PPML help-contract tests,31 allocation-source-contract tests and10 build-
 closure tests passed. Existing independent oracle/memory evidence is retained.
- 12 licensed Mac auto-versus-explicit route pairs pass with exact b/V/objective/
 rank/DoF/sample/mu agreement. Low memory selects MAP; failed auto restores
 prior results. An additional3FE pair exactly matches saved effects and eta/d.
- Terminal Stata sentinel and explicit application returncode0 were checked.
- Git diff whitespace validation passes.

The full evidence chain is listed in the development admission receipt.
The admitted universal artifact is installed in dist and passes the same
fixture in a fresh Stata process, including its explicit terminal sentinel.
Activation receipt: ppml-auto-local-activation-2026-09-08.json.
Final whitespace-only cleanup of the independent coefficient-proof script
preserves its Python AST and nonwhitespace bytes exactly;10 accuracy tests were
rerun. The original executed source is preserved and historical proof hashes
are not rebound to the cleaned file.
Historical optimization evidence remains in its named immutable reports;
nothing is promoted across source revisions. The coherent development commit
includes this handover and the source-bound admission/activation records.

## Remaining performance work (not part of this admission)

- Sparse/mixed3FE ordinary joint LSMR conditioning and operator work remain
 high-leverage; do not restore the rejected retained all-pairs Schwarz policy.
- CMG keeps its bounded nuisance envelope and fresh certificates. Guarded
 weak-edge grouping, graph batches and certified warm starts are retained;
 unconditional grouping and ineffective response-start experiments are not.
- Explicit MAP is still slow on poorly connected graphs. Auto now avoids it
 within its admitted two/three-effect envelope.
- Easy one-FE wide-K weighted-design passes remain a useful optimization target.
- No further upstream API expansion, auto-policy refit or new campaign is implied.

## Release boundary

Development acceptance does not close the beta's formal exact-source
scientific, memory, performance or cross-platform release gates. Windows and
the formal aggregate remain deferred. No push, tag, release or broader
qualification is authorized by this completed development admission.
