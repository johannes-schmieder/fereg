# PPML slope and saved-mean opportunity screen

Local Mac development milestone, 2026-09-23. All 529 computational inputs at
`31a7f201` match `parproj1`, plugin SHA-256
`2ed364308f8c7e239e242414621e23cf6b59d1d9a50d6ab0abd09775e419f80a`.
This checkpoint implements additive PPML measurement and independent-audit
support; it does not change estimation code or claim a new native speedup.

## New coverage and comparable work

`next_subsystem/run.py --ppml-surface` supports positive, finite, unweighted
numeric PPML with categorical or intercept-plus-slope partitions, robust
inference, explicit fast/low memory and requested `mu()` output. It rejects
unmatched weights, inference, generated terms and output requests. Existing
OLS workers and frozen 77/16/111 definitions remain unchanged.

Both estimators fit the same data, FE span, finite controls and sample. Native
`feppml` saves its mean in the public command. Stock `fepois` uses its public
response prediction inside the command timer. Both export those means after
timing. Stock HC0 covariance is normalized by N/(N-1), as in the existing
PPML screen; no frequency-weight comparison is inferred. File loading and
exports are outside timing; native SPI loading, preparation, fit, inference
and mean publication are included. Threads are four on both sides and BLAS
uses one thread. No competing compilation, fit, profiling or archiving runs
alongside the default screen.

The captured default/strict blocks initially omitted `memory(fast)` for cells
labeled fast: their public requests were `memory(auto)`, and every successful
such run selected fast. Tables below describe those actual requests, not a
new explicit-fast timing claim. A final review corrected the worker to emit
both explicit modes. Three repeated small pilots then passed all comparisons,
independent/public-state checks and explicit requested/selected memory codes;
native scientific exports and means were exact against the earlier automatic
requests. `memory-correction.zip` preserves that follow-up without rewriting
the original timing archive.

Slope sources are deterministic Gaussian columns added outside timing. Their
positive multiplicative outcome contribution preserves finite positive y;
this screen requires no separation and does not cover zeros or separation-heavy
fits. Independent singleton pruning establishes retained original row IDs.
The audit checks sample/N, every finite coefficient, covariance, reported
objectives and saved means. It additionally replays objectives, checks fresh
scores on all finite and FE/slope columns, and projects log(mu)-X beta onto
an independent explicit FE design. Fresh span residuals above the threshold
are indeterminate, not proof of infeasibility. Stationarity and span thresholds
follow the existing PPML public-state audit; no native/comparison tolerance is
relaxed. See ADRs 0016 and 0026.

Bounded N2400 pilots use independent column-equilibrated SciPy Newton/LSMR
Poisson fits, exact dense FE-rank checks, fresh Hessian-residualized controls
and robust sandwiches. This extends the separate explicit-design oracle to
slope columns; it does not call the native solver or use reference agreement
as its sole scientific evidence.

## Current default screen and reference discrepancies

Seeds 202609289/290/291 respectively identify independent pilots, the three-repeat
rotating default screen, and isolated profiles. All cases below have N30720,
T4 and saved means. Entries show milliseconds, median [minimum, maximum].

| Case | Native | Default stock | Current comparison |
| --- | ---: | ---: | --- |
| Categorical three FE, K6 | 206 [201,207] | 120 [115,120] | Means fail; stock score fails |
| Unbalanced categorical three FE, K10 | 169 [168,170] | 71 [70,75] | Means fail |
| Degree-six two FE, two slope partitions, K6 | 1488 [1482,1488] | 71 [69,71] | Means fail; stock score fails |
| Unbalanced three FE, two slope partitions, K6 | 1017 [1015,1023] | 71 [68,78] | Pairwise and public-state checks pass |
| Same unbalanced slope design, native low memory | 1196 [1187,1199] | 71 [70,75] | Pairwise and public-state checks pass |

These are descriptive default timings, not a blanket stock ranking. Nine of
15 large comparisons fail saved means: maximum discrepancies in the first
three rows are 4.53e-5, 1.10e-6 and 5.84e-5, respectively 34.3, 1.06 and 74.3
times the unchanged envelope. Samples, beta, V and reported objectives pass
on these large rows. Stock stationarity also fails rows one and three. All
native public states pass. Six large comparisons pass completely, but the
corresponding small stock slope pilot still misses its mean envelope.

All three native small pilots pass every independent check. All three default
stock pilots fail means; the two-slope pilot also fails eight covariance
coordinates, up to 1.926 times tolerance. Two stock pilots exceed the 1e-8
stationarity threshold. Reported objectives and model-space checks pass.
The independent mean discrepancies remain visible even where the reported fit
objective agrees. No old weak/disconnected discrepancy is reinterpreted.

## Separate strict-stock diagnostic

After those failures, the same pinned, unmodified fixest source was called
with explicit public parameters `glm.tol=1e-10, fixef.tol=1e-11`. The first
request used `fixef.tol=1e-12`, which the pinned API rejects below its supported
floor of about 2.22e-12. Three stock warmups fail before estimation and remain
archived. Correcting that API request does not change native or comparison
tolerances. The diagnostic uses the original pilot/screen data and is separate
from default headline timings.

All three corrected strict-stock pilots pass samples, coefficients, covariance,
objectives, means, fresh scores and model space against the independent oracle. The
independent FE ranks are 538, 819 and 274, giving total model ranks 542, 823
and 278 with four finite controls. These equal stock `nparams` and native
`rank + df_a`; native structural residual DoF are exactly 1858, 1577 and 2122.
The native structural constant is counted in `rank`, so its absorbed-only
`df_a` is one less than the explicit FE rank.
A single full-size diagnostic per cell also passes every pairwise/public-state
check. Native exports and means are byte-identical across default/strict
reference-control runs in all eight cells.

