# Profiled slope Poisson trials — local development, 2026-09-25

Adopt **slopecurve3** under [ADR 0187](../../../../docs/adr/0187-profiled-slope-poisson-trials.md).
A conditional Poisson correction of one absorbed partition removes one outer
iteration on all five fresh two-partition slope designs. Four fresh medians
improve **3.1–4.5%** and the weak-connectivity median ties; **22 wins/three ties**
in 25 affected pairs. Development two-slope fits remove two iterations and
improve 7.4% threaded / 8.3% serial in the retained-artifact confirmation.
Eight of ten fresh PPML medians meet the 0.90-times-stock target. Threaded two-
slope and narrow categorical fits still lose to stock. This is a bounded
algorithmic milestone, not completion of the broader goal or qualification.

## Source and matched comparator

Baseline **slopeinit2** matches HEAD `a3bd6dc6` before this change. Both complete
candidates use that commit plus captured overlays of `ppml/prepared_irls.rs` and
`ppml/profiled_intercept_curve.rs`. All **555 captured inputs** of slopecurve3
match the accepted computational checkpoint. The newly added licensed two-FE
allocation fixture was created after capture and is separately bound by SHA-256
`a34babbe8e675655e846d781d1d9d97ca8cee9679e725683d47db63069a78358`.

| Artifact | Role | Plugin SHA-256 |
|---|---|---|
|slopeinit2|Baseline|2cfb165a645db02ef6fed1a6c787bfbdbee89893757e3d556eedc8514280c79b|
|slopecurve2|Broad multi-FE trials; rejected|c2c9c4c48935ddf597cfc3950d16fd698e19f8212e9d368cb54b401b69a76592|
|slopecurve3|Two-partition trials; adopted|ebfca50ca203293d14e3b710951276b14d4221ff550b610a3c171bb1a7e22fe8|

Both complete release builds pass the actual-plugin ABI and injected-panic
containment checks. A first build invocation accidentally retained only one of
two required overlays; it failed compilation and produced no measured artifact.
Its source hashes, overlay and compiler log remain in the receipts.

Pinned, unmodified fixest **0.14.2**, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
uses `glm.tol=1e-10`, `fixef.tol=1e-11`, matched application threads, robust
inference and fitted means. BLAS is serial. No diagnostic reference contributes
a headline timing, and default-reference discrepancies remain separate evidence.
Native low mode is not a matched physical-memory cap.

Each public warm command runs in a fresh process after matched warm-up, in
rotating baseline/candidate/stock order. Startup, data generation/import and
exports are outside the command clock; inference and means are inside. All
headline calls pass the isolation monitor without competing builds, profiling,
audits, archiving or fits. No frozen benchmark definition changed.

## Mechanism, scientific failure and scope selection

The accepted seed already improves cold starts. This change acts on each ordinary
Newton trial. It extends the existing categorical profiled-trial owner rather
than adding another direction engine. Immutable prior-weight group outcome
moments/scales are prepared once. Current-mean mass, slope moments and profiled
curvature are recomputed for each trial. The exact intercept correction and one
bounded, locally objective-checked Newton proposal per slope remain within the
declared model space; finite coefficients keep their certified interpolation.
Full trial objective/bounds and all current-weight and final checks remain.

The first broad candidate improved the principal two-partition fits but added
1.1–3.4% to three-FE medians without removing an iteration. The retained scope is
exactly two intercept-containing partitions. Three-plus FE keep their previous
path. No row-count, case-name, seed or tolerance threshold was tuned.

An initial strict disconnected unit case at tolerance 1e-12 and component prior-
weight disparity 1e13 failed with persistent predictor disagreement around
1.12e-10 despite a passing score. The optional nonlinear curve now yields
permanently after a rejected strict slope checkpoint, allowing established
canonical refinement/rebuild recovery to proceed. The unchanged tolerance test
passes after that correction, including injected final-product refusals. The
joint-witness fixture explicitly disables the curve to continue exercising the
retained alternative. Failed draft sources and outputs are archived.

## Complete timing distributions

Development seed **202609347**, three rotations per artifact. Fresh seed
**202609422**, five rotations, and OLS seed **202609423** were frozen before
generation. They are consumed validation, not future tuning inputs. Two excluded
control losses receive one five-pair same-input confirmation, without source
changes; byte equality of their datasets is checked. All public states pass
science and are rankable. Values are milliseconds, median [minimum,maximum].
Losses/ties are paired candidate versus native baseline.

### Broad candidate — not adopted

