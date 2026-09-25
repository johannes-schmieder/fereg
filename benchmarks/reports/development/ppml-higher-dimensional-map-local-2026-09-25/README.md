# Fast higher-dimensional PPML: coefficient and joint engine admission

Date: 2026-09-25. Retain **higherdim1** under
[ADR 0203](../../../../docs/adr/0203-higher-dimensional-fast-ppml.md).
This is bounded local development evidence, not qualification or completion of
the multi-FE <=0.90-stock objective.

## Source and matched work

Candidate: **d46b584e plus two overlays**, the admission condition and focused
Rust test in prepared_irls.rs, and the new licensed higher-dimensional oracle.
All **565 captured computational inputs** match the accepted source. The plugin
SHA-256 is `92df56c44f5c40aabc4c297e3d2bbc863d528f0fe3f0ce395c7a7fb6723e50a2`.
Baseline: **c7c4c473 / controlplane3**, native slopetopology1, 564 inputs, plugin
`1c598ce6ecbdea8e1a6e0ffcc9dfcd655c89165750a8d2b02d8189ec15b9ae1e`.
Full source snapshots, identities, build/ABI evidence and candidate.patch are
archived. Later documentation is outside the captured computational inputs.

Reference: unmodified pinned fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Headline Poisson calls use glm.tol=1e-10 and fixef.tol=1e-11, matched threads,
robust covariance with the documented matched correction and fitted means.
Public Stata19 clocks include parsing, native work, scientific acceptance,
inference and requested publication. Startup/import are outside the clocks;
each call has a preceding warm-up. Independent sample checks agree. N in the
tables is the generated size, before the common singleton sample policy.

Inventory and pilot use reused development seed **202609347**, three rotations.
Fresh validation uses predeclared, previously unused **202609435**, five rotations,
with the candidate frozen before declaration. That seed is now consumed; no
production tuning follows its results. OLS controls use the same frozen source
and three rotations. All 228 attempts have clean sampled contention monitors.
No builds, tests, profiles, audits or archiving accompany headline clocks.

The isolated additive higher-FE harness extends the existing three-FE generator
with declared extra group levels, imbalance or disconnected components. Six
small harness checks verify unchanged outcome/control values and row order,
repeated positive groups and component isolation. Generator code, declarations
and data hashes are captured. Frozen 77/16/111 definitions are unchanged.

## Opportunity and implementation

The initial source inventory showed three-FE fast/low PPML competitive at
87/234 and 86/232 ms, four-FE low at 151/323, four-FE slopes at 199/738, and
wide K72 low two-FE at 262/421. **Four-FE fast lost 9965/316 ms.** Those are
baseline/stock medians, each scientifically comparable in this inventory.

Fast categorical checkpoint engines cover at most three FE partitions. With
four-plus FE, the selected MAP path previously used generic observation-space
projection of RHS banks and ordinary finite products. Low storage already
admitted the coefficient/joint owner for this design. The existing slope engine
also already supported more than three partitions. The accepted change extends
that complete admission to fast categorical MAP with more than three effects
and finite controls. It adds no solver layer, public option or tuning threshold.

Canonical projection now uses the existing coefficient Schur/CG engine with
current-weight group geometry. Ordinary steps use joint finite/FE directions,
avoiding a complete within-design/inverse construction at each step. Initial
canonical rank, strict Newton checks and independent final construction retain
their duties. Current-weight projection, finite/FE score, model-space, finite
solve, reconstruction, objective and exact final q*mu Hessian checks remain.
Changing weights rebuild numerical geometry; previous solutions are only guesses.
No scientific threshold, sample, weight, DoF or failure policy changes.

The first fresh dense call records projection 8.1663→0.0548 s and strict/final
6.7976→0.0555 s. Preparation is 0.0128→0.0149 s and inference
0.00053→0.00055 s: the gain is in solving, not an omitted output. Outer steps
change 7→4 and reported inner iterations 2380→148, with strict rejections 1→0.
Generic sweeps and coefficient Krylov iterations are different units; these
counters are descriptive, not a comparable operation count. The large unbalanced
case keeps four outer steps while projection drops 0.7203→0.0423 s and strict/final
0.3106→0.0331 s. These individual phase observations are not additive median
attributions or separate headline profiles.

## Pilot and fresh results

Pilot B/C/stock medians in ms:

| Case; N/K/T; mode | B / C / stock | C/stock |
|---|---:|---:|
|Four categorical FE; 30720/6/4; fast|9903 / 128 / 320|0.400|
|Same, low excluded control|154 / 153 / 320|0.478|
|Three categorical FE; 30720/6/4; fast excluded|89 / 89 / 234|0.380|
|Four FE, two slope partitions; 30720/6/4; fast excluded|203 / 203 / 736|0.276|
|Four categorical FE; 61440/10/8; fast|24243 / 272 / 660|0.412|
|Narrow two-FE; 24960/2/4; fast excluded|36 / 38 / 32|1.188|

