# Two-FE dense loading, solver, and recovery investigation

This local Mac development checkpoint contains 129 application attempts,
including the failed setup/reference checks described below. It is an
investigation, not final acceptance: neither final five-pair block nor the
regression matrix has run. The committed baseline is `637f2407`, the completed
one-FE endpoint. All artifacts are isolated; the installed package is unchanged.

The primary input has 983,040 observations, 196,608 worker levels, 9,831 firm
levels and one connected component. It uses the existing five-period,
20%-mobility generator, K=10/50, and sixteen native workers on the M2 Ultra.
Stock fixest is pinned at 0.14.2. Imports and audit exports are outside the
command timers; SPI input, estimation, inference and requested publication are
inside. Builds, tests, generation and archive hashing do not overlap timing.

## Measured increments

Each row below is a separate three-pair screen; medians are milliseconds.

| Change / request | K | Before | After | Stock fixest |
|---|---:|---:|---:|---:|
| Numeric multi-FE loading / IID | 10 | 665 | 618 | 427 |
| Numeric multi-FE loading / IID | 50 | 853 | 754 | 1505 |
| Narrow Schur admission / IID | 10 | 626 | 311 | 436 |
| Narrow Schur admission / wide control | 50 | 756 | 776 | 1504 |
| Direct graph recovery / saved FE | 10 | 921 | 608 | — |
| Direct graph recovery / full outputs | 10 | 966 | 641 | — |

The wide control in the Schur screen is about 3% slower even though its routing
is unchanged; these measurements remain in the record. No run is discarded.
The first six narrow breadth cells (IID, robust, FE-level clustering, crossing
clustering, and both two-cluster cases) pass the numerical comparisons and each
beat stock fixest by more than 10% in their single-pair screen. This is not a
repeatability claim. Saved effects still miss the speed target at this checkpoint.

The loading extension admits multiple ordinary numeric intercept effects to
the existing checked row pipeline and loaded-RHS/moment reuse. Width, weights,
sample, memory and thread guards remain. Six incremental native comparisons
are exact for b, V, sample, status, DoF, fit and certificate exports.

The Schur prototype adds unit-weight fits with 8–15 actual RHS columns and at
least eight requested workers to the existing eligibility envelope. The graph
probe, 0.01 contraction threshold, iteration accounting, memory preflight and
independent certificates are unchanged. Weighted and lower-thread narrow fits
retain their routes. The initial narrow solve falls from approximately 400 ms
of CMG work to 162 ms including the Schur probe. The CMG profile's ten-column
and two-column solve batches cost 157 and 131 ms respectively.

The direct recovery prototype propagates a fitted two-effect target through a
spanning forest, then applies the existing component normalization and complete
reconstruction certificate. Failed reconstruction returns to the existing
recovery chain. Its coefficient and queue banks occupy 16 bytes per FE level;
the queue is released before normalization, within the existing admitted
128-byte-per-level finishing allowance. Public admission is limited to large,
unit-weight, two-intercept OLS recovery on at least eight workers. PPML and
weighted recovery are unchanged.

## Profiles and saved-output accuracy

The separately instrumented fixest fit spends 321 ms demeaning at K=10 and
1.18 seconds at K=50. The trace library is identified separately in
`references.json`; those calls do not supply stock headline timings.

The narrow saved-FE prototype spends approximately 203 ms on the strict final
residual projection and 390 ms recovering effects. Direct graph recovery
removes most of the latter cost. Ordinary CG for the final one-RHS projection
is the next isolated candidate, outside this archive.

Default fixest saved effects differ from both native artifacts by up to
1.2875e-5, failing the unchanged output tolerance. The native baseline and
Schur prototype differ by at most 1.5e-12. Tightening only `fixef()` recovery
does not change the reference values: its two-FE routine starts from the
estimator's stored `sumFE`. Tightening `feols()` to 1e-10 reduces the discrepancy
but leaves 123 near-zero entries just outside tolerance (maximum violating
error 1.1255e-9). A separate 1e-11 estimation diagnostic passes every output
comparison. Its slower reference timing is not substituted for default timing.
All failed comparisons are retained and are not called passing science.

## Validation and preserved failures

The expanded loading unit covers one/two/three effects, thread and exact-budget
boundaries, reordered regressors and missingness. The new public Stata fixture
checks two/three-FE dummy oracles, IID/robust/clustered inference, alternate
loading, saved outputs, missingness, cascading singletons, restrictions, weights
and low-memory execution. Artifact-specific fixture receipts include the
transaction rollback check. Five focused Schur tests pass, including weighted
cuts, independent MAP, memory/iteration limits and continuation. The new narrow
guard also rejects a cycle through its original probe. Direct graph recovery
passes connected/disconnected, reversed-row, partial-output and inconsistent-
target tests. Actual-plugin ABI/panic-containment checks pass for each artifact.

Earlier harness failures remain visible:

- The first R smoke lacked a binary input sidecar and failed before fitting.
- The disconnected 960-row default reference overcounted FE rank by one;
  disconnected diagnostics now explicitly request `K.exact=TRUE`.
- That small reference's default robust covariance also missed the registered
  numerical tolerance; it was not converted into a pass.
- The first tight smoke passed the wrong recovery argument name (`tol`). Its
  estimation was tight but recovery used the default. The corrected four-case
  saved-output smoke passes independent dummy checks.
- The first large breadth series stopped at the default-reference saved-output
  mismatch. Separate recovery-only and 1e-10 estimation diagnostics also failed,
  followed by the passing 1e-11 diagnostic described above.
- Two archive attempts failed while handling the pre-fit reference failure's
  missing sample/timing fields. The corrected collector retains that failed
  call without manufacturing a sample or timing ratio.

`measurements.json`, `summary.json`, `series.json` and `profiles.json` retain the
per-call evidence. `identities.json` binds source and artifact hashes. The
baseline provenance preserves its original pre-commit build identity rather
than rewriting it as a new build. The build helper's embedded date remains
2026-09-19; these measurements were run on September 20. Raw licensed logs stay
local under `target/`.