| Case | N/K/T | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses/ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Large three-FE slopes, fast|61440/10/8|186 [182,189]|188 [187,189]|230 [228,234]|1.011|0.817|2/0|
|Same, low|61440/10/8|204 [203,204]|210 [208,212]|232 [228,232]|1.029|0.905|3/0|
|Two slope partitions T8|30720/6/8|88 [88,88]|80 [80,81]|81 [80,81]|0.909|0.988|0/0|
|Smaller three-FE slopes T4|30720/6/4|89 [88,90]|92 [90,92]|118 [118,118]|1.034|0.780|3/0|
|Two slope partitions T1|30720/6/1|143 [140,145]|126 [125,130]|319 [312,320]|0.881|0.395|0/0|
|Narrow categorical|21120/2/4|36 [36,37]|36 [36,37]|31 [30,31]|1.000|1.161|1/1|

### Retained two-partition candidate

| Case | N/K/T | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses/ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Large three-FE slopes, fast|61440/10/8|198 [194,202]|199 [198,204]|237 [234,254]|1.005|0.840|3/0|
|Same, low|61440/10/8|215 [211,218]|213 [213,213]|234 [233,235]|0.991|0.910|1/0|
|Two slope partitions T8|30720/6/8|94 [93,95]|87 [84,87]|78 [77,78]|0.926|1.115|0/0|
|Smaller three-FE slopes T4|30720/6/4|95 [95,97]|93 [93,94]|116 [115,117]|0.979|0.802|0/0|
|Two slope partitions T1|30720/6/1|144 [143,145]|132 [130,133]|322 [318,325]|0.917|0.410|0/0|
|Narrow categorical|21120/2/4|39 [39,39]|38 [38,40]|28 [27,28]|0.974|1.357|1/0|

### Fresh validation

| Case | N/K/T | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses/ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Large three-FE slopes, fast|69120/10/8|217 [207,223]|217 [214,222]|264 [259,270]|1.000|0.822|2/0|
|Same, low|69120/10/8|236 [228,239]|233 [228,249]|262 [260,276]|0.987|0.889|2/0|
|Two slope partitions T8|36480/6/8|99 [96,100]|95 [93,98]|83 [82,86]|0.960|1.145|0/0|
|Smaller three-FE slopes T4|34560/6/4|100 [98,101]|100 [98,101]|131 [129,134]|1.000|0.763|3/0|
|Two slope partitions T1|36480/6/1|154 [150,155]|147 [146,149]|332 [331,334]|0.955|0.443|0/0|
|Narrow categorical|24000/2/4|41 [40,42]|40 [40,41]|33 [32,34]|0.976|1.212|0/2|
|Weak slope barbell|14400/4/4|40 [40,41]|40 [39,40]|1028 [1018,1034]|1.000|0.039|0/3|
|Disconnected slopes, low|24960/8/4|96 [95,98]|93 [91,93]|147 [143,149]|0.969|0.633|0/0|
|Two slopes per partition|28800/10/8|142 [140,143]|137 [135,139]|192 [190,192]|0.965|0.714|0/0|
|Four categorical FE, low|30000/8/4|171 [170,175]|174 [170,180]|494 [488,507]|1.018|0.352|3/0|

### Excluded OLS controls

| Case | N/K/T | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses/ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Small serial two-FE|3840/2/1|49 [48,49]|48 [48,50]|—|0.980|—|1/1|
|Three-FE|30720/8/4|54 [54,55]|56 [53,56]|—|1.037|—|2/0|
|OLS slopes|28800/6/4|100 [100,101]|99 [97,103]|—|0.990|—|1/0|

### Four-FE same-input confirmation

| Case | N/K/T | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses/ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Four categorical FE|30000/8/4|171 [167,174]|170 [169,175]|—|0.994|—|2/0|

### Three-FE OLS same-input confirmation

| Case | N/K/T | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses/ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Three-FE OLS|30720/8/4|52 [51,52]|52 [51,53]|—|1.000|—|2/2|

The initial four-FE control loses 3 ms and three-FE OLS loses 2 ms. Their single
confirmation has four-FE 171/170 ms and OLS 52/52 ms; retain both blocks, including
all paired losses. All 14 OLS scientific pairs are exact. The fresh PPML block
has 35 wins/10 losses/5 ties overall; all ten losses are excluded three-plus-
FE controls, whose computational path and iteration counts are unchanged. No
claim of universal performance non-regression follows from these small blocks.

## Work attribution and remaining gap

Fresh two-partition outer counts fall 5→4 for threaded, serial, weak and
disconnected cases; the multiple-slope case falls 7→6. Corresponding mixed
inner counts are 50→42, 50→42, 81→68, 75→63 and 96→80. These are native accounting
units, not comparable fixest iteration units. Full-command gains are smaller
than iteration reductions because the trial adds fresh nonlinear passes and
canonical preparation/final work remains.

