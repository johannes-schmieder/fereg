# Fast three-FE joint-engine comparison — local development, 2026-09-25

## Decision and scope

**Reject the automatic engine replacement.** Preserve generatedscan1 runtime and
retain the expanded categorical scientific/transaction fixtures. The corrected
candidate still loses 14% on sparse and 60% on disconnected development cases.
This milestone establishes no accepted production speedup and does not close the
multi-FE 10% goal. [ADR 0207](../../../../docs/adr/0207-fast-three-fe-joint-directions.md)
records the decision and canonical-owner invariant.

The accepted fast two-/three-categorical-FE ordinary engine already is joint
LSMR. This experiment compares it with the existing coefficient Schur joint
engine. It does not compare repeated full-control demeaning with a joint solve.
Automatic two-FE, low storage, slopes and four-plus FE retained their routes;
explicit fast three-FE MAP gained the coefficient owner in the experiment.
The candidate kept one topology, current-weight geometry, independent acceptance
and a borrowed canonical categorical projector. Optional memory refusal restored
LSMR. No scientific tolerance, rank policy or check frequency changed.

## Source and execution identities

- Baseline **generatedscan1 / df0472d6** plugin SHA-256:
  `4443226a78caa96a264b9dd8e6d7e9a72b5551aec99e349bd7e22895ffd2b469`.
- Timed first candidate **fastjoint3**, df0472d6 plus captured overlays:
  `8e339fff7727f479a3ff819b816f6a43285d91f3623431518a6b80d10a888f54`.
- Corrected candidate **fastjoint4**, same base plus captured overlays:
  `a507073f1522cfab8861f2a8aa4c87303d531a7686d4d3dc787ed9834affccfa`.
- The archive retains six source identities, 565 inputs each, including diagnostic
  jointcompare2 and failed/superseded fastjoint1/2. Identity/source-match receipts
  bind each artifact. Build ABI checks passed; injected panic containment is
  intentional. Candidate4.patch preserves the final rejected implementation.
- Unmodified pinned fixest archive SHA-256:
  `17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
  Comparable stock uses `glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads,
  robust or two-way cluster inference and means. Source R/estimation.R and
  src/demeaning.cpp are preserved separately; stock was not instrumented.
- Local Apple Silicon/Stata MP 19. Controllers capture artifact, monitor and
  command identities. Sampled contention monitors passed. No compilation,
  profiling, audit, archiving or competing fits accompanied headline clocks.
  This is bounded local development evidence, not platform qualification.

## Identical weighted-frame attribution

Diagnostic jointcompare2 captures exactly the same FE design, raw Hessian weights,
working target and active finite controls for both solvers. It executes LSMR first,
then the coefficient call, recording setup, solver/certificate work, reconstruction
and actual LSMR operator counts. All **17 frames** pass the coefficient engine's
own certificate and an additional original-input LSMR-form FE/finite check.
Public diagnostic exports match baseline (six public states, three pairs).

|Frozen full-size frames|Coefficient call ms|LSMR setup/solve/check ms|Additional costs outside coefficient call|
|---|---|---|---|
|Three-FE factors N153600/K19/T8, three frames|25.945, 24.336, 23.491|79.699, 126.289, 112.423|First owner 6.891; separate cross-check 8.152, 7.026, 7.396|
|Three-FE numeric N153600/K19/T8|about 24 per frame|71.8, 45.0, 37.3|See frozen-frame-summary for exact frames|
|Two-FE N983040/K6/T8, two frames|about 63, 64|about 54, 55|First owner 22.555; extra cross-check about 12 per frame|

For the three factor frames, LSMR iterations are 19/31/30, forward applications
20/36/33 and adjoint applications 21/36/33; coefficient iterations are 7/4/4.
These iteration labels do not count equal work. The sequential order, extra
checks and diagnostic I/O prevent treating these as headline speedups. Large
captured frames have no separate dense explicit WLS oracle; independently
implemented weighted kernel oracles and small public Poisson/Hessian checks
supply separate mathematical evidence. Frame binaries are hashed, not published.

