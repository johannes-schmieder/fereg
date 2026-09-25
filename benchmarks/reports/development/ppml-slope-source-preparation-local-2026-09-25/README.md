# PPML slope source preparation — 2026-09-25

Local Mac arm64 development evidence. [ADR 0199](../../../../docs/adr/0199-ppml-slope-source-preparation.md)
retains **slopetopology1**. Removing unnecessary categorical encoding improves
all 35 fresh slope pairs, with 3–18-ms median savings. It does not complete the
multi-FE <=0.90-times-comparable-stock objective. OLS slope timing sensitivity
remains unresolved and is reported below.

## Exact source and scope

Baseline runtime is c3623dec / **jointchecks1**, originally built from 181735c6
with the ADR 0198 overlays. Candidate is **d25c6e06** plus five captured files:
`ols/mod.rs`, `ols/prepared.rs`, `ols/prepared_categorical_data.rs`,
`ols/prepared_fit.rs` and `ppml/prepared_model.rs` under `crates/fereg-core/src/`.
The final two files add tests only. All **558 computational inputs** match the
final candidate tree. Each measured arm has a captured source snapshot, plugin
identity, controller revision and immutable run receipts.

| Artifact | Plugin SHA-256 |
|---|---|
|jointchecks1|515e1fa04ca6c62338001a033eb71ca5c5da0a2fe1d6769e91374b350e913a9d|
|slopetopology1|1c598ce6ecbdea8e1a6e0ffcc9dfcd655c89165750a8d2b02d8189ec15b9ae1e|

The candidate splits complete borrowed numeric classification from categorical
completion. After the original source-use/domain/sample checks, PPML with
continuous finite terms removes synthetic slope-category markers. It still
retains numeric slope values and encodes every actual partition and cluster
source. Every sample generation repeats the same classification. This extends
ADR 0087's accepted preparation mechanism without extending its OLS solver guard.
No weighted geometry or certificate is reused across a changed frame.

Finite factor recipes carry indices into the full availability table and retain
the previous path; their guard is necessary unless those indices are explicitly
rebound. There is no size, thread, weight or memory-route threshold. No numerical
solve, iteration schedule, scientific envelope, final q*mu Hessian, inference,
separation, SPI boundary or publication transaction changes.

## Attribution and preserved failed diagnostics

The pending accepted-source lifecycle experiment is now captured with this
milestone. `jointcheckslifecycle1` instruments c3623dec; the ado-only diagnostic
reuses the stock jointchecks1 plugin. Its four cells have two rotations each.
All 32 public states in the completed lifecycle/ado/preparation diagnostics pass
independent replay; eight instrumented-versus-stock-native pairs pass too.
These diagnostics are distinct from comparator headlines.

The lifecycle ledger partitions actual intervals to avoid adding nested or
worker-inclusive times. On disconnected low categorical, initial/strict canonical
work occupies about 28.4 ms and ordinary joint work 19.6 ms. Narrow categorical
has about 6.2 ms canonical and 5.6 ms ordinary LSMR. Three-FE slopes have about
83.6 ms canonical and 31.2 ms ordinary joint work. Full interval lists, call counts
and disjoint wall categories are in `ledger.json`; per-category medians should
not be treated as an exact additive decomposition of median public time.

Same-command ado timers separate metadata, shadows/request, plugin, bounded
validation, observation validation and publication. Non-plugin timer sums are
14.5 ms low categorical, 8.5 ms narrow, 10.0 ms two-partition slopes and 13.0 ms
three-FE slopes. These rounded diagnostic sums have timer/entry/exit limitations;
the public-minus-native residual is not automatically removable Stata work.

The paired preparation diagnostic uses `jointchecksprepare1` and
`slopetopologyprepare1` in two uncontended rotations. Inclusive medians:

| Phase | Two-partition slopes B/C ms | Three-FE slopes B/C ms |
|---|---:|---:|
|Prepared model|6.351 / 2.092|14.282 / 4.944|
|Categorical factorization|4.022 / 0.298|8.652 / 0.487|
|Availability FE topology|0.831 / 0.241|1.899 / 0.745|
|Canonical preparation|21.790 / 20.505|83.427 / 83.217|

The removed factorization/membership work explains the useful shared saving.
Do not add these nested rows or infer a canonical improvement from timing noise.
The remaining canonical and narrow lifecycle costs need a different intervention.

Preserved development failures: the first lifecycle tag collided with an existing
artifact and did not overwrite it; a preparation recipe initially lacked
`__file__`; the first candidate compile used the wrong private module path. One
preparation diagnostic controller stopped on a competing compilation after its
first application. That partial run is excluded from attribution and a new,
uncontended two-rotation block is used. No such overlap occurred in headline
rotations. Raw licensed logs are excluded from the archive.

## Public timing protocol and pilot

