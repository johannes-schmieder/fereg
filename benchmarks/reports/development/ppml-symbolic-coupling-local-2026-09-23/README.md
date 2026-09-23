# PPML symbolic group-pair coupling and compact coefficient coordinates

Local macOS arm64 development milestone, September 23, 2026. Accept `slsymbol2`,
based on `a2147baf`; all **532 computational inputs** match the final source.
Plugin SHA-256:
`f86fd3e0b9864874df5a5700233e7e5a1b9bce347181d4772e07982db6159740`.
The contemporaneous baseline is accepted `slframe3`, plugin
`c5e41cef298abced05af47874afc957eb4e6ffa0e5ad13174e58d3e7624c1b5a`.
This completes a bounded implementation milestone, not the broader objective
of public PPML at most 0.90 times comparable stock fixest or release qualification.

## Adopted organization

[ADR 0126](../../../../docs/adr/0126-ppml-symbolic-coupling.md) extends the current
coefficient Schur solver. Prepare group-pair sparsity, sorted adjacency and
observation-to-block mappings once per immutable topology. Refill numerical
products only when current-weight QR geometry changes. Compact application uses
small diagonal/cross-partition blocks and group-local C/C-transpose work; it has
no dense D construction bank. Small remaining systems may form explicit Schur.
A predeclared 20% structural operation margin selects these optional products;
there are no benchmark-name/seed guards or post-holdout threshold changes.

Remove eliminated-partition coordinates from the four CG vectors, reuse one
group's scratch per worker, and calculate the fresh all-partition moment bound
without storing eliminated coordinates. RHS convergence remains independent.
Prepared cancellation-small curvature falls back to the observation operator.
Original-RHS reconstruction and the full fresh residual certificate remain
mandatory, as do all rank, finite-solve, score, model-space, objective, strict
final-Hessian, inference and publication conditions. No scientific tolerance,
weight/sample/DoF/separation policy or final-check schedule changes.

The operator is admitted after the numerical solver and ordered certificate
bank. Refusing its optional budget/storage retains those mechanisms. All
construction and retained capacities are charged conservatively. Numeric reuse
requires the session's explicit sample/topology/weight generation authority;
errors and unframed calls revoke it. Categorical, one-effect, mu separation,
explicit non-MAP and public OLS routing remain excluded.

## Final fresh public timings

Predeclared seed **202609319**, three alternating baseline/candidate/stock
rotations per cell. Entries are milliseconds, median [minimum, maximum]. Timers
include the public command, native loading/preparation, estimation, robust
inference, fitted means and publication. Generation/import, process startup,
warmup and exports are outside. No build, profile, archive or competing fit ran
alongside headline measurements; all overlap/monitor records are clear.

Stock is the unchanged pinned fixest 0.14.2 archive, SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched application threads,
single-thread BLAS and the existing comparable robust-covariance normalization.
Native diagnostic builds contribute no headline clocks.

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Two slope partitions, N30720 K6 T4 | 189 [188,190] | 161 [160,162] | 118 [114,123] | 1.36 |
| Unbalanced three FE, slopes on two partitions, N30720 K6 T4 | 191 [190,193] | 178 [177,181] | 113 [113,118] | 1.58 |
| Larger unbalanced slopes, N61440 K10 T8 | 376 [375,384] | 367 [358,369] | 226 [225,231] | 1.62 |
| Two slope partitions, N30720 K6 T1 | 292 [292,301] | 208 [205,210] | 319 [319,325] | 0.65 |
| Unbalanced slopes, N30720 K6 T4, native low memory | 393 [389,401] | 358 [355,375] | 119 [119,121] | 3.01 |
| Multiple slopes, N7680 K6 T4 | 61 [59,61] | 55 [55,55] | 55 [54,55] | 1.00 |
| Four FE, slopes on two partitions, N7680 K6 T4 | 99 [99,100] | 67 [65,67] | 207 [206,207] | 0.32 |

Native median reductions are **14.8%, 6.8%, 2.4%, 28.8%, 8.9%, 9.8%, 32.3%**.
All large-cell public states and pairwise checks pass. Serial and four-FE cells
clear the requested median target. These are bounded cell results, not universal
family claims. In particular, the fresh smaller two-partition stock pilot below
fails its independent covariance oracle and remains unranked. Native low memory
versus ordinary stock is not an equal-budget comparison. Wide gains are small;
this report does not hide that result behind the larger serial/four-FE gains.