The finite block/choice of FE-equivalent control representatives is a hypothesis
for the large factor advantage. No isolated attribution yet proves it. Two-FE
ordinary solve replacement cannot remove the full large-N public-command gap.

## Complete public timings

Entries below are **median [individual repetitions] in milliseconds**. B is
accepted generatedscan1; C3/C4 are distinct rejected artifacts. Order rotations,
complete attempts, monitor outcomes, native phase tables and scientific audits
are archived. No wins are transferred between candidate versions or to retained
runtime. Cases with † have failed stock mean comparisons and are **unranked
against stock**, regardless of speed; B/candidate comparisons still pass.

### Development seed347, three rotations, candidate3

|Case; N|B|C3|Stock|
|---|---:|---:|---:|
|Fast three-FE factor cross; 153600|843 [843, 843, 853]|640 [644, 625, 640]|725 [724, 725, 728]|
|Fast three-FE numeric; 153600|582 [583, 582, 578]|552 [552, 543, 572]|642 [641, 642, 654]|
|Fast three-FE unbalanced K6; 30720|88 [88, 86, 88]|70 [70, 72, 70]|79 [79, 79, 79]|
|Fast million-row two-FE (control); 983040|1290 [1282, 1308, 1290]|1326 [1326, 1463, 1296]|948 [927, 962, 948]|
|Low three-FE factors (control); 153600|739 [739, 744, 739]|746 [745, 746, 750]|731 [731, 731, 730]|
|Low two slope partitions (control); 30720|74 [74, 75, 72]|74 [78, 74, 74]|78 [80, 76, 78]|
|Fast four-FE (control); 30720|129 [127, 129, 130]|127 [130, 127, 125]|318 [318, 321, 317]|
|Fast three-FE narrow T1; 1920|12 [12, 12, 13]|13 [13, 13, 13]|35 [35, 34, 35]|

### Frozen fresh seed202609438, five rotations, candidate3

|Case; N|B|C3|Stock|
|---|---:|---:|---:|
|Fast three-FE factor cross; 184320|1058 [1014, 1066, 1046, 1072, 1058]|886 [872, 874, 886, 908, 896]|3268 [3241, 3207, 3268, 3270, 3296]|
|Fast three-FE balanced T1; 38400|190 [215, 195, 188, 187, 190]|260 [263, 260, 261, 256, 259]|1276 [1308, 1283, 1276, 1275, 1274]|
|Fast sparse three-FE†; 38400|185 [185, 184, 190, 184, 186]|336 [330, 340, 347, 327, 336]|16309 [16309, 16339, 16261, 16323, 16272]|
|Fast disconnected three-FE†; 38400|183 [183, 182, 185, 183, 183]|484 [486, 484, 484, 485, 483]|10332 [10332, 10345, 10332, 10323, 10337]|
|Fast wide three-FE K65; 15360|220 [219, 219, 220, 221, 223]|215 [215, 214, 214, 217, 226]|1008 [1011, 1007, 1006, 1008, 1019]|
|Fast large two-FE (control); 737280|939 [985, 993, 936, 931, 939]|935 [933, 989, 930, 948, 935]|774 [774, 762, 779, 775, 770]|
|Low three-FE factors (control); 61440|351 [343, 361, 374, 351, 341]|353 [355, 353, 341, 353, 340]|416 [421, 416, 405, 416, 408]|
|Low three-FE/four slopes (control); 38400|121 [120, 119, 128, 121, 122]|121 [121, 120, 120, 122, 121]|119 [121, 121, 117, 117, 119]|

### Canonical-owner correction, development seed347, three rotations

