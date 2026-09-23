# PPML slope extrapolation: bounded rejection and certificate profile

Local Mac development evidence, 2026-09-23. Production computational source
remains `1de0b1bb`: all 529 inputs match `ppmlproj1`, plugin SHA-256
`f42ed127513640759dd4b0135c665d39e49061b40627390574bbb81fe48b17c2`.
This checkpoint retains diagnostic instrumentation and evidence. It adds no
native estimator speedup or production route. See [ADR 0120](../../../../docs/adr/0120-ppml-slope-extrapolation-closeout.md).

## Experiment and controls

The isolated `slope_it_probe2` build applies the existing categorical per-RHS
Irons–Tuck helper to generic slope MAP after four ordinary sweeps. It requires
at least two FE partitions, a slope, N>=128 and RHS width<=64. Each proposed
residual is retained only if a fresh certificate improves the relative
two-norm and does not worsen the infinity norm. A degenerate/nonimproving
proposal closes acceleration for that invocation and resumes ordinary MAP.
Input checks, references, requested tolerances, rank, final-Hessian, score,
objective and public transaction checks remain unchanged.

Two temporary matrices and three RHS vectors belong only to that absorption
call. No topology/weight generation gets reusable numerical state. This is
explicitly not a production memory admission or stable-capacity implementation;
no memory check is relaxed to run it. A practical gain would still have
required a separately admitted fit-lifetime owner. The experiment does not
route to generic CG or repair its known nonretaining slope span issue.

Both estimators request robust inference and saved means inside the command
timer. Native SPI loading/preparation/fit/publication are included; external
file loading and scientific exports are excluded. Stock is the pinned,
unmodified library with public `glm.tol=1e-10, fixef.tol=1e-11`, as independently
adjudicated in the preceding PPML report. No default-stock discrepancy is
reinterpreted. Threads are four on both sides and BLAS is serial. No build,
profile, archive, audit or competing fit accompanies elapsed-time screens.

## Results and rejection

Seeds 202609297/298 identify the N2400 independent pilots and N30720 screen.
Both have K4/K6 respectively, positive outcomes and two slope partitions.
The full-size screen uses three alternating/rotating repetitions. Times below
are experimental diagnostics, not adopted production performance claims.
Milliseconds are median [minimum, maximum].

| Full-size case | Production | Experiment | Strict stock | Paired experiment/production |
| --- | ---: | ---: | ---: | ---: |
| Degree-six two FE | 901 [896,915] | 908 [902,910] | 118 [116,122] | 1.0067 [0.9945,1.0078] |
| Unbalanced three FE | 604 [603,609] | 586 [586,601] | 117 [116,119] | .9718 [.9622,.9950] |

The first row loses 0.7% at the paired median; the second gains 2.8%. Added
tracked peaks are 2,456,592 and 2,456,640 bytes. These peak differences are not
workspace bounds: the temporary storage itself is two current N-by-RHS
matrices plus three RHS vectors, and different phases determine the action
peak. The small, single-run pilots give 93→80 and 109→96 ms, with strict stock
23/27 ms; those isolated small-fit observations do not warrant a size guard.

Reported ordinary inner iterations fall 276→120 and 132→74. This alone does
not establish a speedup because certification and strict work remain part of
the public command. The limited, inconsistent elapsed gains do not justify
production lifetime/admission complexity or extra memory. Reject the probe
without clamp, seed, pilot or acceptance-rule tuning. No fresh performance
expansion is warranted after this predeclared representative screen.

## Science and application outcomes

All 28 application calls complete successfully: six small pilot, 18 screen
and four isolated profile calls. There is no timeout, overlap or monitoring
error. All baseline/experiment/strict-stock comparisons and fresh public
states pass samples, coefficients, V, means, objectives, normalized scores,
fixed-beta FE model space and objective replay. Coefficients differ by at most
about 1e-16 between native variants; saved mean differences reach 1.11e-10 and
2.93e-11. No bitwise equality between changed recurrences is claimed.

