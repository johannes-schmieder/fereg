# Current-call joint PPML acceptance consolidation

Local Mac arm64 development evidence, 2026-09-25. Not qualification.
[ADR 0198](../../../../docs/adr/0198-current-call-joint-ppml-checks.md) retains
**jointchecks1**. This is a shared 1–5-ms median improvement on seven fresh
families; the narrow excluded median ties. It does not complete the multi-FE
<=0.90-times-comparable-stock objective or replace the accepted solver.

## Source, ownership and independent authority

Candidate source is **181735c6** plus `fe/map.rs`,
`fe/slope_coefficients/joint.rs` and `ols/linear_session.rs` under
`crates/fereg-core/src/`. The map.rs changes add norm-handoff assertions to
existing independent analytical tests. All **558 captured computational inputs**
match the accepted candidate worktree. Baseline is **e7731129/finalreplay1**;
its artifact was built from f97c9dbe plus the earlier final-replay overlays.

| Artifact | Plugin SHA-256 |
|---|---|
|finalreplay1|84db18f8e91f31eb64f26c854df98609b3722a3d49828edbad040118b3f188b3|
|jointchecks1|515e1fa04ca6c62338001a033eb71ca5c5da0a2fe1d6769e91374b350e913a9d|

A successful streamed joint call already computes stable weighted norms from
untouched original finite columns, in selected-column order, before its recurrence.
Those immutable same-call norms now supply the independent score denominator.
A failed call exposes none. Dense FE-equivalent representatives have different
norms and remain ineligible; their acceptance still scans original-column norms.
Every original finite score and current mean numerator is freshly recomputed.
No recurrence residual or cached score substitutes for an acceptance numerator.

One serial row pass computes the two stable response/residual norms, compensated
weight sum and optional reporting mean with each statistic's original row order.
Independent reconstruction checks run in bounded parallel tiles; their maxima and
boolean conjunction merge exactly. These checks overlap original-column score/mean
work on the installed team. The mean handoff lasts only through the current
reporting call. The other frozen reporting route retains its previous mean scan.
There is no changed iteration schedule, weighted operator, tolerance or finalization
policy. Ordinary/strict joint users benefit; categorical LSMR and OLS are excluded.

## Diagnostic attribution

One separately declared native rotation uses unchanged production artifacts and
solver tracing, not an instrumented stock comparator. Its eight public states
and four native pairs pass. Reported phase milliseconds below are inclusive and
must not be added as a causal decomposition of the rounded public clocks.

| Case | FE projection B/C | Finite solve/checking B/C | Strict/final B/C |
|---|---:|---:|---:|
|Disconnected low 61440/K6/T4|24.459 / 24.153|11.715 / 7.585|17.408 / 17.168|
|Two slopes low 30720/K6/T8|18.720 / 18.626|6.121 / 4.138|12.365 / 12.394|
|Two slopes fast 30720/K6/T8|18.107 / 18.425|6.125 / 3.765|11.329 / 10.480|
|Three-FE slopes low 61440/K10/T8|51.167 / 52.786|14.417 / 9.948|46.840 / 47.459|

The observed savings lie in the changed checking boundary. Recurrence, reported
outer/inner iterations and fallback counts are unchanged in every measured pair.
The larger canonical preparation/final-Hessian cost remains. These diagnostics
neither promise another 20 ms from checking nor revive reference-weight proposals.

## Complete public distributions

Pinned unmodified **fixest 0.14.2**, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
uses `glm.tol=1e-10`, `fixef.tol=1e-11`. OpenMP/Rayon match requested threads;
BLAS remains one. Both implementations include robust inference and fitted means.
Data generation/import, process startup and warmup precede the clock; public
loading, estimation, checks, inference and requested output remain inside it.
No build, profile, archive or competing fit overlaps headline timing. All process
monitors pass, subject to their sampling rather than an ambient-load guarantee.

The six-case pilot has three alternating baseline/candidate/stock rotations on
development seed347. No source tuning follows its result. Times are listed in
repetition order, including losing pairs and ties.

| Case; N/K/T; mode | Baseline ms | Candidate ms | Stock ms | B/C/S medians | C/stock | C/B wins/losses/ties |
|---|---|---|---|---|---:|---|
|Disconnected categorical; 61440/6/4; low|112, 111, 115|108, 109, 106|101, 99, 95|112/108/99|1.091|3/0/0|
|Narrow categorical; 24960/2/4; fast|40, 41, 39|39, 40, 39|35, 29, 30|40/39/30|1.300|2/0/1|
|Two slope partitions; 30720/6/8; low|82, 79, 81|79, 78, 76|78, 82, 82|81/78/82|0.951|3/0/0|
|Two slope partitions; 30720/6/8; fast|74, 77, 78|76, 76, 73|79, 80, 80|77/76/80|0.950|2/1/0|
|Wide categorical; 23040/36/8; low|86, 85, 85|85, 85, 88|128, 121, 132|85/85/128|0.664|1/1/1|
|Unbalanced three-FE slopes; 61440/10/8; low|195, 194, 196|191, 189, 190|234, 233, 235|195/190/234|0.812|3/0/0|

The pilot has **14 wins, two losses and two ties** against baseline. Its three
principal low-mode targets win all nine pairs. Fast slopes lose one pair, and
wide controls lose one pair and tie their median. The narrow path is unchanged;
its one-ms median improvement is not attributed to this optimization.

Fresh seed **202609431** was declared before generation. The candidate remained
frozen for five rotations over eight predeclared definitions. This block is
separate from the pilot and older seed430 clocks; different seeds/blocks are not
pooled to claim an absolute-time change.

