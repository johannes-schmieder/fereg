# Native PPML coefficient mean correction: development evidence

These are focused implementation checks, not release qualification. The
statistical contract is [documented here](../../../../docs/ppml_bias_correction.md).
Current incomplete gates and the resume point belong to
[STATUS.md](../../../../STATUS.md).

## Artifact binding

The starting revision was `08891ef17ceba7256824e78d95256c16126d63db`.
The v05 local runtime was built from the immutable
[v05 source manifest](source-v05.json), identity
`f359a43902da7742a085e7b36d9ab909de16a9c1d8ffbff7602f754aa2d6363a`.
Its universal Mac plugin SHA-256 is
`57c5f334d4e777bce80f81b92b6aa23004401f6bdbd019c3774f63bc8552c2ce`.
All 206 recorded Rust, Stata, include and Cargo runtime files still matched
that snapshot when the verification receipt was written. Subsequent test
controllers and documentation have their own hashes; they do not retroactively
change that build identity. The actual plugin ABI/self-test passed.

The Monte Carlo plan remains bound to [v04](source-v04.json), plugin
`b4c51562a8620abd4fa0a4d0041776f7dec8ff03dfb77e637613a15c20bc1c76`.
The only v05 runtime change rejects nonfinite total numerical variance before
the roundoff clamp, preventing an overflow from being reported as zero. Its
new unit test passes. All ten v04/v05 independent-oracle numerical summaries
are identical; no contraction, solve, RNG or ordinary-estimation policy changed.
The frozen MC artifact was not silently replaced.

The original baseline plugin SHA-256 is
`bc0f523b6ce3680b80f0be1c292fea87dc300be6a942ad3bb3762d12fe70f64c`.
The intermediate v03 plugin SHA-256 is
`a21a72a374bd875bd944d2384593c93d9207b316a591913d195bfb1dc7104c16`,
bound to [source-v03.json](source-v03.json). The preserved
[v03 retained-fit receipt](retained-v03-original.json) originally recorded the
base Git HEAD in its `source` field. That field is not the modified artifact's
source identity; use the v03 manifest and binary hash. The original receipt
has not been rewritten.

Raw logs, attempted fixtures, build products, datasets and invocation receipts
are retained locally under `target/ppml-bias-20260926/`. The
[v04](verification-v04.json) and [final verification receipt](verification-v05.json) record hashes without copying
licensed logs or machine-specific paths into Git.

## Accepted-predictor recovery correction

The frozen confirmation campaign exposed one native raw-effect reconstruction
failure in duration replication 10131. The independent accepted means were
accurate; subtraction of large IRLS working quantities contaminated the raw
FE target. The v06 correction recovers the same anchored raw effects from the
accepted predictor, offset and finite contribution. No tolerance, ordinary
solve, D/C contraction or probe policy changes.

[Source v06](source-v06.json) and [local verification](verification-v06.json)
bind the new universal Mac artifact. All 14 focused core tests and 170 serial
plugin tests pass. The concurrent plugin run has 16 allocation-counter failures
and is preserved as failed. The [retained-draw regression](reconstruction-v06.json)
passes 15 full/A/B exact/randomized and reordered core calls against independent
oracles: maximum raw/D/C errors are 4.81e-10/2.47e-11/7.29e-13, respectively.
Ordinary coefficients, outputs and covariance are identical with the option on.
The [same-harness old/new comparison](reconstruction-adjudication.json) confirms
that v05 fails on this input. Its failure remains in the frozen MC denominator.
The [captured Rust harness](core-reconstruction.rs) links the core rlib from each
immutable release plugin build; the input is the decompressed synthetic CSV
under `tests/stata/bias/`. This direct-core check is not licensed-Stata evidence.

[SCC v05 feature evidence](scc-feature-v05.json) records four passing fixtures
and ten independent oracles under Linux/Stata 19. The original diagnostic
wrapper failed because of its terminal marker and is not reported as passed.
[SCC v06 validation](scc-v06-result.json) passes shared smoke, all four feature/
help fixtures, ten control oracles and all 15 reconstruction cases. The maximum
control/reconstruction discrepancies are 6.62e-10/9.19e-10. The
[Mac Stata 18 validation](licensed-mac-v06.json) passes the same feature and
oracle checks. Its eight-second overlap with the existing MC controller is
recorded; the timing preflight rejected that attempt before any measurement.
The new Mac binary is installed in the local development dist directory.
Campaign completion and any outstanding work are tracked only in STATUS.md.

## Independent mathematics and retained fits

The [ordinary baseline](retained-baseline.json) passes all 62 retained retry
datasets, each fitted on full/A/B samples: 186 independent comparisons.
The v03 candidate also passes all 186. Its maximum fitted-mean discrepancy is
`2.151e-9`, normalized-effect discrepancy `3.267e-11`, and normalized score
`6.282e-11`. Row identities and exclusions were checked. The original baseline
used Stata 19; candidate and final feature checks used actual Stata 18.

The [v05 explicit-design oracle](controls-v05.json) passes ten counts/duration,
unweighted/fw/pw, sparse, collinear, high-leverage and imbalanced cases. The
largest raw/increment/corrected finite or FE discrepancy is `1.086e-10`.
Ordinary coefficients, covariance matrices and samples remain identical in
these comparisons. The public fixture additionally checks unchanged ordinary
saved effects and means, shared normalization, missing-ID rollback, omitted
positions, independent streams, changed row order, threads and low-memory
execution.