Pinned unmodified **fixest 0.14.2**, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
uses glm.tol=1e-10 and fixef.tol=1e-11. Both commands include robust inference and
fitted means, with matched requested OpenMP/Rayon threads and BLAS1. Process
startup, data generation/import and a warm fit precede the clock; native loading,
estimation, checks, inference and requested outputs remain inside it. No build,
profile, archive, audit or competing fit accompanied headline measurements.
All sampled process monitors pass; this is not a guarantee against ambient load.

The existing development seed202609347 has three alternating B/C/stock rotations:

| Case; N/K/T; memory | Baseline ms | Candidate ms | Stock ms | B/C/S medians | C/stock | Wins/losses/ties |
|---|---|---|---|---|---:|---|
|Disconnected categorical; 61440/6/4; low|115, 113, 111|113, 112, 111|98, 97, 99|113/112/98|1.143|2/0/1|
|Narrow categorical; 24960/2/4; fast|41, 41, 42|40, 41, 41|30, 31, 32|41/41/31|1.323|2/0/1|
|Two slope partitions; 30720/6/8; low|84, 84, 82|78, 78, 78|77, 78, 78|84/78/78|1.000|3/0/0|
|Two slope partitions; 30720/6/8; fast|80, 81, 81|76, 76, 79|79, 81, 78|81/76/79|0.962|3/0/0|
|Wide categorical; 23040/36/8; low|91, 91, 88|91, 92, 90|128, 129, 127|91/91/128|0.711|0/2/1|
|Unbalanced three-FE slopes; 61440/10/8; low|198, 199, 195|191, 188, 188|235, 230, 233|198/188/233|0.807|3/0/0|

The pilot has **13 wins, two losses and three ties**. All nine slope pairs
improve. Wide categorical has two losses and a tied median; its path is excluded.
No source correction follows the pilot or fresh results.

## Fresh frozen-source confirmation

Seed **202609432** and all eleven cells were declared before generation. Five
alternating B/C/stock rotations retain the eight previous family definitions and
add multiple slopes, weak connectivity and a larger local sample. This block is
not pooled with seed431 or earlier artifacts.

| Case; N/K/T; memory | Baseline ms | Candidate ms | Stock ms | B/C/S medians | C/stock | Wins/losses/ties |
|---|---|---|---|---|---:|---|
|Disconnected categorical; 61440/6/4; low|113, 113, 114, 114, 112|112, 113, 113, 111, 114|98, 98, 100, 99, 98|113/113/98|1.153|3/1/1|
|Serial categorical; 38400/10/1; low|143, 143, 141, 140, 143|144, 144, 144, 141, 141|246, 248, 245, 244, 245|143/144/245|0.588|1/4/0|
|Narrow categorical; 24960/2/4; fast|43, 44, 41, 44, 43|45, 40, 42, 43, 41|31, 32, 31, 30, 31|43/42/31|1.355|3/2/0|
|Wide categorical; 23040/36/8; low|88, 88, 86, 86, 87|89, 89, 88, 85, 85|129, 127, 127, 128, 123|87/88/127|0.693|2/3/0|
|Two slope partitions; 38400/8/4; fast|103, 104, 105, 103, 105|98, 99, 99, 98, 99|185, 185, 184, 185, 187|104/99/185|0.535|5/0/0|
|Two slope partitions; 30720/6/8; low|79, 80, 78, 79, 83|75, 74, 72, 77, 73|81, 80, 80, 81, 79|79/74/80|0.925|5/0/0|
|Two slope partitions; 30720/6/8; fast|79, 76, 77, 74, 75|75, 74, 71, 70, 72|79, 79, 79, 80, 79|76/72/79|0.911|5/0/0|
|Unbalanced three-FE slopes; 61440/10/8; low|189, 189, 191, 191, 193|184, 185, 184, 180, 181|225, 229, 226, 228, 230|191/184/228|0.807|5/0/0|
|Three slope channels on two partitions; 38400/8/4; low|144, 143, 144, 147, 146|136, 136, 135, 139, 140|270, 272, 270, 274, 276|144/136/272|0.500|5/0/0|
|Weak barbell slopes; 17280/4/4; low|47, 45, 43, 43, 45|43, 43, 42, 42, 42|1075, 1067, 1067, 1060, 1060|45/42/1067|0.039|5/0/0|
|Larger unbalanced three-FE slopes; 122880/10/8; low|400, 402, 399, 416, 402|382, 388, 379, 384, 389|485, 485, 489, 489, 493|402/384/489|0.785|5/0/0|

Fresh totals: **44 wins, ten losses and one tie**. All **35 slope pairs** improve;
median savings are **3–18 ms (3.7–6.7%)**. Seven of eleven medians meet 0.90 stock,
all already met by the baseline in this block. The narrow control has two losing
pairs, and serial/wide categorical medians each lose one ms. Their unchanged
preparation paths do not receive an attributed benefit from this change.

