# PPML weight-frame geometry and ordered parallel certificates

Local macOS arm64 development milestone, September 23, 2026. The accepted
`slframe3` artifact is based on `54cc82bc`; all **531 computational inputs**
match the final source. Plugin SHA-256:
`c5e41cef298abced05af47874afc957eb4e6ffa0e5ad13174e58d3e7624c1b5a`.
The baseline is the preceding accepted `slschur3` artifact, plugin
`ed5e7855efbea24863d6ab9d6c0dcecf260459339be75563d19feaa0bd689467`.
This is one implementation milestone toward the user's broader 0.90-times
stock-runtime target, not completion of that objective or qualification.

## Adopted changes

[ADR 0124](../../../../docs/adr/0124-ppml-frame-geometry-and-parallel-certificates.md)
extends the admitted multipartition PPML coefficient solver with two mechanisms:

- Fresh group certificates share row loads in bounded 16-RHS tiles, run groups
  on the existing native team, and merge results in the original order. Every
  scalar entry keeps its arithmetic, observation order and compensation.
- The owning linear session supplies explicit sample/topology/weight generations.
  Equal generations authorize reuse of local weighted QR across compatible
  response/control/checkpoint calls. New generations rebuild. Unframed calls,
  validation failures, solve failures and unwinds clear authority before reuse.

No coefficient solution, residual, score or acceptance result is reused.
Reconstruction and fresh final certificates remain mandatory on every call;
rank, finite solve, model space, objective, score, exact-final-Hessian,
inference and publication policies are unchanged. Categorical and one-effect
PPML, mu separation, explicit non-MAP families and public OLS routing are excluded.

The parallel result bank is separately optional: a budget that admits the
Schur solver but not the extra bank keeps serial fresh certification. This
supersedes the first candidate's combined admission, which could have displaced
an affordable solver at a tight budget. Refused allocations also keep the solver.

## Source-bound public timings

Three uncontended rotations per cell, baseline/candidate/pinned stock, same
samples, threads, robust covariance and timed fitted-mean output. Entries are
milliseconds, median [min,max]. Data generation/import, process startup, warmup
and exports are outside the command timer; native SPI loading, preparation,
estimation, inference and publication are inside. No compilation, profiling,
archiving or competing fit ran with headline measurements. Mac scheduling and
frequency are uncontrolled; old checkpoint medians are not substituted for the
contemporaneous baseline.

The unchanged pinned fixest 0.14.2 archive SHA-256 is
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Stock calls use `glm.tol=1e-10, fixef.tol=1e-11`, effective matched threads,
single-thread BLAS and the existing matched robust-covariance normalization.
Diagnostic native builds supply no headline clocks.

Fresh seed **202609312**:

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Two slope partitions, N30720 K6 T4 | 219 [216,220] | 187 [186,191] | 118 [111,118] | 1.58 |
| Unbalanced three FE, two slope partitions, N30720 K6 T4 | 214 [212,215] | 180 [176,182] | 120 [118,120] | 1.50 |
| Larger unbalanced three FE, N61440 K10 T8 | 491 [489,503] | 385 [377,388] | 232 [227,234] | 1.66 |
| Two slope partitions, N30720 K6 T1 | 318 [312,324] | 308 [300,310] | 315 [314,322] | 0.98 |
| Unbalanced slopes, N30720 K6 T4, native low memory | 407 [395,409] | 357 [349,364] | 118 [118,119] | 3.03 |
| Multiple slopes, N7680 K6 T4 | 73 [73,74] | 64 [64,66] | 53 [49,54] | See accuracy limitation below |

Candidate/baseline median reductions are **14.6%, 15.9%, 21.6%, 3.1%, 12.3%
and 12.3%**, respectively. Native low memory versus ordinary stock is not an
equal-budget comparison. Every large-cell pairwise and public-state audit
passes, including the multiple-slope row. A fresh smaller multiple-slope stock
oracle failure prevents an unqualified family-level speed ranking; its large
stock clocks above are descriptive.

The four-way tuning screen, seed **202609311**, isolates the mechanisms:

