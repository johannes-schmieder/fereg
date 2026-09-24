# Categorical incremental coefficient closeout and current one-FE opportunity

Development evidence, 2026-09-24. Production baseline is `e9c7f598`, artifact
`componentwarm3`, plugin SHA-256
`5434193a5a9b0408ed9636c625f1d134996f2224bff2642049b129a47245ce52`.
All 543 captured computational inputs match that source. No prototype below
is adopted. See [ADR 0157](../../../../docs/adr/0157-incremental-coefficient-engine-closeout.md).

## Scope and receipts

Development seed **202609347**, matched robust inference and requested means,
matched T1/T4, serial BLAS. Stock fixest 0.14.2 is the pinned unmodified library,
with `glm.tol=1e-10`, `fixef.tol=1e-11`. Its exact package and source identities
are in the receipts. Diagnostic libraries are separately labeled and are never
stock headline comparators. Headline fits run sequentially without compilation,
profiling, archiving or competing fits; controllers and monitors are retained.
No new frozen validation seed was consumed. Frozen 77/16/111 definitions and
protected/consumed seeds remain unchanged. This is not qualification.

- [Content-addressed experiment archive](receipts-content-addressed.zip):
  10,060 logical receipts, 1,824 unique payloads, 12,177,760 bytes; SHA-256
  `58970138922c04c29bcb53543b631176065b830f6e6495dbbc00fae93f4090b5`.
  [Restore tool](restore_receipts.py) verified and restored every logical file.
- [One-FE screen archive](one-fe-screen-receipts.zip): 642 receipts,
  6,984,680 bytes; SHA-256
  `2c7c98df9b43e22a115c9c5164431adf2901aa72c69a03e87dad9ccd95f242ba`.
  Every archived payload was hash-verified against its manifest.

The first archive restores under `incremental-coefficient/`. It includes exact
computational source maps for baseline and all measured diagnostic/prototype
artifacts, source payloads, recipes, build/test results, public exports, audits,
profiles and controllers. Data/binary hashes preserve identity without committing
datasets, plugins or licensed logs. Raw licensed logs are omitted, with hashes.
Local paths are sanitized. The first failed `incrementalcoeff1` build is recorded
and excluded: its temporary builder had an invalid git-archive cwd/pathspec.

## Candidate design and source binding

| Artifact | Change | Plugin SHA-256 |
|---|---|---|
| incrementalcoeff2 | Joint coefficient Newton increment through generic reusable MAP owner | c35773008dd69b043c9102b9eba5e406cdb17c4ac9eb428a678838ae769b7e96 |
| borrowcoeff1 | Numeric-only owner; borrow topology and original controls; independent categorical group/finite certificate | 16f983d3fb922487977250c4776439ac7ee4050b9a45329bfc64ab9ff57706c1 |
| borrowcoeff2 | Column-wise original-control access | 1c3b3081db27ffecc31af399219ad0b68d6965f4757ce90b7c7ae45d4fe0b7ff |
| borrowcoeff3 | Guarded reconstruction normalization hoist using existing scratch | a22ca9c923675a46ed9882fb3dfc63437d44b2e9b090ce57e3677a05ecd03a56 |

The initial candidate changes automatic two-FE directions from joint LSMR to the
existing joint finite/FE coefficient engine. It solves the increment target
`y/mu-1`, preserving independent canonical response history, initial rank,
strict solving and final inference. Its generic owner duplicates topology and
controls and performs both generic FE and original finite/FE acceptance.

The borrowed successors retain only numeric workspace and original-control
identities. They refresh weighted group factors and finite/FE coupling for each
call, borrow immutable topology and controls, and return a candidate rather
than a certified linear result. Independently accumulated original finite and
group moments certify the increment. For intercept-only groups the latter is
the complete local rank-one span check. Full canonical checks remain separate.
The proof does not apply to slopes. No changed-weight authority is reused.

The last candidate hoists representable normal coefficient/scale quotients from
observation reconstruction, with the previous order retained on exceptional
quotients. It uses existing beta scratch. This has not been measured as an
independent kernel improvement and is not adopted separately.

## Whole-command results: all losses retained

Each main cell has three alternating baseline/candidate/stock rotations.
Numbers below are medians in milliseconds; individual observations, exports,
orders and checks are in each evaluation's `summary.json` and audit files.

