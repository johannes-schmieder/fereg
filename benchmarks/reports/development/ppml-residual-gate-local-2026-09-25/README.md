# Fresh canonical residual certificates before correction

Local Mac arm64 development evidence, 2026-09-25. No qualification or release.
Decision: [ADR 0190](../../../../docs/adr/0190-canonical-residual-certificate-gate.md).

## Accepted implementation and scientific boundary

Retain **resgate1**, based on `27c408ea` with changes to `fe/map.rs` and
`ols/linear_session.rs`. All **556 captured inputs** match accepted source.
The coefficient projector previously solved every canonical finite residual,
even when it was already assembled from projected columns. At this one boundary,
try the unchanged full fresh FE certificate first. Passing returns the same rows;
refusal performs the original solve/reconstruction/final certificate. The early
return reports zero inner iterations and does not manufacture a Newton update
or outer convergence witness.

Require already authorized current sample/topology/weight geometry and coupling.
A changed generation follows the original solve: local QR preparation alone
cannot authorize an operator that has not been filled under these weights.
Authority is revoked before validation and restored only on independently
certified success. Nonfinite/invalid inputs and errors retain existing behavior.
The finite/rank/inverse, original score, model-space/reconstruction, outer
objective/score, strict comparisons and exact final-Hessian conditions remain
unchanged. No tolerance, sample, weight, inference or publication rule changes.

The entry is scoped to the PPML coefficient owner without an alternate canonical
projector and to one finite residual RHS. It can also serve eligible four-plus
categorical owners. Ordinary RHS calls, alternate categorical projectors and OLS
retain their paths. Share the existing implementation using a private scoped
flag; add no solver layer, observation bank or memory-plan credit.

## Identities and matched reference

B is the accepted **normhandoff2** artifact from `27c408ea`; C is **resgate1**.
**normstage2** and **resgatestage1** are isolated instrumented counterparts.
They are never headline comparator builds. Complete captured sources, build
logs, identity maps and preparation recipes are archived; both new builds pass
the actual-plugin ABI/self-test harness.

| Artifact | Plugin SHA-256 |
|---|---|
|normhandoff2|a92b48fdb328d115d0a5f7a42b4797d789caf520cd7291a1eaf8afa91ef24962|
|resgate1|9103c3092e83291e2941634ac07de06d445bfec85c39ca372a3b96f833f166d9|
|normstage2|ef31b8a339a4182e4630be650c78ebebe5b9a8b6db55cfd01268bb07432a3349|
|resgatestage1|9e419a02b049d97b1f0ed553efd3b9ddf06e01f319f3e6f50d58c589dbada8a1|

