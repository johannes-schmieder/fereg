# Cooperative compact PPML products — local development, 2026-09-25

This accepts **jointteam1**, the cooperative single-RHS compact operator in
[ADR 0185](../../../../docs/adr/0185-cooperative-compact-ppml-products.md).
It is a bounded improvement, not completion of the multi-FE 0.90-times-stock
objective. Large threaded slope cases improve 4.4–5.6% on fresh inputs without
another solver bank. Four categorical FE in low memory share the operator and
improve about 11%. Serial slopes and an OLS control retain small measured losses.

## Source, comparator and measurement boundary

Baseline **geoteam1** is accepted main `9a297e44`; its plugin SHA-256 is
`149871641cdce3d36e4551c9b1313352821ae8bce51375177e5f89fe4e3f4e7e`.
Candidate **jointteam1** has the same base plus four captured Rust overlays:
`fe/map.rs`, `fe/slope_coefficients.rs`, `fe/slope_coefficients/coupling.rs` and
`fe/slope_coefficients/joint.rs`. Its plugin SHA-256 is
`89b247490669492e928b5cb9fc9f33f3b0b85fef073733624b084f35a84d970b`.
All **554 captured inputs** match the accepted checkout. The new licensed
compact-route fixture was added after artifact capture and is separately bound
by `external-fixture-identity.json` and its archived source. It is not an
uncaptured compiler change. Build, actual ABI and injected-panic checks pass.