|Case; N|B|C3|C4|Stock|
|---|---:|---:|---:|---:|
|Fast balanced three-FE T1; 38400|186 [187, 186, 182]|256 [256, 255, 260]|187 [187, 187, 188]|1318 [1318, 1312, 1329]|
|Fast sparse three-FE†; 38400|178 [179, 178, 177]|311 [311, 310, 323]|203 [202, 204, 203]|11132 [11132, 11140, 11106]|
|Fast disconnected three-FE; 38400|248 [248, 251, 247]|651 [651, 650, 652]|396 [398, 395, 396]|10069 [10067, 10069, 10102]|

Candidate3's hard factor median improves 843 -> 640 ms, C3/stock=0.883;
all three paired ratios clear 0.90. Its smaller three-FE K6 median is 70/79,
but one repetition misses 0.90. These successes cannot offset fresh serial,
sparse and disconnected regressions of 36.8%, 81.6% and 164.5% against baseline.
The original hard low-factor, million-row and multi-slope losses remain visible.
The fresh factor stock slowdown is not evidence that the original hard gap closed.

### Canonical dispatch defect and corrected rejection

Candidate3 retained the borrowed categorical canonical owner for nonfused
projection but dispatched fused response/control strict projection through generic
coefficients. A red/green test checks the actual requested LSMR canonical report.
Candidate4 repairs that dispatch; it does not change acceptance tolerances.

On development balanced/sparse/disconnected cases, strict-phase medians B/C3/C4
are respectively **45.8/113.8/45.7**, **41.1/149.5/40.6**, and
**101.7/364.4/98.8 ms**. Ordinary FE-projection medians remain
**70.9/70.7/70.4**, **83.1/106.9/107.4**, and **83.6/223.9/235.0 ms**.
The fix removes duplicate expensive canonical organization but cannot cure the
ordinary sparse/disconnected losses. Fresh candidate3 outer counts match baseline
(four for factor/serial, six for sparse/disconnected), with no reported fallback
or strict-profile rejection. No candidate4 hard-factor headline or new fresh
campaign ran. Seed438 is consumed/frozen; seed439 is not consumed.

## Scientific results and failures

All **240 public states pass** across capture, small science, pilot, fresh and
corrected development screens. Baseline-anchored comparisons pass **145/162**;
independent explicit-design checks pass **4/6**; direct candidate/stock pairs
pass **62/75**. These counts are not interchangeable and are not an all-green
science claim. Detailed boundaries:

|Screen|Public states|Anchored pairs|Independent oracle|Direct candidate/stock|
|---|---:|---:|---:|---:|
|Frozen-frame capture|6/6|3/3|Not run|Not run|
|Small public science3|6/6|0/4|4/6|2/2|
|Development pilot3|72/72|48/48|Not run|24/24|
|Fresh candidate3|120/120|70/80|Not run|30/40|
|Corrected development|36/36|24/27|Not run|6/9|

Small N4800 science covers generated three-FE controls, T4 robust means and T1
clustered full outputs. Candidate3 and stock pass both explicit Poisson/Hessian/
sandwich checks. Baseline fails one near-zero coefficient coordinate in each
case: maximum absolute error 2.749545e-10, registered tolerance ratio 1.34971.
The baseline-anchored timing comparisons are therefore not ranked there.

Fresh stock mean failures affect sparse (one coordinate, maximum absolute
baseline/stock difference 2.5902e-7, tolerance ratio 1.091) and disconnected
(four coordinates, 6.069e-7, ratio 2.518) cases in all five repetitions. Direct
candidate/stock checks also fail there. Corrected development sparse stock fails
one mean coordinate, maximum 1.5971e-6, ratio 3.9829; disconnected comparisons
pass on that distinct development input. No tolerance was relaxed or discrepancy
adjudicated away.

**16 distinct Rust tests pass** across candidate snapshots: 11 core boundary
checks (rerun after the canonical correction), three separately implemented
weighted joint-solve oracles and two plugin tests. Tests cover route/admission
refusal, changing weights, slopes, strict rollback, higher-dimensional truth,
concrete solver reports and stores. A pre-existing checkpoint-count assertion
expects one rejection but observes zero on both candidate and isolated accepted
baseline; it is unchanged. The new perfect-fit LSMR fallback subcase hits an
existing relative-certificate numerical floor; the fallback check was moved to
the existing noisy fixture while analytic truth remained covered. Both failed
attempts are retained, not counted as passes.

