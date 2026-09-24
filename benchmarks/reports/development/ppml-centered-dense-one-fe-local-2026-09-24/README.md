# Certified group-centered preparation for fast one-FE PPML

Local Mac development evidence, 2026-09-24. Adopt the preparation change in
[ADR 0158](../../../../docs/adr/0158-group-centered-fast-one-fe-ppml.md).
The wide final-artifact median falls from 87 to 74 ms; stock takes 65 ms.
The objective of at most 0.90 times comparable stock remains unfinished.
Serial gains are not established. This is not release or memory qualification.

## Source and artifact identity

Baseline is `componentwarm3`, numerical source `e9c7f598`; all 543 captured
computational inputs remain identical at the starting documentation checkpoint
`c490fef1`. Baseline plugin SHA-256:
`5434193a5a9b0408ed9636c625f1d134996f2224bff2642049b129a47245ce52`.

Accepted artifact **`onefecentereddense5`** is `c490fef1` plus the four Rust
overlays and new Stata oracle captured in its source map. All **544** captured
inputs match the adopted worktree. Plugin SHA-256:
`887001e476148c757b10362e09bc5ae7349b147d0129ed1ad00e5e3f6dcefbfc`.
`accepted-source-verification.json` records the comparison.

Development artifact `onefecentereddense1` established the design. Artifact2
renamed its internal ownership methods and added the initial generated-term
fixture; its plugin is
`881725a7b49d01482dd98d3b40ca6ac834dc7457c008b6ce6e8701fc3041cdc8`.
Artifacts3/4 update only that fixture and reuse artifact2's exact native binary
and ado. Artifact4 has the final fixture. Final source review then changes three
Rust comment lines only; artifact5 is independently rebuilt. Its binary hash
differs, so it receives its own public timing/scientific confirmation and three
licensed fixtures. Do not relabel artifact4 timings or artifact2 Rust test
execution as artifact5 execution.

Stock reference is unmodified pinned fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads, robust inference and
fitted means. BLAS/OMP are serial. Data creation/import precede timed public
commands; each process warms up first. Rotations alternate baseline, candidate
and stock ordering. No compilation, profiling, archiving or competing fits run
alongside headline fits. Instrumented artifacts are separate and never ranked
as stock or production headline results.

## Adopted change

Reuse the existing independently certified current-weight one-FE group-center
builder in admitted fast PPML sessions. It computes centers and raw/within
statistics, then separately certifies the original-minus-center coordinates.
Materialize those coordinates once into the existing fast dense bank. Strict
refresh rebuilds geometry from immutable originals under the current weights.
Low mode keeps its prior implicit representation. Dense finite products, rank,
QR fallback, inverse checks and inference keep their existing representation.

No tolerance, sample, weight, separation, DoF, solver-family or publication rule
changes. No dimension/thread/seed guard is introduced. Optional geometry memory
is admitted conservatively without credit for the displaced projector. Slopes,
multi-FE, synthetic no-FE and ordinary OLS keep their existing routes.

## Final compiled artifact confirmation

`reviewed-screen`: three rotations of a predeclared four-case subset of the
already frozen fresh set. All 36 applications, own-state checks and 24 native
or baseline-stock comparisons pass; all **12 direct candidate-stock** comparisons
also pass. No contention or monitor errors. Times below are milliseconds;
bracketed values give every repetition in order, not confidence intervals.

| Case | Baseline values | Final5 values | Stock values | Medians B / C / S | C/S |
|---|---|---|---|---|---:|
| One FE N23040/K8/T4 | 44,46,44 | 42,43,43 | 25,26,28 | 44 / 43 / 26 | 1.654 |
| Wide one FE N21504/K32/T8 | 84,88,87 | 74,76,72 | 67,65,61 | 87 / 74 / 65 | 1.138 |
| Unbalanced one FE N26880/K4/T1 | 50,60,56 | 56,58,56 | 23,27,23 | 56 / 56 / 23 | 2.435 |
| Protected two FE N16800/K12/T4 | 64,71,74 | 67,69,68 | 57,52,57 | 71 / 68 / 57 | 1.193 |

