# Reusable PPML slope projection

Local Mac development milestone, 2026-09-23. The implementation is
`fa20dfd5` plus the seven source overlays captured in `identity-ppmlproj1.json`.
All 529 computational inputs match the accepted candidate. Plugin SHA-256:
`f42ed127513640759dd4b0135c665d39e49061b40627390574bbb81fe48b17c2`.
Baseline `parproj1` is the unchanged computational state at `31a7f201`, plugin
`2ed364308f8c7e239e242414621e23cf6b59d1d9a50d6ab0abd09775e419f80a`.
This is bounded development evidence, not qualification or a universal ranking.

## Change and ownership

[ADR 0119](../../../../docs/adr/0119-reusable-ppml-slope-projection.md)
admits ADR 0118's parallel group coefficient solves and disjoint row writes
inside the reusable PPML slope MAP owner. It reuses storage and immutable
topology, recomputing all coefficients and weighted Grams on every projection.
Every observation/channel/RHS accumulation and subtraction keeps its original
arithmetic. Certificates, rank, MAP recurrence, sampling, objectives, inference,
final-Hessian checks, generations and failure atomicity are unchanged.

The optional fit-lifetime bank requires resolved MAP, slopes, N>=16384 and
T>1. Fast RHS capacity includes response plus all generated columns; low
capacity follows the existing bounded pool. A separate admitted memory charge
covers preparation through the final linear frame. It is released before
inference; capacities are included in the steady-state fingerprint. Size,
budget or allocation refusal preserves serial work. Categorical paths, other
PPML engines and Mu exploration remain outside this admission; Mu has a
separate parent separation envelope. No worker accesses SPI.

The previous [opportunity screen](../ppml-slope-opportunity-local-2026-09-23/README.md)
found native default slope losses but also stock mean/score discrepancies.
This milestone uses its adjudicated, unmodified stock library with public
`glm.tol=1e-10, fixef.tol=1e-11`. These are explicitly stricter stock calls,
not replacement default timings or diagnostic source builds. Both commands
include robust inference and requested means; file loading and export are
outside the command timer. Native SPI loading and mean publication are inside.
Positive outcomes avoid separation in these timing designs. BLAS uses one
thread; native and stock use matching requested thread counts. All explicit
fast/low requests are emitted by the corrected worker.

## Alternating paired timings

Three rotating repetitions per cell; milliseconds are median [min,max]. Gains
use paired candidate/baseline ratios rather than ratios of marginal medians.
No build, profile, archive, oracle or competing fit ran beside these timings.
Seeds 202609292/293 identify the initial and fresh blocks; cells were declared
before measurement. No threshold, partition or seed tuning followed.

| PPML case | Baseline | Candidate | Strict stock | Paired gain |
| --- | ---: | ---: | ---: | ---: |
| N30720 K6 T4, degree-six two FE, two slope partitions | 1463 [1463,1477] | 910 [894,915] | 121 [120,122] | 37.8% [37.5,39.5] |
| N30720 K6 T4, unbalanced three FE, two slope partitions | 918 [918,934] | 596 [595,609] | 119 [118,120] | 35.1% [34.8,35.2] |
| Fresh N61440 K10 T8, unbalanced three FE slopes | 2201 [2189,2231] | 1406 [1402,1419] | 232 [229,232] | 36.1% [36.0,36.4] |
| Fresh N30720 K6 T4, unbalanced slopes, low memory | 1158 [1153,1164] | 832 [814,847] | 118 [114,119] | 27.8% [27.2,29.7] |

Stock still wins all slope rows. The serial PPML slope control is
1437 [1429,1444] → 1443 [1437,1444] ms, a 0.4% paired median loss (range
0.5% gain to 1.0% loss); strict stock is 361 [359,362] ms. The categorical
three-FE control is 208 [208,210] → 209 [206,210] ms, paired median unchanged;
strict stock is 244 [243,246] ms. That categorical comparison is specific to
this strict reference call, not a claim about default stock performance.

At N30720/K6/T4, excluded OLS controls give slope medians 67→68 ms (1.5%
paired median loss; range 4.3% gain to 3.0% loss) and categorical three-FE
44→44 ms (2.2% paired gain; range 2.3% gain to 2.3% loss). These small variations
are retained, not optimized away. Both N2400 PPML pilots have unchanged native
times, 91/108 ms, and remain below parallel eligibility.

