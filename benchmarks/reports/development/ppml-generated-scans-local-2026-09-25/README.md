# Generated-column PPML scans — 2026-09-25

Adopt **generatedscan1**, based on **5d84eb9d** plus six Rust overlays.
[ADR 0206](../../../../docs/adr/0206-bounded-generated-column-scans.md) records the
decision. All **565 computational input hashes** match the accepted source.
Plugin SHA-256:
`4443226a78caa96a264b9dd8e6d7e9a72b5551aec99e349bd7e22895ffd2b469`.
Baseline **pairedscan2** is the accepted 7cd9c579 runtime, captured from daa0ce4e
plus its four overlays; plugin
`4d4ae065abac8b9250c8f9218172ea6853f9b3d0e90a869e702a47ce53dc3020`.
Captured identities, source files, build recipes and ABI-check output bind each
artifact separately. The frozen primary/holdout/native-control definitions are
unchanged.

## Attribution and implementation

An isolated baseline diagnostic build, **factorprof1**, wraps native phase
functions and records the existing solver trace. Its plugin hash is
`e3b2dbd8ad8f2ae5f6b6b3afc0362c954daa14c27f89afdc852c959bf07f9f80`.
It is never a headline arm. Stock R profiling is diagnostic only; the stock
library is unmodified. No separately instrumented stock or exported identical-
weighted-frame inner-solver experiment was needed for this implementation; those
remain available for the next solver redesign.

In the full low-factor diagnostic, initial retained design construction costs
174 ms; three ordinary joint calls cost 93/91/91 ms. Six outer score certificates
cost roughly 22–23 ms apiece. The strict transaction includes 115 ms retained
reprojection, 35.5 ms reference norms, 33.6 ms response absorption and 24.1 ms rank
solve. These nested timers must not be summed with their enclosing timers. The
numeric-width comparison changes the model and weighted frames; its faster scores
and joint calls are a clue, not causal attribution of the entire difference.

Source inspection finds pointwise interaction evaluation and serial recipe-backed
residual/certificate passes around an already efficient joint solver. Scratch
block I/O was already tiled. The accepted replacement shares an immutable column
range visitor, resolves interactions in 128-row Boolean tiles, fills existing RHS
blocks in row panels, reconstructs joint residuals in disjoint 1024-row panels,
and computes independent moments by column on the existing team. Outer finite
scores reuse the existing 64-entry scalar bank in chunks. Numeric slices and
simple indicators use the same interface without an expanded design.

No recurrence, check frequency or tolerance changes. Current-weight geometry,
rank, FE/finite scores, objective, reconstruction, independent final Hessian and
publication remain authoritative. Per-row FMA and per-column observation orders
are preserved. Baseline/candidate outer and reported inner counts match in all
63 pilot/fresh pairs. This milestone improves surrounding data access and checks;
it does not replace the already accepted slope solver or attribute all residual
cost to stronger scientific requirements.

Public native phase medians on development seed347, B/C milliseconds:

|Case|Preparation|Projection|Finite|Objective/line search|Strict final|Inference|Publication|
|---|---:|---:|---:|---:|---:|---:|---:|
|Low factor|102.6/80.4|428.5/222.7|39.7/33.6|94.4/14.0|236.4/184.8|39.1/40.0|23.0/23.0|
|Fast factor|98.0/77.0|406.6/407.7|51.1/50.4|94.9/14.0|166.0/116.8|12.5/13.2|19.7/19.8|
|Million rows|233.7/234.3|245.6/246.4|105.1/109.4|51.9/50.9|258.0/263.5|6.7/6.9|121.8/121.4|

The low-factor preparation/projection/objective/strict reductions account for
about 360 ms, consistent with the public gain. Fast ordinary projection remains
unchanged. These are measured phase differences, not candidate function-profile
timings. Native phases exclude some public loading/control costs and are not an
exhaustive sum of the public timer.

## Matched public timings