| Case; N/K/T; mode | Baseline ms | Candidate ms | Stock ms | B/C/S medians | C/stock | C/B wins/losses/ties |
|---|---|---|---|---|---:|---|
|Disconnected categorical; 61440/6/4; low|118, 121, 116, 117, 116|117, 114, 113, 112, 112|100, 98, 97, 98, 99|117/113/98|1.153|5/0/0|
|Serial categorical; 38400/10/1; low|142, 143, 146, 144, 143|140, 140, 138, 141, 141|241, 242, 245, 241, 244|143/140/242|0.579|5/0/0|
|Narrow categorical; 24960/2/4; fast|40, 41, 41, 41, 42|44, 40, 41, 45, 41|29, 31, 31, 29, 30|41/41/30|1.367|2/2/1|
|Wide categorical; 23040/36/8; low|91, 89, 89, 89, 91|90, 90, 88, 88, 88|127, 129, 129, 129, 129|89/88/129|0.682|4/1/0|
|Two slope partitions; 38400/8/4; fast|109, 107, 110, 106, 105|106, 106, 105, 109, 106|180, 178, 182, 181, 179|107/106/180|0.589|3/2/0|
|Two slope partitions; 30720/6/8; low|85, 82, 82, 83, 83|81, 80, 82, 82, 83|78, 77, 78, 76, 87|83/82/78|1.051|3/0/2|
|Two slope partitions; 30720/6/8; fast|81, 84, 82, 82, 81|79, 79, 78, 78, 76|79, 79, 79, 79, 78|82/78/79|0.987|5/0/0|
|Unbalanced three-FE slopes; 61440/10/8; low|201, 205, 201, 200, 198|196, 197, 198, 196, 196|230, 228, 227, 231, 229|201/196/229|0.856|5/0/0|

Fresh totals: **32 paired wins, five losses, three ties**. Seven medians improve
and the excluded narrow median ties. The T4 slope gain is only one millisecond
and two pairs lose; wide has a losing pair. Narrow has two losses of four ms.
Retention rests on shared work removal, no added retained memory or solver
policy, and consistent wins across low categorical, serial, fast T8 slopes and
three-FE slopes. This bounded optimization is closed; do not tune it further.

Four of eight fresh medians meet 0.90 stock. Remaining savings to reach the
threshold in this block are **24.8 ms** for low categorical (113 to 88.2),
**14.0 ms** for narrow (41 to 27), **11.8 ms** for low T8 slopes (82 to 70.2),
and **6.9 ms** for fast T8 slopes (78 to 71.1). This candidate does not close a
previously losing family at the ten-percent threshold. The next work must address
larger canonical/preparation or narrow-lifecycle costs, with a measured work budget.

## Science, controls and memory

Thirteen distinct final Rust tests pass, including analytical weighted slope
truth, deficient slope channels, independent input-norm calculations, changed
weights/responses, invalid/failing calls, zero responses, thread controls, strict
current-Hessian truth, budget refusal, rollback and failed recovery. The new
response-check test covers partial tiles, T1/T4, scales from 1e-150 to 1e150,
zero response, nonfinite/invalid weights and unrepresentable arithmetic. Its norms,
means and reconstruction agree with the separately executed former passes.

Six licensed scientific fixtures pass with explicit terminal sentinels and zero
process/Stata return codes: iterative canonical, multi-partition slopes,
three-partition slopes, scientific contract, weights/inference and separation
outputs. Independent explicit-design Poisson/Hessian/sandwich and weight-semantic
oracles remain authoritative. The isolated plugin passes its actual ABI/self-test
and intentional panic-containment check.

All **182 public states, 120 baseline-anchored comparisons and 58 direct stock
pairs** pass. All **62 native PPML pairs** have byte-identical coefficient,
covariance and fitted-mean exports. No new scientific discrepancy, timeout or
application failure occurs. Existing unrelated scientific gaps remain open.

The unchanged OLS controls have nine exact-export pairs. Baseline/candidate
medians are **9/10, 60/60 and 116/116 ms** for small categorical, three categorical
and absorbed slopes. Two pairs win, three lose and four tie. The small-fit median
loss remains visible; these controls do not establish universal non-regression.
No OLS algorithm or routing is changed.

The norm handoff borrows the existing joint scale bank. Concurrent acceptance
uses the finite-solve scratch already reserved by initial canonical rank solving;
no new retained owner, observation bank, heap allowance or worker team is added.
Every measured PPML pair has zero scratch and tracked peak delta. Selected-model
telemetry varies by -112 to +112 bytes; it is not a payload increase.

Four fixed-configuration licensed allocation audits pass: categorical low T1/T4,
and compact three-partition slopes fast/low T4. Each measures 16 warmed iterations
with zero allocation events/current growth, stable capacities and cold/warm
scientific equivalence. This does not repair the documented static inventory
source drift, prove physical caps or resolve mixed-thread Crossbeam reclamation.
Constrained-fit/nonretaining-CG failures, the baseline checkpoint-count assertion,
the older three-FE coefficient discrepancy and deferred platforms remain open.
Frozen 77/16/111 definitions are unchanged. No qualification, push or release.

## Immutable receipts

`receipts-content-addressed.zip`: **6,769 logical receipts, 1,787 payloads,
15,048,585 bytes**; SHA-256:
`64009b22fdefd9ad3fa7b2839b9c450fbdbcbb680dd434dc8c92b231a25280f2`.

Run `python restore_receipts.py DESTINATION` to restore source snapshots, build/test
recipes, declarations, monitors, full timing vectors, audits and protocol traces.
All logical receipts have been restored and hash/privacy checked. Data and plugin
binaries are omitted with hashes and reproduction recipes; raw licensed logs are
excluded. `summary.json` and `verification.json` expose the checks above.
Seed431 is now consumed/frozen and cannot become a tuning seed.