All **five focused licensed fixtures pass** on candidate3, corrected candidate4,
and finally the restored accepted runtime with the retained fixture sources:
runtime validator, hidden transaction, categorical final oracle, joint-column
products oracle and scientific contract. Process/Stata return codes and terminal
sentinels establish completion. The expanded categorical fixture runs 72 native
fits: MAP/LSMR/auto, two/three FE, T1/T4, unit/FW/PW, robust/cluster, offset and
mu/eta/xb/d/residual reconstruction, with explicit Poisson and independent Mata
q*mu Hessian/sandwich authority. Three injected automatic three-FE publication
failures verify held b/V/sample and absent output names.

Earlier fastjoint1 returned r3499 at the native hybrid-report validator; the
precise candidate extension fixed it and was later reverted with the route.
The hidden transaction's stale wire-minor assertion failed on both candidate and
baseline; its retained correction is 2 -> 3, matching existing protocol 1.3.
An unsupported `store` fixture injection caused a test-authoring failure before
being replaced by supported postplugin/postpublic/mutatesolver stages. All attempts
are archived. The final fixture's comment says three-FE rather than hybrid; this
comment-only clarification follows the captured test run.

## Memory, preservation and limits

Candidate3 paired tracked-peak deltas range 0 to +20,591,736 bytes in pilot and
-34,340,792 to +26,212,848 fresh. Modeled selected deltas range 0 to +61,375,800
and -48 to +77,307,864 respectively. Corrected development peak deltas range
-34,033,080 to +7,643,792; modeled deltas +11,094,440 to +12,234,824.
Observed scratch deltas are zero. These are tracked allocation/model measurements,
not RSS or physical-cap certification. Conservative admission retains the entire
original LSMR allowance plus optional coefficient/canonical/current-weight state.
No warmed-allocation campaign or Linux/Windows confirmation ran.

After rejection, all runtime/build inputs match accepted generatedscan1. Of 565
captured inputs, only the two retained Stata fixtures differ; do not claim all565
match this test-only source checkpoint. No new production binary is claimed.
Predeclared OLS timing controls were not run after rejecting/restoring runtime;
byte-identical runtime is preservation evidence, not a new OLS timing claim.
Static allocation inventory drift, constrained fits, nonretaining CG and prior
scientific comparison gaps remain open. Frozen77/16/111 definitions are unchanged.

## Receipt archive and next experiment

`receipts-content-addressed.zip`: **10,056 logical receipts**, 2,312 payloads,
11,333,134 bytes; SHA-256:
`6c1cddaf97d93d4afcdbb03f9bba6c9d824e66b90dcf004d8261d43fb366fad0`.
550 binary/large generated outputs and raw licensed logs are hash-recorded but
omitted; controllers, source snapshots, summaries, sanitized protocol evidence,
all failed attempts and source identities remain. Restore into a new directory
with `python3 restore_receipts.py <destination>`, then run
`python3 verify_restored.py <destination>`. All restored hashes and privacy checks
passed. Known public shared-reference paths and synthetic redaction-test literals
in source are distinguished explicitly from private run metadata.

Next compare better finite-control conditioning **within joint LSMR**, retaining
its FE core-pair/Schwarz preconditioner, against the same frozen development frames.
Test full finite blocks and canonical FE-equivalent representatives as at most two
coherent alternatives; account for weight/rank generation, coefficient mapping,
warm starts and memory. Preserve original-input certificates. Advance only a
candidate that keeps sparse/disconnected controls and removes meaningful hard-case
cost. Then address canonical weighted-frame construction and the large-N traversal
budget; the current ordinary two-FE calls alone cannot supply it. This is a testable
next hypothesis, not an accepted speedup or a generic request for more profiling.