Independent scalar and full-design checks verify the signs and half factor.
For four completed-duration units with exposures 4, 6, 10 and 20, the fitted
rate is 0.1, J=4, K=1.52, D=-0.095, C=0.0475 and D+C=-0.0475. The Python oracle
does not invoke native numerical helpers. Its fixed-probe checks include
counts 8/31/256/1024 and tile widths 1/3/8/32. Native randomization tests use
independent seeds, verify the grouped numerical variance and D/C covariance,
and distinguish probe variation from solver error. Native production streams
one RHS at a time. Random123 known-answer vectors independently bind Philox.

## Regression and failure coverage

The final targeted core bias suite has 14 passing tests. The broader v03 core
screen had 1,140 passes, four failures and one ignored test. Two failures were
stale protocol expectations and passed after correction. Two reproduce on the
untouched starting revision: the initial full-width LSMR fallback assertion and
the strict-checkpoint rejection-count assertion. They remain visible.

The plugin screen passed 132 tests; its remaining stale protocol assertion was
fixed and passed in a targeted rerun. Core integration checks passed 22 tests.
Two separate memory-oracle assertions reproduce the same 320,000,000-byte
baseline model drift. Testkit suites have 89 passes and one independently
reproduced baseline CMG oracle discrepancy. No numerical tolerance or solver
policy was weakened to hide these failures.

The Python PPML screen ran 252 tests: 248 passed and four were skipped. The help
contract has 16 passes and the independent Python correction suite has three.
Three evidence-audit tests check summary precision and artifact/settings binding.
The [final local audit](final-local-review.json) also verifies all 95 frozen
Separations reference files, the unchanged production ado hash, and identical
public/canonical help bytes.
Clippy reports the same 50 distinct error messages on base and candidate; it
is not globally green. No new error message remains.

After updating stale wire/inventory assertions, the broad licensed Stata screen
has 51 passes and three baseline failures: public/hidden wrappers of the same
PPML route assertion and an exact OLS covariance equality assertion. Both
underlying behaviors reproduce with the original plugin and ado sources.
The final new feature fixture, all seven public and seven hidden help examples,
and the test-only Split Branch example pass under Stata 18. Allocation,
auxiliary-solve, interruption, missing-ID and publication-store failure checks
retain transaction rollback. An early help invocation used an incorrect test
path and returned r(601); the correctly located fixture subsequently passed.

## Frozen Monte Carlo and performance protocols

The [confirmation plan](mc-plan.json) fixes seven Separations designs, fresh
repetitions 10001–10300, native/legacy B=256 pairs, native exact checks on the
first 20, and native B=1024 checks on the first 50. This is 2,100 distinct data
draws and 4,690 estimator attempts. Full/A/B native streams are 0/1/2. Wage
models and FEVC code, seeds and probes remain unchanged. Every failed attempt
stays in the denominator and retains a reproducible seed and failure dataset.
The [analysis plan](mc-analysis-plan.json) distinguishes all available, paired
available and joint-interior populations. It reports bias, MCSE, RMSE and tails
for all ten external moments; Split Branch worker-wage variance is undefined
and FEVC M/MS results remain separately reported. There is no universal
zero-bias or RMSE-improvement acceptance criterion.

All fourteen original native/legacy development pilots passed on matching
datasets. Their largest unchanged wage/FEVC moment difference was `7.14e-14`,
consistent with summation order after FE spreading. The frozen comparison
tolerance is `1e-10`; the data signatures must match exactly. These development
pilots are not fresh-seed confirmation results.

The [v05 benchmark plan](benchmark-plan-v05.json) fixes eight local configurations,
2,880–120,000 rows, 8–256 levels per FE set, 240–10,000 units, 64–1,024 probes,
one/two/four threads, and dense/sparse memory routes. Each has two reversed
engine orders and one cold plus five warm calls per process. Current-solve
and cached-factor Mata references use the frozen row operators, matched
Philox probes, identical contractions and numerical diagnostics. Every route
publishes eighteen FE variables and nine finite coefficient/diagnostic matrices.
End-to-end reference calls also refit ordinary PPML. Probe preparation, phases,
repeated-call elapsed time and peak RSS are reported. Process elapsed includes
startup, Mata compilation, all calls and exports; startup/compilation are excluded
from repeated-call timings but are not individually timed. The superseded v04
plan in `benchmark-plan.json` was cancelled before its first measurement.

The initial matched-kernel matrix passes full increment and FE numerical
covariance comparisons to `8.14e-15`. Its timings lacked the required process
monitor and are diagnostic only. The [monitored end-to-end results](benchmark-v05.json)
pass all eight cases and all 48 process-isolation receipts. Across 288 calls,
the maximum matched increment/numerical-covariance discrepancy is `8.14e-15`.
The [measurement table](performance.md) reports warm distributions, phase scope,
peak RSS and route limits. Native correction medians are 3.4–9.7 times faster
than cached-factor Mata on these designs; end-to-end ratios are 2.8–8.3.
The four-thread dense correction does not improve over its one-thread case.

The [first confirmation block](mc-firstblock.json), with
[all-ten-moment tables](mc-firstblock.md), contains 280 attempts on
140 fresh datasets, twenty per design. All pipeline calls completed. Native
had no failures or boundary flags; four legacy PPML fits fell back to zero,
leaving sixteen joint-interior comparisons in `fixed_3840` and twenty in every
other design. Both FEVC populations succeed in every attempt. The largest
unchanged wage/FEVC difference is `1.93e-13`. This partial receipt reports all
ten moments, their Monte Carlo precision, RMSE and tails, and sets `complete`
to false. It is not the completed 300-draw confirmation campaign.

Linux/SCC,
million/ten-million-row performance, and Windows scientific behavior are not
established by the local tests. FEVC, its pin, Separations production ado files,
and the older SCC production campaign are unchanged.