Remaining savings to 0.90 in this block are **24.8 ms low categorical, 14.1 ms
narrow, 2.0 ms low T8 slopes and 0.9 ms fast T8 slopes**. Do not tune a guard or
revisit the closed OLS weighted-slope effort for these small residual slope gaps.
A later broad canonical/lifecycle improvement may help them naturally.

## Science, OLS controls and memory

Eleven distinct Rust tests pass. The new classification test compares samples,
models, topology, source order and errors across 72 combinations, including
multiple/slope-only channels, categorical interactions, missing outcomes/slopes,
excluded invalid values, singleton deletion, source-role overlap, deficient
channels, FW/PW and factor-route controls. The generation test covers subset
rebasing, raw pweight restoration, exact capacity bounds and failure rollback.
Independent analytical weighted slope tests and current-Hessian truth remain.

Six licensed fixtures pass with terminal sentinels and zero process/Stata return
codes: iterative canonical, two-partition slope, three-partition slope,
scientific contract, weights/inference and separation/output oracles. Explicit
Poisson designs independently check coefficients, means, current Hessian and
robust/cluster sandwich products, including frequency multiplicity and pweights.
The built plugin also passes actual ABI and intentional panic containment.

All **247 public PPML states, 160 baseline-anchored comparisons and 73 direct
candidate/stock pairs** pass. All **87 native PPML pairs** have byte-identical
coefficient, covariance and mean exports, with unchanged outer/inner iterations
and fallback counts. Additional lifecycle diagnostic states are counted separately
above. No scientific discrepancy or application timeout occurred in these blocks.

Excluded one-FE PPML controls have B/C medians **22/22 ms categorical** and
**73/74 ms low slopes**, with one win, three losses and two ties. Their scientific
exports agree exactly. The one-FE slope median loss is retained.

OLS public controls have exact exports in all nineteen pairs across the initial
and one predeclared confirmation block. Initial B/C medians are **11/11, 59/57,
108/114 ms** (small categorical, three categorical FE, two slope partitions),
with five wins, three losses and one tie. The serial/slope confirmation has
**9/8 and 107/113 ms**, with six wins and four losses. The repeated **6-ms slope
median loss is a development concern**, not a proven harmless fluctuation.

It triggered a bounded attribution diagnostic. The first attempt's inherited
FEREG_PHASE_TIMING setting was correctly stripped by the harness, so it supplied
no phase evidence. The corrected explicit --trace block records median OLS
preparation **5.683/5.506 ms**, initial absorption **58.236/53.247**, and robust
score-design storage **30.849/31.652**. Its public medians reverse to **113/110**;
that diagnostic is not headline evidence and does not erase either loss. Source
inspection, equal solver counts and equal allocation/scratch reports find no
additional OLS work or changed numerical path. Cause remains unresolved; carry
this control forward. The six diagnostic pairs also have exact exports, making
**25 OLS pairs** across all blocks. Full timing vectors are in `summary.json`.

Retention is justified by removing demonstrably unnecessary preparation and all
35 improving fresh PPML slope pairs, without adding persistent machinery. It is
not an overall non-regression or release decision. No OLS solver tuning was added
to compensate for the control clocks.

Two fixed-configuration slope allocation executions (fast/low T4) pass, each
with sixteen warmed iterations, zero events/current growth, stable capacities and
cold/warm scientific equivalence. No observation bank or retained cache is added.
All PPML scratch deltas are zero; tracked peaks change -48 to zero bytes and
selected telemetry -112 to +48 bytes. Temporary preparation allocations shrink,
but later phases dominate whole-command peaks. Existing admission estimates stay
conservative; no physical-cap or static-inventory repair is claimed.

## Next boundary and immutable receipts

The preparation change is closed. Continue with shared canonical construction
and narrow lifecycle costs, using the archived whole-command ledger and carrying
the OLS slope timing concern forward. Preserve the current independent checks,
exact final weights, frozen 77/16/111 definitions, previous rejected experiments,
constrained-fit/nonretaining-CG failures and older three-FE discrepancy. Linux
and constrained-platform confirmation remain deferred; no SCC retry, push,
release or qualification occurred. Seed432 is consumed/frozen.

`receipts-content-addressed.zip`: **13,421 logical receipts,
2,652 payloads, 23,461,770 bytes**; SHA-256:
`04297021fc960fed2b8b734394f45e9c908882eea9229a9ff33148064f035fa6`.

`restore_receipts.py DESTINATION` verifies and restores all logical receipts.
The archive contains full source snapshots, build and diagnostic recipes, declared
cells, controller/monitor receipts, timing vectors, scientific audits and failed
attempts. Data and binaries are omitted with hashes/reproduction recipes; paths
are sanitized and raw licensed logs excluded. `summary.json` and
`verification.json` provide machine-readable results.