| Case | Baseline | Certificate only | Certificate + frame | Stock |
| --- | ---: | ---: | ---: | ---: |
| Two partitions T4 | 230 [230,233] | 206 [203,207] | 200 [199,202] | 119 [115,121] |
| Larger unbalanced T8 | 490 [486,493] | 386 [381,387] | 378 [371,379] | 223 [219,227] |
| Two partitions T1 | 332 [324,336] | 321 [318,332] | 322 [318,325] | 321 [313,326] |
| Low memory T4 | 405 [389,406] | 369 [369,375] | 366 [346,374] | 118 [117,118] |

The final candidate adds separate optional-bank admission and formatting after
this screen; fresh timings above are bound to that final source. Serial cache
benefits are small; no threshold tuning followed.

## Measured attribution and remaining work

Isolated baseline/candidate diagnostic builds use identical seed311 data. Times
nest; absorption includes geometry and certification, so these are not additive
whole-command components.

| Design | Absorptions / full certificates | QR preparations, old/new | Geometry ms, old/new | Certificate ms, old/new | Absorption ms, old/new |
| --- | ---: | ---: | ---: | ---: | ---: |
| Two partitions T4 | 13 / 13 | 13 / 10 | 23.55 / 18.31 | 39.58 / 12.54 | 120.90 / 87.96 |
| Larger unbalanced T8 | 11 / 11 | 11 / 8 | 27.70 / 19.78 | 133.40 / 19.91 | 283.86 / 158.88 |
| Low memory T4 | 19 / 19 | 19 / 8 | 24.32 / 10.04 | 43.28 / 14.62 | 161.47 / 116.17 |

The wide certificate is 6.7 times faster without removing a check. Identical
native exports and certificate arithmetic show this is scheduling and reuse,
not a change in accepted numerical work. More certificate tuning cannot close
the remaining gap: candidate wide absorption still has roughly 119 ms outside
geometry/certification on this diagnostic. The next solver investigation is
prepared cross-partition coupling operators and current-weight coefficient
warm starts, comparing compact and observation-based work on identical frames.

A representative fresh wide call reports 369 ms native time: preparation 32,
ordinary projection 160, finite solving 33, objective/line search 31, strict-final
work 76, inference 1.3 and publication 9.8 ms. A fresh low-memory call reports
344 ms: ordinary projection 98, finite solving 104, strict-final work 81,
inference 16, preparation 14 and publication 5.7 ms. These are per-call native
phase measurements, not a decomposition of the median public timers. The
low-memory finite path deserves its own redesign; caching alone is insufficient.

## Categorical inventory, difficult designs and OLS controls

Fresh seed **202609313**, same PPML robust/mean request:

| Categorical PPML | Baseline | Candidate | Stock |
| --- | ---: | ---: | ---: |
| Dense three FE, N30720 K6 T4 | 190 [188,192] | 196 [195,197] | 239 [236,240] |
| One FE, N61440 K10 T4 | 150 [147,151] | 145 [144,147] | 54 [53,54] |
| Degree-six two FE, N61440 K10 T4 | 234 [231,234] | 229 [228,235] | 139 [136,139] |
| Unbalanced three FE, N30720 K10 T4 | 178 [178,180] | 181 [178,182] | 110 [109,117] |

All comparisons/public states pass; native exports are exact. This change does
not optimize these excluded paths. Dense three FE already clears the requested
10%-ahead median target, while one FE remains 2.69 times stock and the other
two rows about 1.65 times stock. These current timings replace historical
opportunity estimates only for their captured artifact and requests. Profiled
one-FE Poisson optimization is a separate promising architectural candidate.

The dense categorical control's initial 3.2% native loss was repeated once in
five fresh process pairs, same frozen input: baseline **190 [189,197]** versus
candidate **193 [189,197]** ms. Retain the small 1.6% median loss; overlapping
ranges do not establish zero regression. No guard or algorithm was tuned to it.

Fresh seed **202609317**, three rotations and independent Poisson/Hessian checks
on the same bounded N960 K3 T4 designs:

| Design | Baseline | Candidate | Stock |
| --- | ---: | ---: | ---: |
| Weak slope barbell | 15 [14,15] | 14 [14,15] | 23 [23,24] |
| Disconnected three-FE slopes | 44 [43,44] | 42 [42,43] | 756 [754,758] |