| Fresh threaded two-slope phase | Baseline ms | Candidate ms |
|---|---:|---:|
|Preparation|17.803|17.547|
|FE projection|26.188|21.975|
|Finite solve|8.488|7.520|
|Line search/objective|4.848|6.088|
|Weight/response refresh|2.049|1.468|
|Strict final solve|13.337|13.316|
|Publication|6.280|6.207|
|Inference|0.755|0.592|

The full command is **99/95/83 ms** baseline/candidate/stock. It still needs
**20.3 ms** to reach 0.90 stock. The narrow categorical command is **41/40/33 ms**,
leaving **10.3 ms**. Do not transfer the broad candidate's earlier 80/81-ms ratio
to this fresh artifact/block. The fresh source-bound ratios above supersede no
historical receipt; each remains attached to its own source and inputs.

A separate current-source ADO diagnostic uses unchanged slopeinit2 native code
with six disjoint wrapper timers. All six diagnostic public states pass independent
science. Warm-up/load timers remain separate. Two repetitions give these full-fit
phase medians (millisecond-resolution Stata timers):

| N/K/T; design | Parse/metadata | Shadows/request | Native call | Bounded validation | Output validation | Publication |
|---|---:|---:|---:|---:|---:|---:|
|21120/2/4; categorical|1.5|1.5|27|2|1.5|2|
|3840/2/1; categorical|3|1|8|3.5|1|2|
|30720/6/8; two slopes|2|1|78.5|3|1|2.5|

These diagnostic medians are not exact additive decompositions or headline
stock comparisons. They show no single wrapper phase able to close the narrow
10-ms target gap. Stop isolated metadata tuning here. The next substantial
native target is canonical current-weight construction/reconstruction shared by
initial rank setup and the strict final checkpoint: the threaded slope case
still spends about 17.5 ms in preparation and 13.3 ms in final construction,
beside 22.0 ms in FE projection. Isolate those original-input passes and ownership
before proposing reusable work; changing-weight geometry must remain current.
A cheaper speculative trial alone cannot remove the measured canonical cost.

## Science, ownership and memory

All **274 PPML public states**, **177 baseline comparisons**, **86 direct stock
comparisons**, **14 exact OLS pairs** and **14 licensed fixture executions** pass.
There are 268 headline PPML calls, six separate ADO diagnostic calls and 28 OLS
calls. The licensed fixtures cover explicit weighted intercept/slope Poisson and
robust/cluster Hessian/sandwich oracles, three-partition slopes, canonical product
rejection boundaries, one-FE PPML and three-FE OLS. The new two-partition audit
runs fast/low T1/T4 in separate fixed-configuration processes: all sixteen-
iteration windows have zero events/growth, stable capacities and fresh/warm
agreement. No frequency-weight inference claim relies on ordinary fixest weights.

The final focused Rust run has **56 distinct passes and one known baseline
assertion failure**. Three profiled-curve tests include an independent Newton
formula, log-weight scaling 1e±200, slope scaling 1e±80, constants/duplicates,
thread invariance, group mass and refusal. Prepared-IRLS gives 53 passes and the
previously reproduced dynamic-range assertion (expected one strict rejection,
observed zero); this checkpoint does not fix or hide it.

The new owner is optional and fully charged. It retains N log prior weights,
G log outcome masses, G shifts, G slope steps, 2GS immutable slope moments and
identified finite slots. No bank grows inside ordinary iterations and no changing
Hessian factor is carried across frames. Fresh affected tracked-peak additions
are **139,232–535,088 bytes**, with the principal threaded/serial cases adding
535,088 bytes. Selected-model additions there are 535,336 bytes. Scratch is
unchanged. Excluded three-plus-FE tracked peaks are exact; categorical admission
adds 72 descriptor bytes without a higher measured peak. These are allocator and
model telemetry, not physical-cap/RSS qualification.

Original projection/model-space, rank, finite-solve, score, objective,
reconstruction and exact final-Hessian conditions remain mandatory. The curve
has no certificate authority. Failed admission/construction retains the old
path; rejected strict slope checks revoke the optional correction. Samples,
weights, separation, DoF, inference, tolerances and publication atomicity stay
unchanged. Frozen 77/16/111 definitions, allocation-inventory drift, constrained-
fit/nonretaining-CG failures and deferred platforms remain open. No push, release,
SCC retry, qualification or stopped campaign occurred.

## Immutable receipts

See `receipts-content-addressed.zip`, its SHA-256 sidecar and
`archive-verification.json`. `restore_receipts.py` checks payload hashes and
reconstructs logical receipts. Exact artifact inputs, experiment recipes,
declarations, distributions, source-bound failed drafts, raw protocol fields,
independent scientific audits and fixture sentinels are included. Licensed logs,
binaries and data are omitted with hashes. Machine-specific paths are sanitized,
with original and archived hashes retained. The root is
`target/ppml-ordinary-correction-20260925`; all operations are terminal.