Both N2400 pilots pass independently implemented explicit Poisson Newton/LSMR,
current-Hessian control residualization, robust sandwich and means for all
three variants. Independent FE ranks 819/274, total ranks 823/278 and
structural residual DoF 1577/2122 match the native and reference metadata as
applicable. All 12 within-variant repeat comparisons, scientific exports and
saved means are exact. Normal-equation agreement alone is not the oracle.

The first experimental build failed compilation because the non-Copy baseline
certificate was moved before its comparison. Cloning that small certificate
and removing an unused mutable binding fixed compilation without changing the
experiment. The first fine-profile insertion then failed its unique-source
anchor assertion; the anchor was narrowed to the intended RHS loop. Failed
build/instrumentation receipts and successful successors are preserved.
The experimental and both successful profiling builds pass actual-plugin
ABI/panic containment. No new Rust or licensed-fixture campaign is claimed:
production source is unchanged, and this candidate is not adopted.

## Whole-command explanation and next boundary

Fresh diagnostic builds instrument the same baseline and experiment on the
screen data. Warmup rows are excluded. Fine certificate timers separate
normalization, pivoted/reorthogonalized QR, projected residual moments and
raw moments. They add group-level clock overhead, particularly on many small
groups; their totals must not be used as headline speed claims.

| Case and build | Projection calls / seconds | Certificate calls / seconds | Complete reusable absorbs / seconds |
| --- | ---: | ---: | ---: |
| Two FE, baseline | 1368 / .416 | 134 / .522 | 20 / .963 |
| Two FE, experiment | 768 / .229 | 170 / .682 | 20 / .946 |
| Three FE, baseline | 1140 / .299 | 73 / .243 | 16 / .562 |
| Three FE, experiment | 760 / .218 | 94 / .311 | 16 / .552 |

The two-FE experiment saves .187 seconds of projection but adds .160 seconds
of certification; three FE saves .081 but adds .069. Extra certificates largely
offset fewer sweeps. Baseline certificate components are:

| Case | Residual norms | Basis normalization | QR | Residual projections | Raw moments |
| --- | ---: | ---: | ---: | ---: | ---: |
| Two FE | .020 | .071 | .153 | .058 | .132 |
| Three FE | .011 | .030 | .061 | .041 | .092 |

The new instrumentation stays in the benchmark harness. The rejected
experiment and its temporary builder are archived rather than retained as a
selectable route on main. Production still uses ADR 0119's admitted parallel
projection.

The next bounded implementation should examine **certificate geometry valid
only within one absorption call**. That call has immutable weights/topology,
yet rebuilds the weighted basis at each fresh residual check. Reserve geometry
capacity explicitly, rebuild/invalidate it on every new call, and keep residual
norms, moments, span projections and final certificates fresh. This would
address the measured .224/.091 seconds spent normalizing and rebuilding QR.
Independent changing-weight/rank checks and an affordable uncached fallback
are prerequisites. RHS batching of fresh moments is a separate smaller
opportunity; do not replace their scaled compensated arithmetic casually.

No gains are claimed for unmeasured weak/disconnected, weighted/clustered,
separation-heavy, low-memory or wider extrapolation cases. Frozen 77/16/111
definitions, reserved validation seeds, static allocation drift, stale hidden
PPML transaction minor-version assertion, factor-base provenance, constrained
fit/nonretaining-CG issues and SCC deferrals remain unchanged. No stopped
campaign, push, release or qualification is resumed. Seeds 202609297/298 are
used; 202609299 onward remain unused here.

The archive retains declarations, controllers, the isolated transformation and
builder, exact source/artifact identities, scientific/output hashes, all audit
results, build errors and sanitized phase protocols. Data, binaries and raw
licensed logs are excluded.

The immutable `receipts.zip` contains 909 hashed entries and 8,102,778 bytes;
SHA-256: `676aa97c426306a204a928e7b667090c7a40845fba856ca5b29c0d8667cd0daa`.
