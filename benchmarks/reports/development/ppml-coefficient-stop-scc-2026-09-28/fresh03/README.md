# Fresh cost of stricter coefficient stopping

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 27/27 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| stop-profiled-small | 1.124 / 1.250 / 1.297 | 1.763 / 2.162 / 2.346 |
| stop-small-low | 0.968 / 1.000 / 1.003 | 1.662 / 1.664 / 1.781 |
| stop-small-serial | 0.976 / 1.099 / 1.356 | 1.356 / 1.728 / 1.829 |
| stop-wide-serial-adjoint | 1.238 / 1.287 / 1.372 | 0.798 / 0.798 / 0.817 |
| stop-three-fe-core-pair | 0.930 / 0.938 / 0.963 | 0.394 / 0.423 / 0.443 |
| stop-excluded-slope | 0.860 / 0.960 / 1.119 | 0.327 / 0.339 / 0.391 |
| stop-excluded-ols | 0.990 / 1.005 / 1.022 | 1.326 / 1.843 / 1.891 |
| stop-weak | 1.165 / 1.273 / 1.372 | NR |
| stop-strong-d2 | 0.951 / 1.017 / 1.091 | 0.346 / 0.360 / 0.371 |

stop-profiled-small: selected native baseline / candidate teams = 4 / 4.

stop-small-low: selected native baseline / candidate teams = 4 / 4.

stop-small-serial: selected native baseline / candidate teams = 1 / 1.

stop-wide-serial-adjoint: selected native baseline / candidate teams = 4 / 4.

stop-three-fe-core-pair: selected native baseline / candidate teams = 8 / 8.

stop-excluded-slope: selected native baseline / candidate teams = 4 / 4.

stop-excluded-ols: selected native baseline / candidate teams = 8 / 8.

stop-weak: selected native baseline / candidate teams = 4 / 4.

stop-strong-d2: selected native baseline / candidate teams = 4 / 4.

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

## Native thread scaling

| Lower → higher setting | Higher/lower min / median / max | Slower on every repetition |
|---|---:|---|

Independent-oracle failures: 0.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.

## Scope and interpretation

Job **7763404** completes 81 headline applications, 27 candidate/baseline
science pairs and 24/27 candidate/stock pairs. All application states pass;
the three weak-d3 stock comparisons fail only the SE envelope (maximum absolute
SE difference 5.5853e-9). Coefficients, full covariance under its own registered
envelope, objective, sample and convergence pass. Native baseline/candidate
comparisons pass. This does not identify which weak-d3 estimator is inaccurate;
keep all three stock timings unranked pending independent inference adjudication.

Seed **2026092811** was fixed before generation, with three rotations per case.
Public feppml uses robust inference; the OLS control uses IID. No optional saved
observation outputs are requested in these headline fits. The library is pinned,
unmodified fixest 0.14.2. Slopes and strong_d2 explicitly use glm.tol=1e-10 and
fixef.tol=1e-11; the other stock cases use defaults. No small oracle probes are
inside these timing rotations: separate platform fixtures and the exact-original
optimum certificate provide independent scientific evidence.

The repair is accepted for accuracy, **not as a speed improvement**. Small fast,
serial, wide, weak and strong-d2 go from four to six outer iterations, with one
strict rejection; native inner counts change 108→158,108→158,113→168,146→232 and
99→142 respectively. Low remains 4/132; three-FE 7/120; slopes 4/149. The OLS
control retains 34 reported iterations. Native and stock iteration units are
not equated. The extra correction/revalidation costs 25.0% small T4, 28.7% wide
and 27.3% weak in paired medians; serial is 9.9% slower. Strong-d2 is 1.7% slower
with variable pairs. Three-FE/slope changes with unchanged work do not establish
an algorithmic speed gain. OLS is approximately neutral, +0.5%, with two losses.

Candidate/stock still exceeds 1.20 on small T4, serial, low and OLS. Wide,
three-FE, strict slopes and strict strong-d2 beat 0.90 on this source and seed.
The universal 1.20 ceiling and 0.90 aspiration are unmet. The accuracy repair
cannot be rejected in favor of the independently disproven original result;
its extra recovery work is now a concrete performance target.

| Case | Baseline / candidate / stock marginal medians (ms) | C/B losing pairs |
|---|---:|---:|
| stop-profiled-small | 428 / 506 / 234 | 3/3 |
| stop-small-low | 363 / 361 / 217 | 1/3 |
| stop-small-serial | 456 / 501 / 292 | 2/3 |
| stop-wide-serial-adjoint | 1085 / 1377 / 1686 | 3/3 |
| stop-three-fe-core-pair | 1853 / 1784 / 4066 | 0/3 |
| stop-excluded-slope | 579 / 585 / 1496 | 1/3 |
| stop-excluded-ols | 187 / 188 / 102 | 2/3 |
| stop-weak | 472 / 602 / NR | 3/3 |
| stop-strong-d2 | 833 / 847 / 2350 | 2/3 |

Marginal medians and medians of paired ratios can differ. Every ratio and
command/process clock remains in summary.json. Native phase clocks in phases.json
may nest and must not be summed. All modeled PPML memory fields match. Tracked
peaks match except three-FE, +113,040 bytes; no new heap storage is added. Sampled
process peaks remain diagnostic, not a physical-memory guarantee.

Accounting failed=0, exit_status=0, 28 isolated physical cores, **305s**,
maxvmem **1.972G**; both application and wrapper sentinels pass. Build, profiling,
archiving and other fits did not overlap headline measurement. T1/T4 cases were
grouped separately, not directly alternating: this is not a monotonic-scaling
confirmation. The three-repetition block describes this bounded source/input
comparison, not a statistical or universal performance guarantee.

The recorded T4/T1 candidate comparison passes all three exact-input/sample,
coefficient/covariance/objective, native rank/DoF and actual-team checks. Its
paired T4/T1 ratio is **1.068**, range **0.944–1.202**, with two slower T4 calls;
marginal medians are 506/501ms. Because requests were grouped, these clocks do
not isolate the effect of increasing cores. Monotonic scaling remains unproven.
See `thread-scaling.json` and its bound postprocessor; no new fits were run.

Strict-final phase medians (baseline→candidate) are **39.9→82.5ms** for small T4,
**205.6→391.7ms** wide, and **79.8→164.6ms** weak. These nested phase clocks and
increased iteration counts identify repeated strict recovery/finalization as a
specific remaining cost. They are not independent additive wall-time components.
Sampled process-peak changes (MiB, table order) are +2.25,-3.09,+1.72,-1.98,
+2.96,+3.06,-0.25,+6.67,+0.51.