Paired candidate-minus-baseline differences are respectively `[-2,-3,-1]`,
`[-10,-12,-15]`, `[+6,-2,0]`, `[+3,-2,-6]`. The wide reduction is 14.9%, with
all three pairs improving. Serial median is unchanged. Protected two-FE changes
are noise, not a route-specific gain. None reaches 0.90 times stock. The wide
case still needs 15.5 ms (20.9% of its current time) to reach that target.

## Broader fresh validation, exact artifact4

`dense-fresh2`, seed **369**, five rotations. All 120 applications and own-state
checks pass; all 40 native pairs, 40 baseline-stock pairs and **40 additional
direct candidate-stock** comparisons pass. No fit overlaps. The numerical
implementation is unchanged between artifact4 and final5; timing identity
nevertheless remains artifact4.

| Case | Baseline values | Candidate4 values | Stock values | Medians B / C / S |
|---|---|---|---|---|
| One FE N23040/K8/T4 | 44,46,43,44,43 | 42,43,41,42,43 | 21,22,27,27,26 | 44 / 42 / 26 |
| Wide one FE N21504/K32/T8 | 88,86,85,84,82 | 71,75,74,72,76 | 69,64,69,59,70 | 85 / 74 / 69 |
| Unbalanced one FE N32255/K12/T4 | 72,63,64,61,63 | 58,60,57,56,56 | 37,41,39,36,42 | 63 / 57 / 39 |
| Unbalanced one FE N26880/K4/T1 | 55,49,61,55,52 | 49,51,51,57,55 | 25,24,26,27,24 | 55 / 51 / 25 |
| Small one FE N12000/K3/T1 | 34,31,36,30,26 | 26,32,26,32,34 | 14,16,13,13,16 | 31 / 32 / 14 |
| Protected low N21600/K6/T4 | 39,38,38,39,39 | 37,38,38,39,39 | 22,22,19,22,24 | 39 / 38 / 22 |
| Protected two FE N16800/K12/T4 | 70,67,65,64,68 | 69,65,68,66,66 | 53,50,55,49,56 | 67 / 66 / 53 |
| Protected two-slope N16800/K6/T4 | 71,70,69,71,70 | 70,71,69,71,71 | 67,61,67,64,65 | 70 / 71 / 65 |

Fourteen of fifteen changed threaded pairs improve, one ties. Wide and
unbalanced threaded median reductions are 12.9% and 9.5%. Serial distributions
are mixed; the small serial median loses 1 ms. The slope control also loses
1 ms at the median, with no paired increase above 1 ms. Do not convert these
results into a universal speedup claim.

The initial fresh declaration used N32256, which creates a singleton at the end
of the generator's five-row grouping. Generation refused before any fit.
Preflight also found OLS N10000 incompatible with the existing multiple-of-240
design. The preserved v2 declaration changes these to N32255 and N10080 before
any timed fit, retaining all other definitions and seed. Seed 369 is consumed
and frozen, including final-artifact confirmation; it is not a tuning seed.

Focused artifact4 OLS controls run three native pairs each: one FE N8192/K6/T1
has medians 11/11 ms (ranges 10–12/9–12); two FE N10080/K8/T4 has 18/17 ms
(17–18/17–18). All six pairs' scientific exports are **byte-identical**. This
is not the full frozen suite; the 77 primary, 16 holdout and 111 native-control
definitions are unchanged.

## Memory

Representative first accepted fresh attempts, bytes; complete tables are in
`dense-memory-summary.json`. These are tracked/modelled quantities, not a
physical constrained-memory qualification.