The preceding symbolic candidate `slsymbol1` retained full coefficient vectors.
Its development-only seed318 baseline/candidate/stock medians were 184/167/116,
180/168/118, 387/362/228 and 357/325/116 ms for the first, second, third and
low-memory designs. Those source-bound results motivated compact coordinates;
they are not substituted for final-source timings. Plugin:
`3e4eb5cd6f2e1a7766521716de7ba485384b702c50239673d90ed30be31860cc`.
All four gains and distributions remain in the receipt archive.

## Attribution and remaining bottlenecks

A separate instrumented final-source build uses exactly the seed318 input hashes
from the rejected prepared-operator diagnostic. It reports:

| Case | One-time symbolic ms | Numeric frames | Numeric preparation ms | Prepared products / observation fallback | Absorption ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Two FE T4 | 0.45 | 9 | 10.96 | 413 / 0 | 60.16 |
| Unbalanced three FE T4 | 0.96 | 8 | 5.27 | 334 / 0 | 66.62 |
| Wide unbalanced T8 | 2.01 | 8 | 11.28 | 540 / 0 | 150.40 |
| Low memory T4 | 0.94 | 8 | 5.14 | 399 / 0 | 91.45 |

Two FE uses explicit Schur; the other three use compact block products. On the
wide input, numeric preparation falls from the rejected implementation's
48.62 ms to **11.28 ms**, plus 2.01 ms once for topology. The new block refill
accounts for 9.52 ms and local diagonals 1.76 ms. This directly addresses the
rejected preparation organization; faster applications alone were insufficient.

Wide absorption includes 19.67 ms QR geometry, 19.25 ms full fresh certificates
and 11.28 ms product preparation. Roughly 100 ms remains elsewhere in absorption.
Instrumentation counts 92 reconstruction checks, 247.73 summed worker-ms in
products and 67.61 summed worker-ms in reconstruction. Summed parallel worker
costs are not elapsed wall time and must not be added to the nested phase times.
The diagnostic plugin is
`e8eb13f00a6820e093e2a7fb230d00363df6047c60d08033414cbbcc76e5bb3f`.

On a final fresh wide application, native time is 352 ms: preparation 34.5,
ordinary projection 150.5, finite solving 23.8, objective/line search 31.4,
strict-final work 74.0, inference 1.5 and publication 9.9 ms. Low memory reports
341 ms: ordinary projection 92.5, finite solving **114.8**, strict-final work
**70.6**, inference 15.0, preparation 14.6 and publication 5.6 ms. These are
individual native phase records, not a decomposition of median public clocks.

The remaining gap cannot credibly be assigned entirely to certificate geometry
or prepared-operator setup. Further slope work should share block loads across
RHSs and examine reconstruction plus ordinary/strict/final repeated work. The
low-memory finite/strict path is a separate large bottleneck. The one-FE
categorical control below independently supports a profiled-Poisson redesign.

## Science, controls and failures

**36 distinct focused Rust tests** pass on final source: analytic independent
weighted projection, duplicate/zero/slope-only/rank-deficient channels, changing
weights and generations, extreme common weight scales, T1/T4, independent
operator products, capacity/admission boundaries, finite/failure behavior,
strict rollback and session checks. The actual-plugin ABI/panic harness passes.
Six final frozen-frame calls (three frames at T1/T4) pass independent explicit
WLS, model-space and fresh local-projection checks. They include the formerly
failing disconnected frame. Final frozen outputs are byte-identical across
thread counts and to the preceding symbolic candidate.

Three final licensed fixtures pass explicit Stata rc=0, process rc=0 and terminal
sentinels: multipartition slope Poisson/current-Hessian oracles (unweighted,
frequency and probability weights, robust/cluster inference and offsets), saved
slopes, and separation. The same three also passed on `slsymbol1`.