The larger pilot candidate vector is **262,725,272 ms**. Preserve the 725-ms
outlier; it exceeds stock in that pair. Its estimator elapsed field is
0.255440167 s, with the same iteration counts and no strict rejection. Much of
its delay is outside that reported interval; the cause is not established.
No call is discarded and the median does not describe every pair.

Fresh results, five rotations:

| Case; N/K/T; mode | B / C / stock ms | C/stock | Interpretation |
|---|---:|---:|---|
|Four dense FE; 38400/6/4; fast|15029 / 169 / 435|0.389|All five pairs clear 0.90|
|Four sparse FE; 23040/2/1; fast|failed / 394 / 10805|0.036|All five pairs clear 0.90; no baseline clock|
|Five unbalanced FE; 38400/8/4; fast|815 / 112 / 250|0.448|All five pairs clear 0.90|
|Five disconnected FE; 30720/6/8; fast|timeout / 1444 / 26911|unranked|Fitted-mean discrepancy|
|Four unbalanced FE; 61440/10/8; fast|1100 / 154 / 205|0.751|All five pairs clear 0.90|
|Four dense FE; 38400/6/4; low excluded|199 / 202 / 434|0.465|Three-ms median loss to baseline|
|Three dense FE; 38400/6/4; fast excluded|111 / 111 / 327|0.339|Unchanged median|
|Two slope partitions; 30720/6/8; low excluded|71 / 71 / 80|0.888|One pair misses 0.90; limited headroom|

For the three newly routed fresh cases with valid baseline clocks, median
speedups are 7.1–88.9 times. The fourth ranked new case has five baseline warm-up
failures, r(459), and no valid baseline fit timing. All five disconnected baseline
attempts reach the 90-second command timeout. Candidate and stock applications
complete in all fresh attempts; failures are neither assigned synthetic timings
nor removed from denominators. Seven fresh medians are scientifically rankable
and below 0.90, but the slope control is provisional and coverage is bounded.

## Scientific adjudication and controls

Twelve distinct focused Rust tests pass, including analytic weighted Poisson
truth for four/five FE, nested effects and zero controls, reordered partitions,
T1/T4, fast/low, budget refusal, warm-start refusal, generation capacity/rollback,
explicit-family admission and interruption atomicity. Joint-direction tests use
independent weighted mathematical oracles. Existing thresholds remain unchanged.

Eight licensed scientific fixtures pass: the new higher-dimensional oracle,
iterative canonical and two-/three-partition slope oracles, PPML scientific
contract, weight/inference, separation/outputs, and an additional disconnected
extreme-mass/generated-factor boundary. The permanent new fixture has 24 fits:
four/five FE, unit/FW/PW, fast/low, robust/cluster, with analytic means/coefficients
and an explicit-dummy q*mu Hessian and sandwich. FW multiplicity and PW squared
scores are distinct. The extra boundary uses component predictors near 30 and 1,
PW and i.f controls, checking every mean against the existing scientific envelope.

Across inventory/pilot/fresh, **200 successful public states pass** independent
score, span and objective audits. The ten failed baseline states remain failures.
There are **114 passing baseline-anchored pairs** and twenty unavailable/failed
pairs from those baseline failures. Direct C/stock comparisons pass 53/58: all
18 pilot and 35/40 fresh pairs. The five exceptions are the disconnected case.

There, coefficients, covariance, likelihood and deviance agree; 475 means fail
at rtol=1e-7/atol=1e-9, maximum absolute error 1.4443e-5 and tolerance ratio
47.24. Both states pass their independent audit: candidate/stock score maxima
5.16e-12/2.83e-10, with feasible predictor spans. A separate, unmodified-stock
T8 diagnostic at glm.tol=2.3e-13/fixef.tol=2.3e-12 reduces the discrepancy to
fifteen means (maximum 2.2057e-6; ratio 7.18), while finite coefficients and
covariance still agree. Its score is 4.35e-11. This supports sensitivity to stock
stopping, but neither resolves the remaining error nor ranks the original clocks.
The tighter diagnostic is not substituted into the headline distribution.

Initial diagnostic requests below fixest's permitted tolerance floors fail
before estimation and are preserved. The first also requested T8 while its OMP
limit permitted only T1; the valid diagnostic explicitly uses T8. Two target-only
licensed fixtures initially have mismatched sentinel names: Stata assertions and
return codes pass, but harness acceptance fails. Corrected-sentinel runs pass;
original failures remain. New Rust test development also preserves two compile
errors and a budget-fallback test's unjustified 1e-10 eta assertion failure
(1.98e-10 observed); only that new assertion is changed to the existing published-
mean envelope. No runtime or established test tolerance is loosened.