All clocks include estimation, requested robust/two-way-clustered inference and
fitted means. Imports and scientific exports are outside. The same declared
threads apply to both estimators. Stock is the pinned unmodified library at
`glm.tol=1e-10`, `fixef.tol=1e-11`; archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Default-reference accuracy failures are separate historical evidence.
Baseline/candidate/stock order rotates. No build, audit, profile, archive or
competing fit runs alongside headline clocks. Every attempt has clean sampled
monitors, passing application status and a completed controller sentinel.

Pilot, development seed **202609347**, three repetitions. Factor cross adds 11
finite columns to eight numeric controls; all large pilot cases use T8. Slope
control has two intercept-plus-one-slope partitions; disconnected control is T4.
Vectors are milliseconds in repetition order; ratios use medians.

|Case; N/finite K; mode, VCE|Baseline|Candidate|Stock|C/stock|
|---|---|---|---|---:|
|Three unbalanced FE, factors; 153600/19; low, 2 clusters|1081,1077,1077|717,717,723|724,719,719|0.997|
|Three unbalanced FE, numeric; 153600/19; low, 2 clusters|674,666,675|664,663,664|634,648,631|1.047|
|Factor case; fast, 2 clusters|962,999,969|830,815,817|718,713,719|1.138|
|Two FE; 983040/6; fast, robust|1230,1228,1245|1244,1301,1235|883,879,884|1.409|
|Two slope partitions; 30720/6; low, robust|70,71,73|69,69,69|81,83,80|0.852|
|Two disconnected FE; 30720/6; low, 2 clusters|59,60,58|59,60,59|63,59,63|0.937|

Low factors improve **33.4%**, fast factors **15.7%**. The million-row median
loses 14 ms, including a 1301-ms candidate tail. Keep that loss; this change does
not solve the large-N gap. The first three and million-row families still need
approximately **70,93,171,449 ms** respectively to reach 0.90 stock. Only the
slope pilot clears 0.90 in all three pairs.

Fresh **seed202609437**, predeclared before execution, frozen candidate, five
rotations. All cells request means. The first two use more unbalanced factor
levels and are different designs from the pilot; their much slower stock clocks
do not close the pilot gaps. The weak case is categorical, despite the topology
generator's historical `weak_slope_barbell` name. Multiple-slope and numeric
controls deliberately exercise other paths.

|Case; N/finite K/T; mode, VCE|Baseline|Candidate|Stock|C/stock|
|---|---|---|---|---:|
|Three unbalanced FE, factors; 184320/21/8; low, 2 clusters|1477,1481,1494,1480,1472|1067,1077,1067,1063,1073|3372,3347,3380,3352,3356|0.318|
|Same design; T4 fast, 2 clusters|1314,1325,1353,1304,1313|1155,1152,1153,1157,1155|6224,6191,6175,6179,6188|0.187|
|Two FE, factor main; 38400/10/1; low, robust|160,159,161,162,159|159,158,157,155,157|236,227,235,234,236|0.668|
|Two weak FE, factor cross; 38400/11/4; low, 2 clusters|179,187,171,177,181|127,132,153,124,143|3116,3099,3106,3115,3099|0.042|
|Three unbalanced FE, two slopes on each of two partitions; 38400/6/8; low, robust|124,121,123,123,125|123,124,121,124,124|118,116,114,120,123|1.051|
|Four FE numeric; 38400/6/4; fast, robust|160,164,164,158,159|164,164,165,159,162|434,431,431,434,441|0.378|
|Two FE numeric; 38400/4/1; low, 2 clusters|77,81,84,84,86|84,86,83,86,85|114,114,105,117,116|0.746|
|Two FE numeric; 737280/6/8; fast, robust|1058,1049,985,983,994|985,990,984,997,986|777,770,766,772,785|1.277|
|Two FE, wide factor cross; 30720/76/4; low, robust|580,581,579,570,578|355,355,356,358,359|721,724,719,700,723|0.494|

All **25 generated-case candidate/baseline pairs improve**; family median gains
are **27.9%,12.1%,1.9%,26.3%,38.5%** in their table order. Across all nine cells,
34 pairs win, ten lose and one ties. Seven families clear 0.90 stock in every
pair, but multiple slopes and large-N numeric remain slower. Four-FE numeric
loses four ms at the median, serial numeric one ms, and multiple slopes one ms.
No source or guard tuning follows fresh seed437. All earlier validation seeds
remain frozen too. These are local development results, not broad qualification.