Stock is the pinned, unmodified **fixest 0.14.2** archive, SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Comparable Poisson calls use `glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads,
robust inference and fitted means; BLAS uses one thread. Default-reference
accuracy failures from earlier reports remain separate and unranked.

Each public warm command runs in a fresh process after its matched warm-up.
Data generation finishes before fits. Baseline/candidate/stock order rotates;
confirmation uses alternating native pairs. Every call passes the captured
process monitor with no overlaps or monitor errors. No builds, profiling,
archiving, audit calculations or competing fits accompany headline measurements.
Times below include the matched public estimation, inference and requested
output work, not process launch or dataset generation. Diagnostic binaries have
separate identities and their clocks are not stock rankings.

## Computational change and attribution

Ordinary joint directions use one RHS; other admitted column workers are idle.
Compact FE products now distribute eliminated-group C/C-prime contributions
across those workers' existing coefficient and scratch banks, then merge in
worker order. The local Gram and remaining-pair base, rank-aware geometry and
cancellation-sensitive curvature fallback retain their scientific roles. Dense
Schur products and one-worker execution call the existing scalar product.
Canonical column batching and all fresh acceptance checks are unchanged.

Separate ordinary-call diagnostic blocks compare prior **geophases1** from the
[preparation report](../ppml-parallel-slope-preparation-local-2026-09-25/README.md)
and current **jointteamphases1**. The latter plugin is
`14847cb79e2c32d3b86de59b8ee8114aa683c174bb6e069f681187365a754190`.
On development large-fast/low inputs, FE product time falls **17.75/17.89 to
6.03/6.76 ms**, and the whole joint session falls **57.72/56.33 to 45.80/45.35 ms**.
Smaller three-FE product time falls **9.15 to 5.41 ms**. Dense threaded/serial
products remain approximately **1.4 ms**. These are separate diagnostic blocks,
not a paired kernel benchmark; nested spans and worker-time sums are not additive.

An additional two-repeat diagnostic rotation on the fresh four-FE low-memory
case directly compares the instrumented baseline/candidate: **82 FE products**
take **29.82/9.81 ms**, and three joint calls take **44.10/24.01 ms**. Thus the
categorical saving has a direct shared-operator explanation. All outer and inner
iteration counts and step halvings remain equal in the headline pairs.

## Headline distributions

Cells are frozen before generation. Development seed **202609347** has three
rotations; fresh seed **202609418** has five. `median [min,max]` is milliseconds.
Paired losses/ties compare candidate with native baseline; they are not stock
win counts. All these states are scientifically rankable.

### development

| Case | N/K/T | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses/ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Large three-FE slopes, fast|61440/10/8|206 [204,212]|195 [195,198]|231 [226,231]|0.947|0.844|0/0|
|Same, low|61440/10/8|224 [223,225]|212 [211,216]|231 [226,234]|0.946|0.918|0/0|
|Two slope partitions T8|30720/6/8|92 [90,92]|93 [91,94]|80 [80,80]|1.011|1.163|3/0|
|Smaller three-FE slopes T4|30720/6/4|106 [105,106]|101 [99,102]|114 [110,117]|0.953|0.886|0/0|
|Two slope partitions T1|30720/6/1|148 [146,148]|149 [147,150]|318 [318,320]|1.007|0.469|2/0|
|Narrow categorical|21120/2/4|36 [35,36]|36 [35,37]|31 [26,31]|1.000|1.161|2/0|

### fresh

| Case | N/K/T | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses/ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Large three-FE slopes, fast|69120/10/8|233 [232,243]|220 [218,223]|259 [256,267]|0.944|0.849|0/0|
|Same, low|69120/10/8|252 [252,261]|241 [237,245]|258 [255,260]|0.956|0.934|0/0|
|Two slope partitions T8|36480/6/8|112 [111,116]|112 [112,116]|83 [77,84]|1.000|1.349|2/1|
|Smaller three-FE slopes T4|34560/6/4|111 [107,113]|105 [103,108]|131 [129,132]|0.946|0.802|0/0|
|Two slope partitions T1|36480/6/1|186 [184,189]|190 [186,193]|330 [328,331]|1.022|0.576|4/1|
|Narrow categorical|24000/2/4|40 [39,42]|40 [39,40]|33 [30,33]|1.000|1.212|0/4|
|Weak slope barbell|14400/4/4|38 [37,41]|38 [38,40]|742 [740,743]|1.000|0.051|1/2|
|Disconnected slopes, low|24960/8/4|116 [115,120]|117 [116,118]|150 [149,150]|1.009|0.780|3/1|
|Two slopes per partition|28800/10/8|172 [161,175]|167 [161,173]|189 [187,189]|0.971|0.884|2/0|
|Four categorical FE, low|30000/8/4|195 [193,198]|173 [172,181]|484 [481,488]|0.887|0.357|0/0|

### confirm

| Case | N/K/T | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses/ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Serial slopes|36480/6/1|185 [183,189]|188 [185,189]|—|1.016|—|3/0|
|Disconnected slopes, low|24960/8/4|118 [116,119]|117 [114,117]|—|0.992|—|1/1|
|Four categorical FE, low|30000/8/4|187 [186,190]|167 [167,171]|—|0.893|—|0/0|

### ols

| Case | N/K/T | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses/ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Small serial two-FE|3840/2/1|54 [53,56]|50 [50,51]|—|0.926|—|0/0|
|Wide two-FE|24960/10/4|37 [36,38]|37 [37,37]|—|1.000|—|1/1|
|Three-FE|30720/8/4|54 [52,55]|55 [54,56]|—|1.019|—|2/0|
|Wide three-FE, low|34560/12/4|153 [152,157]|152 [151,153]|—|0.993|—|0/2|
|OLS slopes|28800/6/4|98 [94,101]|98 [98,100]|—|1.000|—|1/1|

### ols-confirm

| Case | N/K/T | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses/ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Three-FE|30720/8/4|50 [50,53]|52 [51,53]|—|1.040|—|4/0|

Development has **11 wins, seven losses, no ties** in 18 native pairs. Fresh
has **29 wins, 12 losses and nine ties** in 50 pairs. Seven fresh medians clear
0.90 stock. The remaining gaps to that target are **8.8 ms** large low-memory,
**37.3 ms** two-slope T8, and **10.3 ms** narrow categorical. Do not transfer
older seeds' results to this artifact or infer universal parity from this screen.

The five-pair unchanged-source confirmation regenerates byte-identical full and
warm binary inputs, verified by captured hashes. Serial slopes still lose **3 ms**
at the median (three paired losses); disconnected slopes improve **1 ms** after
the initial 1-ms loss. The four-FE gain persists. OLS uses fresh seed **202609419**,
three rotations over five controls; its scientific exports are exact. The
three-FE OLS 1-ms median loss persists as **2 ms** in its five-pair same-input
confirmation. All native control distributions remain visible. The production
change does not route OLS through the joint PPML solver; no unsupported causal
explanation for these small excluded-path losses is asserted.

## Scientific and allocation validation

All **17 distinct focused Rust tests** pass. The three independent analytic
joint oracles each run 36 thread/coupling/weight configurations, including
forced compact representation, fast and streamed products, duplicate/deficient
slopes, disconnected components and scaled controls. Thirty-six configurations
force compact representation, 24 with helpers. The separately ignored frozen
frame diagnostic is not counted as run.

All **13 licensed fixture executions** pass (11 production, two diagnostic),
with explicit Stata status and terminal sentinels. Independent explicit-design
Poisson/Hessian/sandwich fixtures cover multipartition slopes, three partitions,
canonical products, OLS and unit/FW/PW, offset, robust/cluster inference. Existing
joint fast and low audit fixtures remain controls. The new
`feppml_compact_joint_slope_audit.do` uses enough remaining coordinates to exclude
the dense Schur bank and repeated sparse group triples to admit compact coupling.
Fast/low T1/T4 executions each pass the real 16-iteration zero-events/zero-growth
window, stable capacities and fresh/warm agreement. A separate instrumented
witness records **1,674** cooperative audit products and **302** public products.
The first diagnostic invocation omitted process stderr; it passed science but
could not establish routing. Its receipt is preserved along with the corrected
explicit process-output capture. Neither diagnostic contributes a headline time.

All **258 PPML public states**, **163 baseline comparisons**, **68 direct stock
comparisons** and **20 OLS pairs** pass. Of the PPML states, 234 are headline
calls and 24 are diagnostics; OLS contributes 40 headline calls. Independent
large-state sample, score, objective replay and model-space checks complement
the smaller explicit-design oracles. Reference agreement alone is not the
acceptance criterion. No application failure, timeout, isolation failure or
scientific discrepancy occurred in these screens.

All **83 headline PPML pairs** have exact tracked allocation peaks and scratch
reports. Selected-memory telemetry differences range **-112 to +112 bytes**;
no additional numerical bank is allocated or admitted. Worker accumulations
change floating-point reduction grouping on the cooperative path, as expected;
registered numerical envelopes pass. This is not a physical-cap or RSS result.

## Receipts, decision and next work

`receipts-content-addressed.zip`, its SHA-256 sidecar, `archive-verification.json`
and `restore_receipts.py` preserve and verify the sanitized logical receipts.
The archive contains **11,199 logical receipts / 2,409 payloads / 22,541,741 bytes**,
SHA-256 `750b1d04e875e9f980722f4d4b219bb3cd6d0e089e7cf3b147086723439e075f`.
It includes declarations, identities, source snapshots, harnesses, all scientific
exports/audits, timings, build/test evidence, filtered diagnostic/fixture protocols
and hashes for omitted data, plugins and licensed logs. Restored hashes are
verified; raw licensed logs, license headers, machine home paths and data/plugin
binaries are excluded. Work root is `target/ppml-joint-operator-20260925`.

Retain this coherent no-added-bank improvement with the listed small losses.
Close operator scheduling for now. The remaining two-slope gap is not explained
by its 1.4-ms dense product. Next investigate Poisson initialization and conditional
coefficient updates that avoid whole weighted solves. In current source, slope
partitions skip even exact conditional intercept initialization although those
intercept directions remain in their model space; establish that boundary with
independent truth, refusal/rollback and public evidence before generalizing it.
Then evaluate slope-local Newton updates only if residual profiles justify them.

Seeds 418/419 are consumed validation, not tuning. Frozen 77/16/111 definitions,
static allocation source drift, constrained-fit/nonretaining-CG failures and
platform deferrals remain unchanged. No qualification, push, release, SCC retry
or stopped campaign was performed. The broader goal remains active.
