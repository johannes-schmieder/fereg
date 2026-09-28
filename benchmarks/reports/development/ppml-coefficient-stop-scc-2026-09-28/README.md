# Strict PPML stopping for small coefficients

The candidate repairs the independently proven original positive-outcome PPML
coefficient error. It tightens the shared finite-coordinate Newton movement
check, with no tolerance relaxation, routing change or added heap bank.
[ADR 0236](../../../../docs/adr/0236-ppml-small-coefficient-stopping.md)
describes the policy and its limits.

## Exact original input

The [preceding adjudication](../ppml-original-positive-scc-2026-09-28/README.md)
proves original and prior-current fereg wrong on x8, while strict pinned fixest
is accurate. Job **7763387** repeats the independent full-design oracle and
outward Decimal certificate on that exact original strong_d2 N61440/K10 input,
then checks the unchanged baseline and candidate. Original DTA and every column
hash match. Total rank 31497 and the full sample are independently established.

| Artifact/mode | Outer iterations | Reported inner iterations | Maximum coefficient error | Certified accuracy |
|---|---:|---:|---:|---|
| Baseline fast T4 | 4 | 99 | 1.3539153e-10 | x8 fails |
| Candidate fast T4 | 6 | 142 | 6.6353173e-17 | all pass |
| Candidate fast T1 | 6 | 142 | 5.3342747e-17 | all pass |
| Candidate low T4 | 4 | 166 | 1.2753199e-14 | all pass |

The repeated certificate radius is **8.9602791e-14**; minor anchor variation
between compute hosts stays enclosed. Every candidate also passes sample, rank,
robust covariance, SE, objective and convergence checks. This confirms fast
T1/T4 accuracy and the low-mode control; it does not establish that the previous
low-mode result was inaccurate. Native and fixest iteration units are different.

### Measured stopping attribution

The unmodified compiled artifacts emit diagnostic traces only for this accuracy
job; these traced clocks are never headline timings. At the original fast T4
iteration-four strict checkpoint, the old check accepts and the new check
rejects. The accepted predictor's relative movement is about **3.31e-9**, which
passes the old 1e-8 check. After committing the strict Newton correction, the
candidate's normalized score falls from the baseline's **8.47e-11** to
**3.98e-17**. It resumes the existing two-consecutive-step logic and publishes
at iteration six, with current-weight final products. This directly identifies
the stopping boundary; no different solver, starting point or sample is used.

Trace `beta_change` includes the absorbed normalization coordinate and is not
the active-slot publication statistic. Do not mistake its large gauge movement
for finite-coordinate error. The raw traces contain warmup and measured-input
fits; the final checkpoint sequence is the original-input fit.

The added check traverses K coordinates; it does not scan observations or build
geometry. A rejection can still cost extra IRLS and strict-certificate work.
The cost must be reported, not concealed by ranking the inaccurate baseline.

## Source, platform and controls

Baseline computational snapshot **4c3d7c44…**, plugin **a7e95e67…**. Candidate
snapshot **f2e37c75932a30d857efe3192394bac305decca68845412db186a6b24044f231**,
Linux plugin **7c01f4514454c1fb7f9804abb08ea3917ac98a67ef65d69ed9c3ea4083e792ec**,
Mac universal plugin **932d4fdca3a6ec9b99201948ae44b446a780fddca5d974cf8e8ff4707b168a47**.
Exactly one of 211 computational source files changes: `ppml/prepared_irls.rs`.
The frozen source manifests and deployment inventories bind the evidence.

Mac and Linux each pass **61 prepared-IRLS Rust tests** and **four licensed
oracle/control fixtures**: iterative canonical PPML, categorical final PPML,
three-partition slopes, and multipartition OLS. Linux also passes the PPML smoke;
both actual-plugin ABI/panic checks pass. The old dynamic-range test's obsolete
natural-rejection count fails on unchanged baseline too; explicit injection now
exercises its intended recovery boundary without weakening its analytical
accuracy or iteration bound. See `preexisting-test-adjudication.json`.

Build/validation **7763383**: failed=0, exit_status=0, four slots, 205 seconds,
maxvmem **11.576G** (includes compiler/test peak, not estimator memory). Accuracy
job **7763387**: failed=0, exit_status=0, four slots, 51 seconds, maxvmem **1.605G**.
Both have complete application sentinels and source/output hash receipts.
Directory-metadata transfer warnings did not affect collected bytes: every file
was independently compared to a fresh remote SHA256 inventory.

No new memory bank or admission change. Static allocation-inventory drift and
physical-memory qualification remain open. The separate original zero-outcome
PPML discrepancy is not adjudicated by this positive-outcome proof. Historical
NR and earlier source-bound performance results remain unchanged.

## Fresh performance cost and remaining work

The [fresh isolated rotation](fresh03/README.md), job **7763404**, completes all
81 headline applications, all 27 native pairs and 24/27 stock science pairs.
The three weak-connectivity stock SE discrepancies remain unranked and are not
yet assigned to either estimator. Three fixed rotations per case use fresh
seed2026092811 with unchanged stock accuracy profiles and matched work.

The accuracy repair costs 25.0% small T4 and 28.7% wide in paired medians; weak
costs 27.3%, serial 9.9%, and strong-d2 1.7%. The small-low and OLS controls are
1.000/1.005 of baseline. Remaining candidate/stock medians are 2.162 small T4,
1.728 serial, 1.664 low, 1.843 OLS, 0.798 wide, 0.423 three-FE, 0.339 strict slopes
and 0.360 strict strong-d2. Weak is NR. The full distributions, losses, phase
counts and memory observations are retained, including variation in unchanged
controls. These are new source-bound ratios, not updates to earlier receipts.

Accept the stricter stopping rule because the old one demonstrably published
an inaccurate coefficient. Do not describe it as a speed gain or claim the
performance objective is met. Reducing the repeated strict-correction and
finalization work is now a concrete next performance task: strict-final phase
medians roughly double on small, wide and weak cases. An efficient successor
must preserve a fresh Newton/score/objective witness and final Hessian at the
accepted state; it must not restore the inaccurate stopping rule or weaken its
coefficient envelope. The separate original zero-outcome discrepancy still
needs independent sample and optimum adjudication first. No universal 1.20
ceiling, 0.90-stock target, monotonic core scaling or full qualification claim.