Six fresh independent public pilots cover two/three/four FE, multiple slopes,
weak connectivity and disconnected components. Both native artifacts pass all
six independent Poisson/Hessian oracles and public-state checks. Stock passes
five. Its N1200 two-partition pilot fails four covariance coordinates: maximum
absolute error **5.38175493e-10**, tolerance ratio **1.33073305**. Native covariance
error is 3.84e-17. Stock coefficients, means, objective, stationarity and model
space pass. Preserve this discrepancy as unranked; no tolerance/pin was changed.
The earlier independent stock multiple-slope discrepancy remains unresolved:
a different seed's success does not reinterpret that older evidence.

Fresh excluded controls, seed **202609320**, three rotations:

| Categorical PPML | Baseline | Candidate | Stock |
| --- | ---: | ---: | ---: |
| Dense three FE, N30720 K6 T4 | 192 [191,196] | 192 [188,197] | 249 [248,251] |
| One FE, N61440 K10 T4 | 147 [144,151] | 149 [144,150] | 56 [52,56] |
| Degree-six two FE, N61440 K10 T4 | 236 [232,238] | 233 [226,236] | 138 [137,139] |
| Unbalanced three FE, N30720 K10 T4 | 180 [177,181] | 183 [178,183] | 115 [113,119] |

All categorical public states, stock pairs and native exact exports pass. The
one-FE 1.4% and unbalanced 1.7% native median losses remain visible, with overlapping
ranges. These excluded paths were not optimized by this change.

Four OLS controls give baseline/candidate medians 69/66 ms for multipartition
slopes, 43/43 categorical three FE, 17/17 one-FE robust/full outputs, and 51/54
for generated-factor robust OLS. Five additional pairs on the identical factor
input give **53 [49,57] / 51 [49,56]** ms. Retain both blocks; the initial loss
did not reproduce. All **29 excluded native pairs** retain exact scientific
exports. No universal non-regression claim follows from this bounded screen.

Across both symbolic candidates, **193 public benchmark applications** pass,
with no timeouts, fit/build overlap or monitor errors. Scientific verdicts are
separate: the single stock oracle/covariance discrepancy above remains a failure.
The final controller initially used an incorrect stock/native path filter and
stopped after the fully completed pilot. Correcting the suffix check resumed
only the remaining blocks; original applications, audits and controller failure
are preserved. No fit was rerun to erase that failure. Intentional Rust unwind
and ABI panic injections pass their containment tests.

## Memory and boundaries

Final tracked peaks, baseline/candidate bytes:

| Case | Baseline | Candidate | Increase MB |
| --- | ---: | ---: | ---: |
| Two FE T4 | 22,001,780 | 23,315,172 | 1.313 |
| Unbalanced three FE | 21,606,664 | 23,688,480 | 2.082 |
| Wide T8 | 53,429,462 | 57,732,286 | 4.303 |
| Two FE T1 | 20,234,012 | 22,530,300 | 2.296 |
| Low memory | 21,937,463 | 24,019,279 | 2.082 |
| Multiple slopes | 6,013,562 | 6,203,146 | 0.190 |
| Four FE | 6,481,468 | 7,584,996 | 1.104 |

Modeled final peaks conservatively exceed these observed peaks. The compact
route avoids the rejected dense D bank; eliminated-coordinate removal saves
worker storage. Optional budgets preserve the numerical fallback and certificate
owner. This is not physical-cap qualification or a whole-action allocation proof.
Static allocation drift, constrained-fit failures, generic nonretaining-CG span
instability and deferred Linux/SCC/physical-cap checks remain open. Frozen
77/16/111 definitions remain unchanged. No push, release, SCC retry or stopped
campaign restart occurred.

## Receipts

The immutable content-addressed container preserves exact source/artifact
identities and source snapshots, declarations, harnesses, successful and failed
controllers, all timing distributions/scientific audits, tests and separate
profiles. Raw licensed logs, generated data and binaries are excluded; omitted
payload hashes are retained. The restoration script verifies the container and
every logical receipt before extraction into a new directory.

Restoration verifies **7,794 logical files**, **2,064 distinct payloads**, container
SHA-256 `2a008189fbe88c83557c0a072933e339eb13cf9725f04874fc313ad4121e8705`.
The first sealing guard overmatched the two sanitized licensed result/controller
filenames. Reviewing those exact paths and validating every payload finalized the
already-created container without rewriting it; `sealing-verification.json`
records that correction. No raw licensed log was included.