All nine OLS pairs have exact scientific exports. B/C medians are **10/10**,
**62/62** and **113/115 ms** for small, three-FE and slope controls. The slope
control loses two ms in every pair; unchanged science does not erase this loss.
Existing larger OLS losses are not claimed repaired. PPML exclusions likewise
retain all timing losses in the vectors below.

## Memory and remaining work

All newly eligible storage uses the established complete admission. Budget
refusal retains generic MAP. Successful matched fast cases add **9.58–23.78 MB**
of tracked native peak and **10.99–24.62 MB** of selected modeled storage across
pilot/fresh; scratch is unchanged. Fresh sparse/disconnected candidate peaks are
15.84/32.56 MB, but failed baselines give no paired peak comparison. Excluded
tracked peaks are unchanged; selected telemetry occasionally differs by 48 bytes.
This is a deliberate fast-mode tradeoff, not evidence about physical caps.

Two production-plugin allocation audits, fixed T1/T4 and pool width two, each
pass sixteen warmed iterations with zero events/current growth, stable capacities
and fresh/warm equivalence. The static allocation inventory's documented drift,
weighted constrained-fit and nonretaining-CG span failures remain unresolved.
No SCC retry, Linux T1/T8/T16, physical-memory cap, cold-start campaign, push,
release, stopped campaign or qualification run is part of this milestone.

The next substantial target is exact-weight initial/final canonical work on
weak/disconnected two-way designs, using a bounded sparse-factor feasibility
comparison against actual initial/ordinary/strict frames before integration.
The prior low categorical 21.7-ms deficit needs current-source remeasurement;
this report does not transfer it to higherdim1. Narrow PPML still needs 9.2 ms
against this pilot's 0.90-stock ceiling, and its earlier small inner-recurrence
budget cannot explain away the whole gap. Do not resume packaging, topology
transfer or the rejected incidence/coarse-CG probes. Keep the unresolved five-FE
means and the OLS slope timing concern as explicit controls.

## Immutable receipts

The content-addressed archive contains **7,092 logical receipts**, including both
source snapshots, exact identities, declarations, additive harness, build/test
logs, fixture protocols, failures, per-call results, distributions, audits and
summary. The restore script verifies each hash. Original and sanitized hashes
are retained; licensed logs are reduced to protocols, local home/temporary paths
are redacted, and data/binaries are omitted with hashes. The preliminary archive
before temporary-path redaction remains local and is not the accepted bundle.

## Complete timing vectors

Values are ms in repetition order. A dash is a failed application or timeout,
not a missing observation dropped from the distribution. Case order matches
the declarations and tables above; the inventory order is its six listed cases.

### Inventory

| Case | Baseline | Candidate | Stock | Paired B/C W/L/T |
|---|---|---|---|---|
|0|87,87,85|—|239,232,234|0/0/0|
|1|86,85,88|—|233,232,232|0/0/0|
|2|9968,9748,9965|—|315,316,318|0/0/0|
|3|151,151,152|—|323,319,323|0/0/0|
|4|199,198,200|—|739,738,737|0/0/0|
|5|262,267,261|—|419,430,421|0/0/0|

### Pilot

| Case | Baseline | Candidate | Stock | Paired B/C W/L/T |
|---|---|---|---|---|
|0|9903,10033,9842|128,129,127|320,321,316|3/0/0|
|1|152,156,154|154,153,151|322,319,320|2/1/0|
|2|88,89,91|95,89,88|236,234,227|1/1/1|
|3|202,203,205|203,202,203|732,736,737|2/1/0|
|4|24243,24153,24428|262,725,272|660,660,661|3/0/0|
|5|41,36,36|40,38,37|32,34,31|1/2/0|

### Fresh

| Case | Baseline | Candidate | Stock | Paired B/C W/L/T |
|---|---|---|---|---|
|0|15029,14887,15136,15211,15012|162,166,172,179,169|439,431,430,435,437|5/0/0|
|1|—,—,—,—,—|398,394,391,399,393|10711,10825,10805,10606,10817|0/0/0|
|2|801,800,825,815,816|112,111,114,112,113|250,248,252,251,248|5/0/0|
|3|—,—,—,—,—|1375,1461,1444,1428,1492|26791,26934,26911,26868,27100|0/0/0|
|4|1116,1100,1102,1078,1080|156,153,154,154,153|208,205,221,203,204|5/0/0|
|5|199,199,206,195,204|203,193,200,204,202|434,435,433,432,437|3/2/0|
|6|111,111,111,112,109|111,113,112,111,111|325,327,328,329,326|1/3/1|
|7|71,72,71,71,72|72,71,71,71,71|81,80,79,81,76|2/1/2|

### Ols

| Case | Baseline | Candidate | Stock | Paired B/C W/L/T |
|---|---|---|---|---|
|0|10,11,10|10,11,9|—|1/0/2|
|1|64,62,60|61,65,62|—|1/2/0|
|2|113,112,114|115,114,116|—|0/3/0|