| Candidate | Dense N16320/K16/T4: base → candidate / stock | Unbalanced N15360/K10/T1 | Narrow N12000/K3/T1 | Weak N7680/K4/T1 |
|---|---|---|---|---|
| incrementalcoeff2 | 77 → 75 / 65 | 69 → 70 / 73 | 51 → 46 / 41 | 37 → 35 / 426 |
| borrowcoeff1 | 78 → 78 / 65 | 69 → 68 / 69 | 51 → 42 / 36 | 32 → 33 / 427 |
| borrowcoeff2 | 81 → 79 / 65 | 66 → 68 / 75 | 49 → 47 / 41 | 30 → 32 / 431 |
| borrowcoeff3 | 77 → 79 / 64 | 70 → 72 / 68 | 54 → 44 / 35 | 35 → 33 / 428 |

These are distinct time blocks, not four directly paired candidates. The final
candidate's exact distributions and paired candidate-minus-baseline deltas:

| Case | Baseline ms | Candidate ms | Stock ms | Paired delta ms |
|---|---|---|---|---|
| Dense | 77, 77, 78 | 79, 79, 77 | 64, 65, 62 | +2, +2, -1 |
| Unbalanced | 71, 66, 70 | 76, 70, 72 | 75, 68, 66 | +5, +4, +2 |
| Narrow | 54, 55, 54 | 46, 44, 43 | 35, 40, 34 | -8, -11, -11 |
| Weak | 35, 33, 38 | 35, 33, 32 | 428, 437, 428 | 0, 0, -6 |

Final narrow improvement is 18.5%, but candidate/stock remains 1.257. Dense
and unbalanced ratios are 1.234 and 1.059. The weak baseline already wins
substantially. Final small pilots lose +4/+2/+1 ms, including the excluded
three-FE control (+2). Earlier column-wise dense deltas include a baseline
95-ms outlier and an apparent -18-ms pair; the other two pairs are only -1 ms.
Do not interpret that outlier as a systematic gain.

## Whole-command attribution and memory

Separate instrumented baseline/prototype clocks cover four families, one fit
per artifact/cell. Nested times below are not additive headline timings.

For the initial coefficient wrapper, baseline joint LSMR versus complete new
direction time is 21.234 → 18.240 ms (dense), 18.333 → 17.959 (unbalanced),
17.018 → 11.746 (narrow), and 5.570 → 6.339 (weak). Dense inner coefficient
work is 10.724 ms, versus 18.240 for the complete checked direction. Frozen
kernel wins alone overstate public benefit.

In a separate initial-wrapper/borrowed-owner profile pair, dense reconstruction
increases 3.190 → 9.079 ms and complete direction time 17.766 → 24.012;
unbalanced reconstruction increases 1.926 → 4.867 and complete direction time
19.266 → 22.329. Borrowing eliminates copies and redundant categorical checking,
but its access costs consume those savings. Column-wise access and quotient
hoisting do not fix the broad public result. Attribution beyond measured spans
is an inference, not an operation-level proof of hardware behavior.

Tracked peak bytes, representative final borrowed layout:

| Case | Baseline | Borrowed candidate |
|---|---:|---:|
| Dense | 18,266,460 | 16,764,632 |
| Unbalanced | 11,930,420 | 11,539,388 |
| Narrow | 7,703,335 | 6,709,957 |
| Weak | 3,992,683 | 4,248,667 |

Modeled selected bytes fall from 70,222,541 to 19,326,133 in dense, and from
24,612,423 to 12,419,519 in narrow. The large modeled difference mainly removes
LSMR's conservative allowance; it is not measured physical savings. Small/weak
tracked increases remain visible. No constrained physical-memory claim follows.

## Scientific checks and limits

All 180 public prototype-screen applications and 16 separate diagnostic
applications complete without a final application failure or timeout. All
public-state and pairwise checks pass for this categorical experiment, including
means. Ten licensed fixture executions cover four distinct fixture contents:
incremental categorical explicit-design Poisson/Hessian, canonical explicit
LSMR, dense slope products and streamed low-memory joint products. Unit/FW/PW,
robust/clustered, T1/T4 and requested-output checks are retained.

