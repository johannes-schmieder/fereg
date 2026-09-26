# Automatic RHS report aggregation: development correction

The rejected lighter SCC smoke 7745382 never timed its low-memory three-FE
slope case because initialization failed at N7,680/K6/T4, seed104729. Diagnostic
job 7745601 on the same candidate/input reported `prepared OLS low-memory solver
reports are inconsistent` and Stata rc459. Scheduler failed=0, exit_status=2,
four slots, 10 seconds, maxvmem750.625M: its wrapper looked for the wrong Stata
log basename after the native diagnostic had completed. This is useful failure
evidence, not a passed job. Original logs remain in the rejected run's
`diagnostics/low-ols-1/`; the original frozen controller and smoke remain rejected.

ADR0224 repairs the metadata merger, with no numeric kernel changes. The old
accepted canonicalhelpers2 Mac plugin reproduces rc459 on the exact SCC warm-up
DTA; the candidate succeeds with default poolsize10/T4, poolsize1/T1 and
poolsize3/T4. All accepted results retain the native certificates. A separate
column-equilibrated sparse explicit full-design LSMR verifies the six warm-up
coefficients; no dense-rank or independent warm-up covariance claim is made.
Fast/low covariance is checked separately. The successful candidate must still
pass the new Linux campaign preflight under its newly frozen source/artifact.

Public validation contains 22 attempts: the expected baseline warm-up failure
and 21 successes. Four additional N960 cases use the same seed, K6, three FE,
OLS/PPML, categorical/two slopes on each of the first two partitions, each in
fast/low mode. Independent explicit-design checks cover coefficients and full
covariance; PPML uses independently solved Poisson scores, final Hessian and
sandwich inference. Baseline/candidate controls are identical except for OLS
slope rounding at approximately 1e-15. Initial strict bitwise assertions exposed
that rounding and were replaced by an explicit 1e-12 comparison, below the
unchanged independent scientific envelopes; estimators were not rerun to select
results. All values and artifact/input hashes are in public-checks.json.

The standalone actual-plugin ABI/self-test passed, including injected panic
containment. Forty-nine focused `low_` core tests passed; four `report_merge`
tests and the largest-attempt memory accounting test passed. These overlap:
51 distinct Rust tests. The new unit test checks all 24 orderings with/without a
real fallback and direct-CG/MAP combinations; existing incompatible/uncertified
negative controls remain. The tested Mac artifact precedes whitespace-only
formatting of the new source hunks; the Linux build will use the committed source.
An initial accidental run against stale repository dist failed protocol checking
(rc499), was excluded, and was replaced with the hash-verified accepted baseline.

These are diagnostic/scientific checks, not performance rankings or release
qualification. No new numeric bank or memory allowance is introduced. The
original low-memory instability and allocation-inventory source drift remain
open. Frozen benchmark designs, repetitions, reference pins and skips are intact.