Stock fixest is pinned unmodified 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Use `glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads, robust inference and
fitted means, with BLAS/OMP/vecLib threads one. The headline surface is numeric,
unweighted, positive-outcome PPML. Default-reference failures remain separate
historical evidence; ordinary fixest weights do not establish Stata frequency-
weight inference. Public science is audited before ranking comparisons.

## Native mechanism and public pilot

Two diagnostic rotations on four development cases use the last full plugin
span, excluding warm fits. Both canonical residual gates pass on every slope
repetition: 12 acceptances, zero refusals. Narrow categorical never enters it.

| Case | Rank residual projection B/C ms | Complete plugin B/C ms |
|---|---:|---:|
| Two slopes N30720/K6/T8 |2.325 / 1.206|71.817 / 70.659|
| Three-FE slopes N61440/K10/T8, fast |5.539 / 2.519|174.211 / 166.445|
| Same, low |5.935 / 2.380|188.669 / 185.196|
| Narrow categorical N21120/K2/T4 |0.691 / 0.671|27.905 / 29.317|

These are separately instrumented phase medians. Nested spans are inclusive;
worker-child sums are not wall time. The large-fast whole-plugin gain exceeds
the saved residual span and is not assigned entirely to the gate. The same
scientific check remains; this measures avoided corrective work around it.

Development seed **202609347**, three baseline/candidate/stock rotations:

| Case | N/K/T | B / C / stock median ms | C range ms | C/B | C/stock | Paired W/L/T |
|---|---|---:|---:|---:|---:|---:|
|Large unbalanced three-FE slopes, fast|61440/10/8|184 / 177 / 230|176–179|0.962|0.770|3/0/0|
|Same, low|61440/10/8|200 / 195 / 231|193–200|0.975|0.844|2/0/1|
|Two slope partitions, T8|30720/6/8|78 / 77 / 80|75–77|0.987|0.963|3/0/0|
|Smaller three-FE slopes, T4|30720/6/4|87 / 86 / 117|85–89|0.989|0.735|2/1/0|
|Two slope partitions, T1|30720/6/1|125 / 126 / 319|122–127|1.008|0.395|1/2/0|
|Narrow two-FE categorical|21120/2/4|36 / 36 / 30|36–37|1.000|1.200|1/2/0|

There are **12 paired wins, five losses, one tie**. Four of five slope medians
improve; serial slopes lose 1 ms and narrow categorical ties. The small shared
change proceeds to frozen fresh validation, with no thread/size/seed guard tuning.

## Fresh confirmation and losses

Seed **202609426**, five predeclared stock rotations. Sizes/widths changed before
generation; source and parameters remain fixed throughout this block:

| Case | N/K/T | B / C / stock median ms | C range ms | C/B | C/stock | Paired W/L/T |
|---|---|---:|---:|---:|---:|---:|
|Large unbalanced three-FE slopes, fast|72960/12/8|223 / 222 / 307|220–226|0.996|0.723|3/2/0|
|Same, low|72960/12/8|261 / 262 / 307|257–264|1.004|0.853|2/3/0|
|Two slope partitions, T8|40320/6/8|97 / 96 / 89|94–97|0.990|1.079|5/0/0|
|Smaller three-FE slopes, T4|38400/8/4|127 / 126 / 207|123–127|0.992|0.609|4/0/1|
|Two slope partitions, T1|40320/6/1|158 / 159 / 360|156–164|1.006|0.442|1/4/0|
|Narrow two-FE categorical|26880/2/4|43 / 44 / 33|42–44|1.023|1.333|1/2/2|
|Weak slope barbell|15360/4/4|39 / 40 / 1182|39–40|1.026|0.034|1/2/2|
|Disconnected slopes, low|26880/8/4|96 / 94 / 168|92–95|0.979|0.560|5/0/0|
|Two slopes per partition|30720/10/8|145 / 142 / 209|140–145|0.979|0.679|4/0/1|
|Four categorical FE, low|32000/8/4|168 / 164 / 478|164–169|0.976|0.343|3/1/1|

There are **29 wins, 14 losses and seven ties** across 50 baseline/candidate
pairs. Five of eight slope medians improve by 1–3 ms; large low, serial and weak
slopes each lose 1 ms. Four-FE categorical improves 4 ms, narrow categorical loses
1 ms. The latter does not use this entry, so its movement is a control rather
than a causal gate result. All ranges and original losses remain evidence.

Eight of ten candidate medians meet 0.90 stock. Threaded two-slope PPML remains
**96/89 ms**, needing **15.9 ms** to reach 80.1 ms; narrow categorical remains
**44/33 ms**, needing **14.3 ms** to reach 29.7 ms. These are new sizes/seeds;
do not interpret their changed ratios versus prior reports as the causal gain.
The same-input pilot and fresh baseline/candidate pairs measure that gain.

One subsequently predeclared **diagnostic-only** check uses the identical full
and warm inputs for the three losing slope cases (binary matrices verified
identical), the fixed source and two instrumented rotations. It has no stock
headline comparison or further uninstrumented repetitions:

| Fresh case | Rank residual B/C ms | Plugin B/C ms | Accepted/refused gates |
|---|---:|---:|---:|
| Large low N72960/K12/T8 |6.979 / 2.869|250.166 / 245.968|4 / 0|
| Serial two slopes N40320/K6/T1 |5.393 / 4.112|142.752 / 141.624|4 / 0|
| Weak slopes N15360/K4/T4 |1.342 / 0.815|33.610 / 32.955|4 / 0|

The headline losses are not caused by repeated gate refusal on these measured
frames. The phase savings persist, but these diagnostic fits do not replace the
headline distributions or prove a whole-command win in every case. Retain the
inexpensive shared removal of corrective work and stop this mechanism here.
Do not tune acceptance thresholds, thread exclusions or topology guards.

## Science, controls and memory

All **232 PPML public states**, **150 baseline comparisons**, **68 direct stock
pairs**, **nine OLS pairs** and **nine licensed fixture executions** pass.
There are 204 headline PPML fits, 28 separately instrumented PPML fits and 18 OLS
fits. Every monitor record has passing application status, no sampled overlap
and no monitor error; that does not establish absence of all ambient load.
No compilation, profiling, archiving or competing fits run alongside headline
benchmarks. All operations are terminal before sealing.

Only eight of 68 headline PPML pairs are byte-identical across b/V/means; the
gate deliberately returns already-certified rows without another floating-point
correction. The largest baseline/candidate absolute differences are **1.102e-16
in coefficients, 1.356e-20 in covariance, 4.797e-14 in means and 2.911e-11 in
deviance**; reported log likelihood is identical. All established numerical
comparison envelopes and independent public FE/model-space/objective/score
audits pass. Recorded outer/inner iteration and halving counts match in every
headline pair. Bitwise equality with the old correction is not the contract.

Final-source Rust: **132 distinct passes**, one ignored timing probe and the
known baseline `refined_checkpoint_preserves_accuracy_on_a_small_dynamic_range_model`
assertion failure (strict rejections expected one, observed zero). The baseline
reproduction remains in previous reports; no assertion or tolerance changes.
The new complete-product weighted contrast oracle proves projection truth with
duplicate/zero slopes, T1/T4, weight scales 1e-60–1e60, RHS scales 1e-60–1e60,
zero residuals, changed weights and changed responses. Valid inputs return
unchanged; contamination triggers correction. Invalid shape/nonfinite data/
zero iteration budget revoke authority, and recovery rebuilds geometry.

Five licensed oracles cover weighted multipartition Poisson/Hessian, three-way
slopes, canonical products, one-FE categorical and explicit three-FE OLS. Four
real warmed-allocation fixtures cover fast/low T1/T4, poolsize two: 16 audited
iterations with zero events/growth, stable capacities and fresh/warm/public
agreement. This retains the existing ordinary-iteration audit window, not a
new full canonical-allocation or physical-RSS qualification.

Fresh OLS seed **202609427**, three pairs each, all audited exports exact:

| Control | N/K/T | B/C median ms | C range ms |
|---|---|---:|---:|
| Two categorical FE, robust |4320/2/1|10/9|9–12|
| Three categorical FE, robust |34560/8/4|57/57|57–58|
| Two slope partitions, robust |30720/6/4|99/99|97–99|

The OLS route is unchanged; do not attribute the small-control timing change to
the gate. Scratch and tracked peak deltas are zero in all 68 headline PPML pairs.
Selected-memory telemetry is exact in development and varies **-48 to +48 bytes**
in fresh pairs. No new heap state or admission credit is introduced. Capacity
checks and observed peaks do not qualify physical memory caps or other platforms.

## Immutable evidence

The archive contains **9,465 logical receipts / 2,195 payloads /
23,454,787 bytes**, SHA-256 `22c5a78710656cd631b2d6a8c07464c669d0f3b0247c9124e63ab2f27ff5b967`.
It was privacy-checked, restored and its verification receipt compared exactly.
Sources, identities, declarations, schedules, all timings/exports/audits, phases
and test protocols are retained; data/binaries and raw licensed logs are omitted
with hashes. Use `restore_receipts.py` with a new destination. Work root:
`target/ppml-residual-gate-20260925`.

Frozen 77/16/111 definitions, static allocation-inventory drift, known weighted
constrained-fit/nonretaining-CG failures, deferred platforms and stopped campaigns
remain unchanged. No push, release, SCC retry or qualification. Seeds 202609426/
427 are consumed validation inputs. The 0.90-times-stock goal remains open;
current next steps belong only in STATUS.md.