| Case | Tracked baseline → candidate4 | Modelled selected baseline → candidate4 |
|---|---:|---:|
| One FE N23040/K8 | 12,207,225 → 11,839,882 | 14,761,279 → 15,168,703 |
| Wide N21504/K32 | 25,818,960 → 26,290,928 | 27,055,754 → 28,271,170 |
| Unbalanced N32255/K12 | 19,359,876 → 18,846,505 | 21,148,614 → 21,189,406 |
| Serial N26880/K4 | 10,695,102 → 10,265,419 | 13,758,804 → 13,775,596 |
| Small N12000/K3 | 4,635,389 → 4,443,633 | 10,495,792 → 10,611,488 |
| Protected low | 9,137,852 unchanged | 13,896,933 unchanged |
| Protected two FE | 16,817,016 unchanged | 62,516,003 unchanged |
| Protected slopes | 13,082,716 unchanged | 17,253,761 unchanged |

Wide tracked peak rises **471,968 bytes, about 1.8%**. Four changed peaks fall.
Observed scratch is zero throughout; the low model's 1,209,600-byte scratch
bound is unchanged. Linux, physical caps and documented static-inventory source
drift remain open.

## Independent validation and visible failures

The numeric artifact2 source passes **64 distinct Rust tests**, covering
independent analytic weighted group projection, T1/T4, changing/extreme common
weight scales, rank deficiency, current-weight strict refresh, invalidation,
ownership, prepared IRLS and inference scans. Final5 changes only three Rust
comment lines after that numerical source; its independently built plugin
passes ABI/panic-containment checks and three licensed fixtures:

- `feppml_centered_dense_oracle.do`: generated main effects, exact duplicate,
  interaction FE, offset, unit/FW/PW, robust/cluster, T1/T4 and all saved outputs;
  independent explicit-dummy Poisson means and normalized-q objective.
- `feppml_one_fe_dummy_oracle.do`: independent full dummy Poisson/Hessian and
  sandwich, unit/FW/PW, robust/cluster.
- `feppml_one_fe_profile_audit.do`: T1/T4, sixteen ordinary iterations with zero
  audited allocation/growth events, outputs and independent weighted
  zero-control closed form.

Return codes and terminal sentinels pass. Numeric source2 also passes the
transactional rollback prefix through line 167 of the existing output fixture.
The full historical fixture fails on **baseline and candidates** at its stale
protocol-minor assertion (expects 2, current 3); it is not reported as passing.

The new oracle's first version used unsupported continuous-by-factor terms,
failing r498 on baseline and candidate. The corrected supported fixture then
incorrectly compared native normalized-PW likelihood with Stata's raw-PW
likelihood; both failed r9. Independent means agreed to 1.31e-14 relative error;
raw PW mean 2.1993489583333337 explains the likelihood scale exactly under
ADR 0016. The final fixture independently replays normalized-q likelihood and
passes both. Original failed fixtures and diagnostic results are preserved;
no scientific tolerance changes.

Development seed 347 large one-FE fast N61440/K10 and low N49152/K12 retain
existing stock-mean envelope failures. Their native comparisons and own-state
checks pass, but stock timings remain **unranked**. In the accepted prototype
screen, native medians are 98→87 and 86→85 ms respectively; those observations
do not establish stock parity. Six direct prototype-stock comparisons fail
on these same cases. Smaller development cases are 44→38/27 ms and serial
55→57/26 ms. No discrepancy is hidden or repaired by this change.

## Rejected alternatives

1. **Mata view validator**, `ppmloutputview2`: identical baseline native plugin,
   only ado changed. Development medians B→C are 44→49, 97→117, 86→98, 58→66 ms.
   Every family loses. Earlier view1 fails to load because of unsupported Mata
   increment syntax; preserve that failure. Adopt neither.