Both estimators pass the independent checks here. These cells clear the 0.90
median ratio target, but do not adjudicate the older larger disconnected stock
covariance failure or supply universal topology claims.

Four excluded OLS controls, seed **202609315**, have baseline/candidate medians
68/67 ms for multipartition slopes, 43/44 for categorical three FE, 17/18 for
one-FE robust/full outputs, and 54/50 for generated-factor robust OLS. Every
scientific export is exact. Five additional pairs on the one-FE saved-output
control give **17 [16,18] / 17 [16,18]** ms. The categorical OLS 1-ms loss stays
visible; this bounded screen does not prove universal absence of regressions.

## Science, memory and visible failures

**34 distinct focused Rust tests** pass on final source. They cover independent
analytic weighted projections, duplicate/zero/slope-only channels, changing
weights, exact tiled/serial certificate equality across widths 1/7/16/17/33
and threads 1/4, weight scales 1e-60/1/1e60, generation reuse/invalidation,
error recovery, unchanged capacities, strict rollback/rank/frame checks, and
optional-memory boundary preservation. The actual-plugin ABI/panic-containment
harness and three licensed fixtures pass: independent multipartition weighted
Poisson/current-Hessian robust/cluster inference, saved slopes, and separation.

Fresh N1200 seed316 independent explicit-design Poisson/Hessian pilots pass for
both native artifacts in all three designs. Stock passes the two-partition and
unbalanced-three-FE pilots. The multiple-slope stock pilot fails five covariance
coordinates: maximum error **7.40828e-10**, tolerance ratio **1.57822**. Its
coefficients, means, objectives, scores and model space pass. Native covariance
error against the independent oracle is 4.03e-16. Preserve this discrepancy as
unranked; no tolerance, reference pin or inference definition was changed.
The larger N7680 multiple-slope pairwise/public checks pass, without resolving
this smaller independent-reference failure.

Across final, screening and confirmation blocks, **88 native pairs** retain
exact scientific exports and requested means. There are **254 benchmark
attempts: 242 application successes and 12 stock setup failures**, no timeout,
no detected competing fit/build and no monitor error. Scientific verdicts are
separate from application success. The initial controller invocation omitted
`STATA_MP_PATH` and stopped before launching a fit. The first replay omitted
existing R dependency paths, causing the 12 stock `jsonlite` setup failures;
corrected four-way rotations preserve those failed receipts. The first N2400
independent pilot exceeded the unchanged 1200-FE-column oracle limit. Its
successful applications and refusal remain; smaller fresh pilots supply the
bounded oracle evidence. A test compilation used a nonexistent helper name;
the corrected final tests pass. A summary-only runtime-key typo occurred after
writing the control audits and does not alter any estimator receipt.

The extra tracked peak on the six fresh slope cases is **0.0406–0.3779 MB**
(decimal), exactly matching the corresponding modeled increments. The bank is
admitted throughout ordinary and strict linear work and released before
inference/recovery. This is not a whole-action allocation proof or physical
memory-cap qualification. The static allocation inventory drift, constrained-fit
failures, nonretaining-CG instability, pending Linux/SCC checks and earlier
reference discrepancies remain open. Frozen 77/16/111 definitions are unchanged.
No push, release, SCC authentication retry or stopped campaign was initiated.

## Receipts

The immutable archive contains declarations, exact artifact/source identities,
controller snapshots, all successful and failed application receipts, numerical
exports, scientific audits, test results and separate diagnostic protocols.
Raw licensed logs, data, binaries and credentials are excluded. Original and
sanitized content hashes are retained. The restoration script verifies the
container and every logical receipt before writing them into a new directory.
The archive contains **7,518 logical files** in **1,649 distinct payloads**;
SHA-256 `940650d9cb08ef812efd07bbea8491582d10f97319ad7d2e159421e33413c97c`.
Restoration was verified. Omitted synthetic data and saved-mean binary exports
have separate hashes in `omitted-binary-hashes.json`; their numerical comparisons
are retained in the scientific audits.