| Case, in the same order as above | Native ms | Strict stock ms |
| --- | ---: | ---: |
| Categorical three FE, K6 | 196 | 244 |
| Unbalanced categorical three FE, K10 | 172 | 118 |
| Two slope partitions, K6 | 1475 | 118 |
| Unbalanced three FE with slopes, K6 | 1014 | 117 |
| Same slope design, native low memory | 1194 | 118 |

These one-repeat diagnostics establish a substantial slope opportunity even
at demonstrated comparable accuracy. They are not distributions, a stock
source modification, a replacement for failed default receipts or a new
qualification result. Dense categorical three FE is competitive in this
particular strict diagnostic; the unbalanced categorical result still loses.

## Whole-command costs and mechanism scope

Default native runtime receipts place preparation around 9–14 ms, mean
publication around 5–6 ms and fast inference below 1 ms. Low-memory finite
solve/inference costs are higher. The large slope loss is primarily inside
absorption: reported ordinary FE work is about 1.03/.59/.62 seconds for the
three slope rows, plus .389/.355/.391 seconds for strict/final solve phases.
These are native runtime categories, not isolated input-loading costs.

An isolated `ppmlbreadth_profile` artifact at the same source has plugin hash
`d554a6c71f6eca9b4c00847df347dd2c4a7d67525879f8093d13eb2c6e086c41`.
It never enters default or strict-stock timing tables. Its coarse timers show:

| Slope case | Reusable absorbs | Projection calls / seconds | Certificate calls / seconds | Total reusable absorb seconds |
| --- | ---: | ---: | ---: | ---: |
| Two partitions | 20 | 1380 / .999 | 135 / .432 | 1.458 |
| Unbalanced three FE | 18 | 1260 / .655 | 81 / .256 | .935 |
| Unbalanced three FE, low memory | 19 | 1320 / .695 | 85 / .258 | .978 |

The source trace explains why recent OLS gains do not cover this cost:
`PreparedLinearSession` constructs a `ReusableMapAbsorber` for slope-bearing
canonical MAP frames. That object retains immutable topology and reusable
storage, but uses serial `ProjectionWorkspace`; it does not enter the generic
automatic OLS projection plan from ADR 0118. Its distinct certificate builds
current-weight QR and raw moments, also outside the one-shot certificate cache.
The accepted numeric loading, retained finite design and fresh reference
statistics improvements do reach the surrounding session. Specialized
categorical checkpoint and acceleration guards deliberately exclude this
slope geometry.

Immutable membership/topology and scratch ownership can persist. Hessian
weights, weighted geometry, working responses, group coefficients and residual
checks cannot be carried across PPML frames as valid numerical results. The
promising next mechanism is the existing independent group-coefficient/row
projection, with fresh coefficients on every call. It needs explicit optional
fit-lifetime memory admission and truthful capacity reporting, keeping an
affordable serial route and stable reusable capacities. It is not permission
to reuse OLS weighted geometry across IRLS generations.

## Checks, limits and receipts

There are 67 application attempts: 64 successes and the three documented stock
API-refusal warmups. No completed-call timeout, overlap or monitor error occurs.
All 20 default repeat comparisons pass; their scientific exports and means
are exact for both estimators. Ten focused harness tests pass, including
independent analytic slope means, deliberate score/span/covariance corruption,
row-hash alignment, supported-surface refusal and staged output matching.
The diagnostic plugin passes the actual ABI/panic-containment check. Native
computational inputs remain identical to the prior tested checkpoint; no new
Rust test campaign or licensed fixture suite is claimed.

The first audit assumed retained sample.csv files, while the controller keeps
the certified ordered row hash and deletes that CSV. Alignment now checks the
independent row IDs against that hash before indexing physical native means.
A stock-to-stock repeat check also exposed the existing `ll` versus `loglik`
metadata asymmetry; the comparator now accepts either spelling on both sides
without changing envelopes. Both boundaries have regression tests. The initial
failed audit log and original stock-refusal receipts remain archived.

Native tracked peaks for the five screen rows are about 58.3, 40.3, 15.6,
16.7 and 17.3 MB. This checkpoint adds no production memory or numerical state.
Low-memory mode is not an equal-budget stock comparison. Individual saved FE
and saved slope-channel costs, weighted/clustered PPML, other thread counts,
zeros/separation-heavy samples, weak/disconnected slope topologies and
constrained-memory throughput remain outside this new screen. Earlier
applicable receipts stay historical, not new-source claims.

`receipts.zip` and `memory-correction.zip` include declarations, immutable controllers, source/artifact
identities, scientific and output hashes, sanitized protocols, diagnostic
phase sums, all failed/successful audits and build/test logs. Data, binaries
and raw licensed logs are excluded. Frozen benchmarks, stopped campaigns,
static allocation source drift, nonretaining CG/weighted constrained-fit
issues, factor-base provenance and SCC authentication deferrals are unchanged.

Next implement and measure optional parallel projection in the reusable PPML
slope path, beginning with the verified two-slope and unbalanced three-FE
cases and protecting categorical, serial, low-memory and changing-weight
controls. Do not tune the default stock discrepancy or reopen closed OLS
scheduling experiments. This is a committed coverage/profile milestone;
the substantial native PPML performance gap remains open.

Primary archive: 1,548 hashed entries, 10,557,513 bytes, SHA-256
`4ed1b2a5f452a6265bde632680a8e7f6b3d574b79c51cbffba1ecc93bf3f6433`.

Memory-correction archive: 158 hashed entries, 296,691 bytes, SHA-256
`6a39a61e30993ee1916fcd5fc714faa7c135023730b72e868c24bbcedba2f354`.
