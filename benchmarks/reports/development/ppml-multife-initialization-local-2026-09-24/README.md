# Cold multi-FE PPML initialization and remaining opportunity

Local macOS arm64 development evidence, 2026-09-24. Adopt `multifeseed1` under
[ADR 0161](../../../../docs/adr/0161-categorical-multife-poisson-start.md).
One deterministic Poisson intercept-coordinate sweep avoids one or two full
outer iterations in the sampled categorical fits. Fresh remaining-loss cases
improve 5–19%; the per-case 0.90-times-stock objective is still unfinished.
Further work is scoped to **at least two FE partitions**, as requested.

## Source and receipts

Baseline `inferwork1` matches `dda2983f`, plugin SHA-256
`685bb0102fca0e95f6645c63449b3c8ba1ae383de8ac7a7634dab13f4a58d894`.
Candidate is that commit plus the captured `ppml/prepared_irls.rs` overlay.
All **545 computational inputs match** the adopted worktree. Candidate plugin:
`bbb28cd029cf0eb77e9f8a0b0c206412b663cb2e641fe050395152082f96a744`.
Build ABI/panic-containment checks pass. Separate diagnostics `multifeledger1`
and `multifeledger2` have complete captured sources/identities; neither supplies
headline times. Pinned unmodified fixest 0.14.2 archive SHA-256:
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.

The [content-addressed archive](receipts-content-addressed.zip) contains
**6,607 logical receipts**, 1,686 unique payloads, 12,707,907 bytes, SHA-256
`2b08dffd91a82e1b3969432cc21626acc0a8b17c937032b0bc35ec73ffe2f3f4`.
The [restore tool](restore_receipts.py) verified and restored all files under
`multife-initialization/`. Contents include exact computational sources, build/
validation recipes and logs, declarations, controller code, exports, numerical
audits, profiles and full timing distributions. Data and binary payloads and
raw licensed logs are excluded with hashes; machine-specific paths are sanitized.

All public clocks include robust inference and requested fitted means; stock
includes response prediction. Stock uses `glm.tol=1e-10`, `fixef.tol=1e-11`,
matched requested application threads and serial BLAS. Startup, generation,
import, warmup and exports are outside both clocks. Native low mode is not a
physical-memory-matched comparison. No compilation, profiling, archiving or
competing fits overlap headline runs; all monitors have empty error/overlap lists.

## Current-source inventory before the change

The bounded seed347 screen uses two repetitions per variant. This is an
opportunity screen, not a broad qualification or robust distribution estimate.
Milliseconds below are medians from that block. Do not mix them with later
paired-block baselines.

| Case | Baseline | Stock | Baseline/stock | Reduction needed to reach 0.90× stock |
|---|---:|---:|---:|---:|
| Two FE N12000/K3/T1 | 51 | 35.5 | 1.437 | 37.4% |
| Disconnected two FE N20160/K12/T4 | 84.5 | 66 | 1.280 | 29.7% |
| Unbalanced three FE N23040/K8/T4 | 101 | 81 | 1.247 | 27.8% |
| Low two FE N16320/K32/T8 | 148.5 | 88.5 | 1.678 | 46.4% |
| Low three FE N14400/K7/T4 | 97.5 | 123.5 | 0.789 | Already below target |
| Two slope partitions N16320/K6/T4 | 67.5 | 61 | 1.107 | 18.7% |

All 24 applications and their science checks pass. Other captured weak/sparse,
wide-slope and higher-dimensional results remain source-bound historical evidence
until remeasured. Unresolved comparisons remain unranked; this screen does not
resolve the documented large one-FE mean discrepancies, constrained-fit/CG span
failures or synthetic exact-fit three-FE LSMR refusal. New weights, wider factor
expansions and physical memory caps are not exhaustively timed here. Existing
77/16/111 benchmark definitions remain unchanged.

## Diagnostic attribution and chosen change

Two separate profile builds run these six cells once each. The first isolates
major phases; the second adds canonical construction and operator calls.
Nested exclusive intervals reconcile to the plugin root. More detailed clocks
perturb short operations: use them to localize costs, not as headline comparisons.
All twelve diagnostic applications pass their public-state audits.

Loading is only 0.5–3.2 ms and initial input preparation 0.7–4.0 ms in the first
profile. Narrow ordinary joint solves take 17.8 ms; disconnected 27.0 ms;
unbalanced three-FE 23.2 ms. Final phases take 7.8, 13.9 and 31.8 ms. Wide low
memory spends 40.6 ms in its final phase, within a 132.9-ms native root.