Tracked native peak increases are 655,792, 39,984, 118,864 and 35,040 bytes
in table order. The serial/small generic PPML controls add 80 bytes of fixed
owner/report state; categorical and OLS controls show zero added tracked heap.
The optional numeric bank bound is `8*(L*C*R + T*C*C)` plus T worker
descriptors, beside serial storage. This component admission does not repair
the documented static allocation inventory drift or constitute constrained
memory qualification. Low memory is not an equal-budget stock comparison.

## Science, failures and verification

All 66 headline application calls pass with no timeout, overlap or monitoring
error. All 24 headline native pairs, plus two independent pilots, have exact
scientific exports and non-clock solver fields. All 20 native PPML mean pairs
are byte-identical. All 44 within-variant repeat comparisons, scientific
exports and applicable mean binaries are exact. All 18 headline stock
comparisons pass samples, beta, V, objectives and means; all public states
pass fresh score, fixed-beta model-space and objective-replay checks.

Two N2400 pilots (seed 202609294) pass independently implemented explicit
Poisson Newton/LSMR, current-Hessian residualization and robust-sandwich checks
for baseline, candidate and strict stock. Independent FE ranks are 819/274,
total ranks 823/278 and structural residual DoF 1577/2122, matching native and
stock metadata as applicable. No stock agreement alone substitutes for these
oracles. These small pilots cover science; the larger changing-weight Rust
case and exact public pairs exercise the parallel path.

Sixty-five distinct Rust tests pass: current-weight projection formulas,
rank/unit/thread boundaries, changing weights and RHS widths, capacity
stability, exact-budget/refusal/overflow, fresh certificates, independent
slope oracles, PPML canonical/final-Hessian and rollback boundaries. Two early
development assertions were corrected: an overstrict internal constructor
check conflicted with an existing validation test, and a new telemetry test
initially omitted required route/scratch observations. Their failed logs are
retained alongside passing successors. No scientific tolerance was changed.
Both production and diagnostic plugin builds pass the actual ABI/panic test.

Four licensed fixtures pass: PPML saved slopes, PPML weights/inference, OLS
intercept slopes and OLS eclass transaction. The hidden PPML transaction
fixture fails on both baseline and candidate at its unchanged assertion that
receipt minor version equals 2; the source version is 3. Earlier injected
failure stages execute before that assertion, but neither entire fixture is
counted as passing. The two failed attempts, exact fixture source and sanitized
failure receipt remain visible. Repairing that stale fixture is separate work.
No full Rust/Stata suite, release qualification or Linux confirmation is claimed.

## Isolated profiles and next target

Diagnostic builds are separate from headline artifacts. Candidate diagnostic
plugin SHA-256 is `a19f969d66558d520a402611dc98abc8e31d32bd31636466888f5bdffbe696a4`;
baseline diagnostic is the previously captured `ppmlbreadth_profile`. Seed 202609296 uses
full N30720 rows; warmup phases are excluded. All 2,716 candidate projections
observe the resolved four-worker team. Counts and scientific outputs match.

| Diagnostic case | Projection calls | Projection seconds, baseline→candidate | Fresh certificates, calls / seconds | Reusable absorb seconds, baseline→candidate |
| --- | ---: | ---: | ---: | ---: |
| Two slope partitions | 1476 | 1.086→.472 | 143 / .468→.470 | 1.591→.970 |
| Unbalanced three FE slopes | 1240 | .644→.325 | 80 / .251→.249 | .926→.596 |

The kernel improvement carries across RHS widths, T4/T8 and fast/low storage.
Certificates now cost roughly as much as projection in the two-partition
case. Hundreds of unchanged MAP sweeps still explain much of the residual
loss. Close this scheduling milestone. Next inspect how to reduce repeated
certified slope MAP work—an accelerator scoped to the current frame and fresh
certificate costs—before further loop or thread tuning. Keep changing weights,
rank, response ownership and final-Hessian certificates explicit.

Weighted/clustered public performance, weak/disconnected PPML slope designs,
zeros/separation-heavy slopes and additional saved FE channels remain outside
this timing screen. The weighted fixture and changing-frame tests are science
controls, not throughput claims. Reserved validation seeds, frozen 77/16/111
definitions, factor-base provenance, constrained-fit/nonretaining-CG issues,
SCC deferrals and stopped campaigns remain unchanged. No push or release occurs.

The archive contains declarations, immutable controllers, source/artifact
identities, source overlays, scientific/output hashes, all audit results,
build/test records and sanitized fixture/phase protocols. Data, binaries and
raw licensed logs are excluded. Seeds 202609292–296 are used; 202609297 onward
remain unused by this milestone.

Archive: 2,355 hashed entries, 20,193,439 bytes; SHA-256
`bb65391b56f47e49679a1455b10731c86c9267a2256f28de9de8dedbb7dc751d`.