## Science, controls and memory

Across baseline, diagnostic profile, small science, pilot and fresh screens,
**227 public states /146 baseline-anchored pairs pass**. All **63 direct
candidate/stock pilot/fresh pairs** pass sample, b, V, objective and mu checks.
The small N4800 science screen contributes **six independent explicit-design
Poisson/Hessian/sandwich checks**, including generated controls, low three-FE
robust and fast two-FE two-cluster/all-output cases. Large timing cells did not
run new dense independent oracles. This does not resolve earlier large saved-
residual, disconnected-mean or nested-DoF discrepancies.

**Sixteen distinct Rust tests pass.** New coverage checks raw-level recipe/range
values, tile boundaries, vanished levels, signed zero, interruption and bad
indices; generated outer certificates at N4095/4097, widths8/65/71, three-component
interactions, changing/extreme weights and T1/T4/T8. Focused existing tests cover
independent weighted joint WLS in both layouts, product boundaries, numeric
scores, weighted Poisson truth, strict finalization, changing-frame/unwind
invalidation and low-three-FE budget refusal. An initial incorrectly filtered
`--exact` invocation ran zero tests and is retained; only actual passing tests
count here.

Three licensed fixtures pass with explicit process/Stata return codes and terminal
sentinels: joint-column Poisson/Hessian/sandwich oracles; higher-dimensional
weighted oracles; and the PPML scientific contract. They cover unit/FW/PW,
robust/cluster, fast/low and failure behavior. There is no claim that ordinary
fixest weights validate Stata frequency-weight inference.

OLS controls use three B/C rotations on development seed347. All **12 scientific
export pairs are exact**; six timing pairs win, five lose, one ties:

|OLS control|Baseline ms|Candidate ms|
|---|---|---|
|Two slope partitions, N30720/K6/T4|67,68,68|66,66,66|
|Low three FE, N30720/K6/T4|76,76,77|79,77,76|
|Narrow two FE, N1920/K2/T1|14,14,12|14,11,15|
|Low three-FE factor cross, N30720/finite K17/T4|213,211,211|212,213,213|

All **63 pilot/fresh tracked native peak and scratch pairs are equal**. Modeled
selected bytes have small -48 to +112-byte differences. There is no new heap
bank or persistent cache; interactions need 128 Boolean stack entries per active
worker, replacing the predictor's former 128-double tile where applicable.
No new warmed-allocation audit, physical cap, Linux/SCC/Windows check, full suite
or qualification is claimed. Static inventory drift, constrained-fit failures
and nonretaining-CG instability remain open. No push or release occurred.

## Evidence

[Content-addressed receipts](receipts-content-addressed.zip),
[restorer](restore_receipts.py) and [verification](archive-verification.json)
preserve **8,443 logical files**, all restored and hash/privacy checked.
Container SHA-256:
`a5aef383ab52b8e76a0e65d51872126277049e426425cfce8dbae94b521805d9`.
The archive contains all source/artifact identities, declarations, controllers,
attempt outcomes, timings, scientific audits, phase/operation/memory summaries,
test results and sanitized protocol logs. Original and sanitized hashes remain
distinct. Generated data, row-output binaries/CSV and plugin binaries are hashed
and omitted; raw local evidence is preserved. Source-code privacy-test literals
and public shared-reference paths are explicitly distinguished from private run
metadata. No license data, raw licensed log or private machine path is published.

The original four-case baseline remains independently preserved in the prior
[inventory](../ppml-current-gap-inventory-local-2026-09-25/README.md); it is not a
second independent timing block. The next plan and sole mutable handover remain
in STATUS.md. Outstanding costs are concrete: fast-factor projection about408 ms;
low-factor projection/strict223/185 ms; million-row preparation234, projection246,
strict264 and publication121 ms. The broad 0.90-stock objective remains unfinished.
