# Current-call norms in canonical PPML construction

Local Mac arm64 development evidence, 2026-09-25. No qualification or release.
Decision: [ADR 0189](../../../../docs/adr/0189-current-call-ppml-coefficient-norms.md).

## Accepted change

Retain **normhandoff2**, based on `d1b03ecdb7ed4589686457e89d084de12dd31b9f`
with changes only to `fe/map.rs` and `ols/linear_session.rs`. All **556 captured
inputs** match the accepted worktree. Transfer fresh input/residual norms from a
successful coefficient projection to canonical rank construction, using exactly
the same normalized weights and selected RHS columns. Remove duplicate scans in
initial fast, tiled low-memory and compatible fused/scratch refreshes.

No solver recurrence, stopping rule, certificate, tolerance or memory admission
is changed. Geometry reuse retains its owned sample/topology/weight generation;
RHS norms are freshly computed for every call. Selection or projection failure
and unwinds revoke geometry authority. Original means, current-weight FE/finite
score, rank, objective, reconstruction and exact final Hessian retain authority.
The existing alternate categorical normalization guards remain; categorical
controls are still required for the shared coefficient-owner branch.

The gain is small and broadly useful at little complexity or memory cost. Six
of eight fresh slope medians improve, one ties, one loses 4 ms. A single fixed-
source, identical-input confirmation of the loss gains 1 ms. Both blocks and
all tails remain visible. This step does not close the 0.90-times-stock goal.

## Artifacts and comparator

B is **slopecurve3**, the production artifact retained at `ad776bd5` and unchanged
by `d1b03ecd`; its 555 inputs are captured. C is **normhandoff2**. `normhandoff1`
is an unmeasured preliminary build before the explicit selection/output authority
guard and unwind test. Initial Rust logs identify that draft; only the final
logs below support final-source counts. `canonstage1` and `normstage2` are isolated
instrumented baseline/candidate builds and never headline comparator artifacts.

| Artifact | Plugin SHA-256 |
|---|---|
|slopecurve3|ebfca50ca203293d14e3b710951276b14d4221ff550b610a3c171bb1a7e22fe8|
|normhandoff1|74f3ac60c0b119975cf18ff55e4be20665d0ae9c7a8ffa5931a3022ae570d00d|
|normhandoff2|a92b48fdb328d115d0a5f7a42b4797d789caf520cd7291a1eaf8afa91ef24962|
|canonstage1|bbbbe12a1be91944b2f1d155f81921960a1ff7a1f07f1ab0dbab46abdab1ccf1|
|normstage2|ef31b8a339a4182e4630be650c78ebebe5b9a8b6db55cfd01268bb07432a3349|