Across relevant prototype snapshots, 86 distinct Rust tests pass. **59 run on
the exact final borrowcoeff3 source**: new mathematical direction test, 46
prepared-IRLS tests and 12 MAP tests; the unrelated frozen MAP probe is ignored.
The initial borrowed artifact has 85 existing checks on its own source; the new
mathematical test was added afterward and is not claimed for that binary.

The new analytic WLS test has 160 rows and paired contrasts with known finite
coefficients and residuals. It covers 36 frames: two/three FE, disconnected
components, T1/T4, three relative weight patterns and common scales 1e-100,
1 and 1e100. Dense/original-view scores agree, the independently executed full
FE-space certificate passes, corrupted residuals fail, and invalid weights
revoke state before a valid subsequent frame recovers. Existing strict-resume,
rank, Hessian, memory and failure tests remain applicable.

No production source changes, broad OLS Stata campaign or qualification occurred.
Shared MAP tests and an excluded three-FE control are useful focused protection,
not universal regression evidence. The prototype's numerical fallback, budget
refusal and zero-active-control policy are incomplete. It is rejected rather
than left as another automatic solver layer.

A separate post-hoc zero-Newton diagnostic on unchanged baseline outputs finds
strict normalized moments 3.202254e-12, 5.144742e-12, 4.072096e-12 and
2.080526e-11. All fail the existing 1e-12 bound in all three repetitions.
No shortcut or relaxed tolerance is implemented.

## Current-source one-FE opportunity screen

The separate archive records unchanged `componentwarm3` versus strict stock,
three rotations, robust + means, seed 347. All 24 applications and public-state
checks pass. Two families fail cross-estimator mean envelopes and remain
**unranked**, despite passing coefficient, covariance, objective and own-state
checks. These are scientific discrepancies, not timing wins or losses.

| Case | fereg ms | Stock ms | Interpretation |
|---|---|---|---|
| Fast N16320/K12/T4 | 45, 43, 43 | 28, 27, 27 | Comparable; median ratio 1.593; 0.90 target 24.3 ms |
| Fast N61440/K10/T4 | 99, 95, 96 | 54, 55, 51 | Unranked: five mean coordinates fail |
| Low N49152/K12/T4 | 87, 84, 82 | 54, 54, 54 | Unranked: five mean coordinates fail |
| Unbalanced fast N30720/K3/T1 | 54, 59, 57 | 25, 27, 26 | Comparable; median ratio 2.192; 0.90 target 23.4 ms |

The large fast/low maximum absolute mean differences are 7.137633e-7 and
7.574376e-7; maximum envelope ratios 1.6112 and 1.7057. All repetitions reproduce
them. This report does not adjudicate their origin or grant a reference exception.

Two additional instrumented baseline fits cover the first and second rows,
using `incrementbaseledger1` (plugin
`0fda0dbf453f871f242de604025ecee4068c7505192d28318cc234c934c3d5ab`).
Every exclusive span subtracts the union of nested children; totals reconcile
to the native root. Diagnostic counts/times:

| Phase | N16320/K12: calls / exclusive ms | N61440/K10: calls / exclusive ms |
|---|---:|---:|
| Poisson objective with terms | 7 / 2.363 | 6 / 7.818 |
| Weighted projection column norms | 6 / 1.339 | 6 / 4.551 |
| Original-input outer score | 6 / 1.278 | 5 / 3.095 |
| Concentrated direction preparation | 3 / 1.038 | 2 / 1.980 |
| Analytic group profiling | 6 / 1.045 | 4 / 2.250 |
| Dense finite products | 2 / 1.146 | 2 / 4.428 |
| Native loading | 1 / 1.884 | 1 / 4.895 |
| Inference | 1 / 0.767 | 1 / 1.768 |

Native totals are 31.438 and 84.261 ms. Canonical initial solves take 6.371 and
16.445 ms inclusive; strict solves 6.220 and 13.944 ms inclusive. These overlap
the exclusive rows and must not be added to them. Several milliseconds also
remain in enclosing IRLS/estimator spans. The data support lifecycle redesign,
not another blind replacement of the inexpensive concentrated Newton kernel.
Public/native differences are not exclusively ado overhead without more clocks.

Production remains `e9c7f598`. The next implementation should reduce repeated
one-FE preparation/certification passes and whole-command overhead, then return
to categorical and narrow-slope losses with measured cost budgets. Success is
per-case public time at most 0.90 of comparable stock, with all science intact;
this report does not claim that objective has been met.