The deeper baseline profile finds 53 forward and 54 adjoint observation passes
for the narrow joint solver. Their finite/FE operator spans total 2.2/3.8 ms;
recurrence administration, preconditioning and fresh certification add real
work. Low K32 has 24.7 ms in eleven Schur projection calls, 18.9 ms in two full
rank constructions and 16.4 ms in six streamed reconstructions. These nested
figures must not be summed as if disjoint. The report does not infer that every
millisecond in an enclosing function is a required scientific check.

A cold multi-FE fit still starts from a global intercept. Instead of revisiting
ADRs 0155/0157's rejected solver wrappers, the candidate starts closer to the
Poisson optimum with one categorical coordinate sweep at zero nonconstant
finite coefficients. Each group update is the exact conditional optimum;
adding its constant preserves model space. It uses compensated current-prior-
weight moments, existing memberships and the existing predictor buffer.

No new heap or recurrence layer is added. Unusable sums, zero-outcome groups,
underflow of positive terms, overflow or bound violations restore the complete
global seed. Explicit starts, Mu exploration, slopes, one FE and OLS are unchanged.
The seed has no scientific authority: initial canonical rank, changing-weight
IRLS, two score/deviance passes, strict/canonical final construction, inference,
all certificates and failure atomicity remain unchanged.

## Whole-command confirmation

Three alternating baseline/candidate/stock rotations. Full observations are in
repetition order; ratios use medians of this same block. Development seed
202609347; fresh **202609374 was predeclared and is now consumed/frozen**.

### Development

| Case | Baseline ms | Candidate ms | Stock ms | Paired C−B ms | C/stock |
|---|---|---|---|---|---|
| Two FE N12000/K3/T1 | 51, 53, 51 | 48, 48, 48 | 34, 39, 39 | -3, -5, -3 | 1.231 |
| Two FE disconnected N20160/K12/T4 | 80, 82, 83 | 66, 69, 70 | 66, 66, 66 | -14, -13, -13 | 1.045 |
| Three FE unbalanced N23040/K8/T4 | 102, 101, 103 | 97, 97, 99 | 87, 80, 85 | -5, -4, -4 | 1.141 |
| Two FE N16320/K32/T8 low | 150, 153, 148 | 132, 133, 131 | 88, 91, 84 | -18, -20, -17 | 1.500 |
| Three FE N14400/K7/T4 low | 99, 100, 97 | 88, 91, 91 | 122, 117, 121 | -11, -9, -6 | 0.752 |
| Two FE slopes N16320/K6/T4 | 68, 68, 69 | 69, 69, 69 | 66, 66, 66 | 1, 1, 0 | 1.045 |

### Fresh

| Case | Baseline ms | Candidate ms | Stock ms | Paired C−B ms | C/stock |
|---|---|---|---|---|---|
| Two FE N13200/K4/T1 | 57, 59, 56 | 48, 49, 49 | 47, 47, 40 | -9, -10, -7 | 1.043 |
| Two FE disconnected N22080/K14/T4 | 94, 90, 99 | 76, 75, 78 | 73, 74, 71 | -18, -15, -21 | 1.041 |
| Three FE unbalanced N24960/K10/T4 | 121, 121, 121 | 110, 115, 118 | 92, 97, 97 | -11, -6, -3 | 1.186 |
| Two FE N18000/K28/T8 low | 134, 132, 135 | 117, 117, 116 | 82, 77, 82 | -17, -15, -19 | 1.427 |
| Two FE weak_slope_barbell N9600/K5/T1 | 41, 36, 42 | 38, 42, 42 | 838, 837, 839 | -3, 6, 0 | 0.050 |
| Two FE slopes N17280/K8/T4 | 75, 76, 77 | 75, 75, 77 | 88, 82, 88 | 0, -1, 0 | 0.852 |

### OLS controls

| Case | Baseline ms | Candidate ms | Stock ms | Paired C−B ms | C/stock |
|---|---|---|---|---|---|
| Two FE N10080/K8/T4 | 17, 17, 17 | 17, 17, 18 | — | 0, 0, 1 | — |
| Two FE slopes N10080/K6/T4 | 15, 15, 15 | 14, 15, 16 | — | -1, 0, 1 | — |