Stock fixest is the pinned, unmodified 0.14.2 library, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Comparable work uses `glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads,
robust inference and fitted means; BLAS/OMP/vecLib threads are one. Cases are
numeric, unweighted, positive-outcome PPML. Frequency-weight inference is not
adjudicated by ordinary fixest weights. Historical default-reference accuracy
failures remain separate evidence. Samples and all public states pass their
independent audits before timing comparisons are ranked.

## Development and fresh public rotations

Development seed **202609347**, three baseline/candidate/stock rotations:

| Case | N/K/T | B / C / stock median ms | C range ms | C/B | C/stock | Paired W/L/T |
|---|---|---:|---:|---:|---:|---:|
|Large unbalanced three-FE slopes, fast|61440/10/8|195 / 187 / 230|184–188|0.959|0.813|3/0/0|
|Same, low|61440/10/8|208 / 205 / 233|205–206|0.986|0.880|3/0/0|
|Two slope partitions, T8|30720/6/8|84 / 82 / 78|80–83|0.976|1.051|3/0/0|
|Smaller three-FE slopes, T4|30720/6/4|93 / 91 / 112|91–92|0.978|0.812|2/0/1|
|Two slope partitions, T1|30720/6/1|130 / 128 / 324|128–130|0.985|0.395|2/1/0|
|Narrow two-FE categorical|21120/2/4|39 / 38 / 27|38–39|0.974|1.407|1/0/2|

There are 14 paired wins, one loss and three ties. The six-case block is useful
pilot evidence, not a substitute for fresh validation.

Fresh seed **202609424**, five predeclared rotations; source, cases and parameters
were fixed before generation, with no tuning after this block:

| Case | N/K/T | B / C / stock median ms | C range ms | C/B | C/stock | Paired W/L/T |
|---|---|---:|---:|---:|---:|---:|
|Large unbalanced three-FE slopes, fast|69120/10/8|218 / 211 / 270|207–215|0.968|0.781|5/0/0|
|Same, low|69120/10/8|237 / 233 / 269|231–248|0.983|0.866|3/2/0|
|Two slope partitions, T8|36480/6/8|95 / 94 / 80|93–97|0.989|1.175|3/2/0|
|Smaller three-FE slopes, T4|34560/6/4|101 / 101 / 127|99–101|1.000|0.795|3/0/2|
|Two slope partitions, T1|36480/6/1|151 / 149 / 327|148–153|0.987|0.456|3/2/0|
|Narrow two-FE categorical|24000/2/4|42 / 42 / 29|40–43|1.000|1.448|2/2/1|
|Weak slope barbell|14400/4/4|41 / 40 / 583|39–40|0.976|0.069|5/0/0|
|Disconnected slopes, low|24960/8/4|95 / 99 / 144|92–126|1.042|0.688|2/3/0|
|Two slopes per partition|28800/10/8|154 / 151 / 191|149–179|0.981|0.791|3/2/0|
|Four categorical FE, low|30000/8/4|172 / 169 / 496|168–175|0.983|0.341|4/1/0|

Across all ten cases: **33 paired wins, 14 losses, three ties**. Eight candidate
medians are at or below 0.90 stock. The threaded two-slope case remains **17.5%
slower than stock**, and narrow categorical **44.8% slower**. Their gaps to
0.90 stock are **22.0 ms** and **15.9 ms**. These inputs differ from earlier
reports and must not inherit earlier ratios. The categorical controls' small
time changes do not establish a causal benefit from the slope scan removal.

The low-memory large case reaches 248 ms, disconnected slopes 126 ms and the
multiple-slope case 179 ms. Do not discard these tails or infer absence of ambient
load from a passing sampled monitor. The disconnected original B/C median is
95/99 ms, with two wins and three losses. One subsequently predeclared five-pair
confirmation uses exactly the same full and warm data (DataFrames and binary
matrices verified identical), the same seed/source/options and no stock rerun.
It yields **95/94 ms**, candidate range 93–95, three wins/one loss/one tie. This
limits the evidence for a persistent regression; it does not erase the original
loss or convert the confirmation into an independent fresh-seed validation.
No further repetitions or threshold tuning followed.

## Separate native attribution

Four cases, two baseline/candidate rotations each, use the last full plugin span
and exclude the warm fit. All 16 diagnostic states pass science. Nested spans
are inclusive and must not be added; recursive parallel child totals are worker
activity rather than wall time. The Preparation clock still ends before initial
canonical WLS, as corrected in ADR 0188.

| Case | Initial duplicate norm scan B/C ms | Strict duplicate norm scan B/C ms | Combined retained construction B/C ms |
|---|---:|---:|---:|
| Two slopes N30720/K6/T8 |0.580 / absent|0.487 / absent|29.538 / 21.786|
| Three-FE slopes N61440/K10/T8, fast |1.662 / absent|1.611 / absent|71.535 / 68.431|
| Same, low |2.815 / absent|1.574 / absent|90.534 / 87.090|
| Narrow categorical N21120/K2/T4 |0.233 / 0.226|absent / absent|7.767 / 7.725|

The explicit duplicate norm calls disappear on the slope routes. Initial packing
also omits duplicate norm arithmetic. The two-slope diagnostic's much larger
8-ms construction difference exceeds the removed separately timed scans and is
not assigned entirely to this change: unchanged finite/rank and projection spans
also vary. Its headline pilot gain is only 2 ms and fresh gain 1 ms. Large-case
construction savings are approximately 3.1/3.4 ms. The solver and geometry kernels
are unchanged. Rank residual projection still costs approximately **2.59 ms**
(two slopes), **5.92/6.35 ms** (large fast/low), and **0.78 ms** (categorical).
A future certificate-first residual acceptance path must establish fresh accuracy
and cheaper total work; this profile does not itself authorize removing checks.

## Independent science, controls and memory

All **230 PPML public states**, **149 baseline comparisons**, **68 direct
candidate/stock pairs** and **nine OLS pairs** pass. All **73 headline PPML
baseline/candidate pairs** have byte-identical coefficients, covariance and means.
Recorded outer/inner iterations and step halvings match in every headline pair.
This is observed evidence, not a new universal bitwise-equality requirement.
There are 214 headline PPML fits, 16 instrumented PPML fits and 18 OLS fits.
Every process-monitor record has passing application status, no overlap and no
monitor error. No build, profiling or archive work ran alongside headline fits.

Final-source Rust logs (`rust-handoff-final`, `rust-slopes-final`,
`rust-irls-final`, `rust-session-final`) contain **131 distinct passes**, one
ignored timing probe and the known baseline failure
`refined_checkpoint_preserves_accuracy_on_a_small_dynamic_range_model`
(expected strict rejection one, observed zero). The baseline reproduction is
preserved in the prior accepted reports. No assertion or tolerance changed.
The new analytic weighted-projection oracle covers rank-deficient duplicate/zero
slopes, T1/T4, weights 1e-120 through 1e120, RHS scaling, changed weights/RHS,
reordered and duplicate column selection, zero columns, invalid selection,
nonfinite weights, failed convergence, unwind invalidation and same-generation
recovery with stable capacities. Norms are independently checked by direct sums
within safe arithmetic ranges.

Five licensed fixtures pass explicit terminal sentinels and zero process/Stata
return codes: weighted multipartition Poisson/Hessian, three-partition slopes,
canonical-product oracle, one-FE categorical oracle, and explicit three-FE OLS.
Four additional real warmed-allocation fixture executions cover fast/low T1/T4,
poolsize two: 16 audited iterations, zero allocation events/growth, stable
capacities and fresh/warm/public agreement. This is the existing ordinary
iteration audit window; it is not a full canonical-allocation or physical-RSS
qualification.

Fresh OLS seed **202609425**, three B/C pairs per case, all audited exports exact:

| Control | N/K/T | B/C median ms | C range ms |
|---|---|---:|---:|
| Two categorical FE, robust |3840/2/1|50/50|49–50|
| Three categorical FE, robust |30720/8/4|55/54|54–55|
| Two slope partitions, robust |28800/6/4|96/97|95–104|

The 1-ms slope-OLS median loss remains visible; this change adds no OLS routing.
No observation-sized storage or memory-plan credit is introduced. Scratch and
tracked allocation peaks have **zero delta in all 73 headline PPML pairs**.
Reported selected-memory deltas range **-112 to +112 bytes** and are not claimed
identical or a memory saving. The implementation leaves conservative admission
unchanged. Physical caps and platform qualification remain deferred.

## Evidence boundary

The verified archive contains **9,943 logical receipts / 2,080
payloads / 17,202,626 bytes**, SHA-256 `1dd4500686f239453ff0ac372a72fa3f07b896e79788a662bff7a8819d73ee7c`.
It was privacy-checked, restored and its scientific verification receipt compared
exactly. Complete captured sources, build identities, declarations, schedules,
all timings/exports/audits, diagnostic spans and test protocols are retained.
Data/binaries and raw licensed logs are omitted with hashes. Use
`restore_receipts.py` with a new destination. Work root:
`target/ppml-norm-handoff-20260925`; all operations are terminal.

Frozen 77/16/111 definitions, static allocation inventory drift, weighted
constrained-fit and nonretaining-CG failures, deferred platforms and stopped
campaigns are unchanged. No push, release, SCC retry or qualification occurred.
Seeds 202609424/425 are now consumed validation inputs. Current priorities and
the ambitious remaining implementation plan belong only in STATUS.md.