2. **Fused Stata expression validator**, `ppmloutputexpr1`: identical baseline
   plugin. Initial medians 44→44, 97→95, 86→83, 56→52 ms. Fresh seed 367 gives
   38→39, 64→63, low 54→49, protected two-FE 64→67, slopes 67→65. Science passes,
   but protected losses reject a global change. Seed 367 is consumed/frozen.
   Both corrected validators pass 112 corruption-oracle combinations and the
   rollback prefix. Their private helper/fixture remain archived, not shipped.
3. **Raw norm handoff/direct projection**, `onefefresh1`: 21 distinct Rust tests
   and two licensed fixtures pass, including independent analytic projections.
   Development medians 44→43, 97→95, 84→82, serial 55→59; serial paired deltas
   +3,+5,-1 ms. Inconsistent small gains do not justify extra machinery.

Do not tune width/thread guards around these rejected probes. Full-output and
OLS declarations from the output experiment were not run after rejection.
Several preliminary launch failures occurred before Stata invocation (relative
artifact paths, plugin outside the staged ado root, private-helper visibility);
their receipts remain visible and are not counted as successful fits.

Across all completed headline blocks there are **387 successful applications**,
with no contention, plus ten separate successful diagnostic applications.
Application success does not erase the disclosed stock discrepancies. There
are also two failed diagnostic workers before invocation, the generation
refusal and fixture failures above. No process remains pending.

## Attribution and next implementation target

Separately instrumented seed 347 profiles support the preparation mechanism:
N16320/K12/T4 initial-plus-strict canonical time falls 13.522→8.110 ms;
native command time is 33.388→28.603 ms. Retained finite products are essentially
unchanged at 4.236→4.284 ms. New group construction takes 2.874 ms and dense
materialization 0.213 ms over two calls. The serial diagnostic also improves,
but headline distributions do not establish a serial win.

A separate **development**, not fresh, N30720/K32/T8 profile reports candidate
native 80.587 ms versus baseline 87.224. Candidate largest exclusive costs are
dense retained moments 12.112 ms, unattributed IRLS body 8.212, group refresh 6.912,
core preparation 6.889, inference 6.461, plugin loading 5.530 and cached objective
4.429. Concentrated direction is only 3.361 ms exclusive (4.449 inclusive).
The moments span is 7.125 ms in the single baseline profile: this variation
needs fixed-input attribution, not a regression claim from one trace.

The next coherent target is **dense weighted products plus robust sandwich
inference**, about 18.6 ms in that diagnostic, followed by common loading and
preparation (about 12.4 ms). Measure identical frozen-frame products; investigate
repeated normalization/row binding and bounded symmetric blocked products while
retaining cancellation, rank, inverse and independent score/Hessian safeguards.
Another concentrated-Newton replacement cannot explain most remaining time.
This milestone neither proves residual overhead unavoidable nor reaches the
0.90 goal. Older slope wins remain tied to their own artifacts pending final
remeasurement.

## Immutable evidence

`receipts-content-addressed.zip` contains **17,779 logical files**, **2,964 unique
payloads**, 21,649,430 bytes. SHA-256:
`f22aca1d8f35134a80f3084f8338067e994d59c854ab495494decc5d146858ad`.
The adjacent digest and `restore_receipts.py` independently verify every logical
payload; a clean restore completed with `Verified and restored 17779 logical
receipts`. Source maps/payloads, recipes, predeclarations, benchmark scientific
exports, distributions, direct stock audits, profiles, memory, fixture statuses
and sanitized protocols include rejected experiments and failures. Raw licensed
logs, binaries and generated DTA payloads are omitted with identity records;
machine paths and license data are not committed.

After restoring to a new directory, key entries under the experiment root are
`reviewed-timings.json`, `reviewed-screen-audit.json`,
`reviewed-direct-stock-audit.json`, `dense-fresh-declaration-v2.json`,
`dense-memory-summary.json`, `dense-final-rust-results.json`,
`onefecentereddense5-fixture-results.json` and
`accepted-source-verification.json`. Original runner receipts and each captured
source tree remain available for tracing summaries back to exact inputs.