All 15 development categorical pairs and all 12 fresh remaining-loss target
pairs improve. Development medians improve 5.9%, 15.9%, 4.9%, 12.0% and 8.1%.
Fresh narrow/disconnected/three-FE/wide-low medians improve 14.0%, 19.1%, 5.0%
and 12.7%. The fresh weak case loses 1 ms at its median, with -3/+6/0-ms pairs;
its ratio remains 0.050. Excluded development slopes lose 1 ms; fresh slopes
improve 1 ms with unchanged computation. OLS medians tie, with individual
+1-ms pairs. These losses remain visible; no universal non-regression claim.

Fresh outer iterations change 6→4, 6→4, 5→4, 6→4, 5→4; slopes remain seven.
Corresponding recorded inner units are 60→33, 64→40, 51→39, 71→53, 38→33;
these mixed recurrence/batch units are not comparable stock iteration counts.
The first-repetition runtime records show about 0.2–0.8 ms extra preparation,
while ordinary projection work falls. Final canonical work remains and can
increase: fresh narrow final phase is 8.7→9.5 ms, disconnected 15.9→16.7.
Thus initialization does not make independent final construction free.

## Scientific evidence and memory

**48 distinct prepared-IRLS Rust tests pass** on the exact candidate, including
new separable Poisson truth and refusal/restoration tests, extreme components,
rank/basis changes, strict resume/final-Hessian, transaction rollback, tiny
user tolerances, allocation witnesses and warm starts. New truth tests prescribe
multiplicative group means and paired zero-score deviations independently of
the implementation, with two/three effects and weight scales 1e-50/1/1e50.
They also test group scores, model-space differences, objective improvement,
raw overflow, bounds and a partially completed sweep's complete rollback.

**Four licensed fixtures pass** with process status, explicit Stata rc and
terminal sentinel. The categorical full-dummy Poisson/Hessian/sandwich oracle
covers 24 combinations of two/three effects, unit/FW/PW, robust/clustered and
T1/T4, with offset and eta/mu/xb/absorbed/residual outputs. The low-memory
fixed-process allocation audit retains zero events/growth, stable capacities
and warm/fresh equivalence, and compares with fast public estimates. Multipartition
slope and one-FE dummy oracles protect exclusions. No tolerance was edited.

Across inventory and paired screens, **144 headline applications** and
**132 public PPML states** pass. All 36 native PPML pairs and 36 direct
candidate-stock comparisons pass the existing envelopes. Maximum native
absolute differences: coefficients 2.666e-10, covariance 3.391e-15, means
8.469e-8, log likelihood zero, deviance 3.638e-12. These are inside the unchanged
registered envelopes, not revised thresholds. Six OLS pairs retain exact
scientific exports. No application failures, timeouts, scientific exclusions
or contention occurred in this milestone.

Modeled selected/scratch bytes are identical in all 36 PPML pairs. No initializer
workspace is added. Tracked peaks differ -1,327,056 to +25,136 bytes, with 18
exact pairs; altered recurrence histories change temporary allocation peaks.
Physical-cap/platform confirmation and the documented static allocation source
inventory drift remain open. This is development evidence, not qualification.

## Remaining budget and next subsystem

The four fresh loss cases now need the following further reductions:

| Case | Current / stock ms | 0.90× stock target | Remaining saving |
|---|---:|---:|---:|
| Narrow two FE | 49 / 47 | 42.3 | 6.7 ms (13.7%) |
| Disconnected two FE | 76 / 73 | 65.7 | 10.3 ms (13.6%) |
| Unbalanced three FE | 115 / 97 | 87.3 | 27.7 ms (24.1%) |
| Wide low two FE | 117 / 82 | 73.8 | 43.2 ms (36.9%) |

Move to **canonical full-design construction and streamed reconstruction**,
starting with wide low-memory and unbalanced three-FE. Their fresh final native
phases still cost about 32 and 36 ms; the source-bound deep baseline profile
identifies repeated Schur/LSMR projection, rank products and streamed reconstruction.
Those are concrete cost budgets, not proof that a particular cache is valid.
Separate immutable sample/topology from current-weight factors and fresh
certificates; reuse only current-frame dependencies. First compare identical
frozen batches and count geometry setup, row scans, scratch reads and certificate
passes before implementing a coherent layout/ownership change.

Retain the new initialization without sweep-count or seed-specific tuning.
Preserve weak/sparse wins, slope engines and OLS. Do not reopen the rejected
coefficient wrappers, resume old serial-slope micro-tuning or infer earlier
artifact wins for this source. No push, release, broad qualification, stopped
campaign restart or SCC authentication retry is authorized by this evidence.
