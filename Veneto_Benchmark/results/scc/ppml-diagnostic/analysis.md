# Veneto PPML diagnostic addendum

September 12, 2026. This analysis follows the stopped benchmark report, which was sent unchanged via Telegram. The production campaign remains stopped. The new runs are diagnostic experiments and do not add performance observations to the report.

## Finding

**The reproduced feppml failure is caused by floating-point cancellation in the initial weighted-regression reconstruction check.** It occurs before the first PPML update. The initial fixed-effect projection, rank, regression score, bread/inverse, residual projection, and retained-design inverse all pass their respective checks. Only predictor reconstruction fails.

The original frozen binary reproduces `r(430)` at four threads with the message:

> prepared PPML initial weighted-linear solve did not certify

A separate build of the same source, with logging added to one file, identifies the rejected check at both four and fourteen threads. Its measurements are:

| Initial weighted-regression diagnostic | 4 threads | 14 threads |
|---|---:|---:|
| Reconstruction error relative to the working response | 2.35746e-14 | 1.48636e-14 |
| Required upper bound, 64 × machine epsilon | 1.42109e-14 | 1.42109e-14 |
| Error / required bound | 1.659 | 1.046 |
| Maximum reconstruction error relative to the summed terms | 1.59192e-16 | 1.55816e-16 |
| Maximum absolute reconstruction error | 2.27374e-13 | 2.27374e-13 |
| Maximum cancellation ratio | 392.03 | 392.03 |
| Other six initial-regression checks | All pass | All pass |
| Completed PPML updates | 0 | 0 |

Machine epsilon for these double-precision calculations is 2.22045e-16. Thus, measured relative to the magnitudes participating in the calculation, the error is ordinary rounding. The current check instead scales it by a much smaller result after cancellation and rejects the fit. The recorded values and acceptance flags are in [diagnosis.json](diagnosis.json).

## Why the check fails

Let `z` denote the initial PPML working response, `f` the finite-regressor predictor, `a` the absorbed predictor, and `r` the weighted-regression residual. The frozen implementation computes:

```text
a = (z - f) - r
reconstructed_z = (f + a) + r
error = abs(reconstructed_z - z) / max(abs(z), abs(reconstructed_z), 1)
accept only if max(error) <= 64 * epsilon
```

These identities hold in exact arithmetic. In floating-point arithmetic, subtracting and then adding large opposing terms loses some low-order digits. The error bound needs to reflect the operations and their magnitudes; a constant multiple of the small reconstructed response does not uniformly account for cancellation.

Here the maximum magnitudes are approximately 1,842 for the working response, 1,231 for the absorbed predictor, and 1,104 for the residual. The finite-regressor predictor is below 0.69 in magnitude. The problematic cancellation therefore concerns absorbed predictions and residuals, rather than exploding finite-regressor coefficients. The cancellation ratio in the table is the maximum of `(abs(f) + abs(a) + abs(r) + abs(z)) / max(abs(z), 1)` across observations. The term-scaled error uses the same sum of magnitudes as its denominator, floored at one. Neither calculation changes the fitted result or its acceptance decision.

The initial linear score is approximately 1.70737e-7, within its scheduled 1e-6 inner tolerance. Both finite inverse checks are also accepted. These are checks on the initial weighted regression; they do not establish convergence of the unfinished PPML estimator.

The causal path is explicit in frozen source `d44e36b`:

- `crates/fereg-core/src/ols/linear_session.rs:4830`: subtract the finite predictor and residual, then add them back.
- `linear_session.rs:4848`: apply the `64 * f64::EPSILON` reconstruction cutoff.
- `linear_session.rs:844`: require reconstruction in addition to the six other certificates.
- `crates/fereg-core/src/ppml/prepared_irls.rs:1002`: reject an uncertified initial weighted regression with `r(430)`.

The [logging-only patch](certificate-trace.patch) exposes these existing decisions. Verification against all 2,028 tracked files of the frozen revision finds exactly one modified file, containing only diagnostic output and an extra read-only pass for aggregate error measurements. Production estimator source and the frozen benchmark binary were not changed.

## Why core counts appear to matter

The observed numerical errors lie close to a hard cutoff. Small differences in floating-point calculations can move a reconstruction across that cutoff without materially changing the underlying regression. This is consistent with the benchmark's rapid failures and isolated fourteen-thread success.

It does **not** establish that fourteen cores solve the problem. The new fourteen-thread diagnostic also fails, missing the cutoff by about 4.6%. It ran on an Intel Gold-6226R after the original E5-2680v4 restriction was removed to reduce queue waiting. Hardware, worker scheduling, and rebuilding the binary can affect low-order results; this experiment does not separate their contributions. The old successful fourteen-core fit must not be treated as a dependable workaround.

The original and instrumented four-thread traces have the same projection routes, iteration counts, accepted canonical projection norm, and decisions. A few individual projection diagnostics differ in their last digits; the largest absolute difference is 7.75e-18. They are not claimed to be bit-for-bit identical.

This mechanism is established for the new instrumented reproductions. The earlier quiet logs did not save the precise native error, so it cannot retrospectively prove that every historical `r(430)` had the same cause. Nor does an early failure reveal whether further problems would occur after a reconstruction repair.

## Why Veneto exposes the issue

The initial two-effect feppml mean is the global outcome mean, so the initial response is `z = log(mean(y)) + (y - mean(y)) / mean(y)`. On this panel, daily pay spans a 56.4-million-to-one range. The initial regression must accommodate large response differences across workers and firms, producing absorbed predictions and residuals that can largely cancel on individual observations. The trace directly measures that cancellation; the role of any particular observation or graph component has not been isolated.

The retained panel has 3,159,720 observations and 41,901 connected worker-firm components. All outcomes are strictly positive. With positive outcomes, each observation's Poisson negative log-likelihood `exp(eta) - y*eta` tends to infinity in both tails. Restricted to the model's finite-dimensional predictor space, it therefore has a finite minimizing predictor. Redundant fixed-effect parameterizations can make coefficients nonunique, but do not by themselves imply a nonexistent fitted mean. This mathematical fact, together with successful ppmlhdfe and R fixest fits, argues against separation or a fundamentally infeasible PPML specification as the explanation here.

Neither winsorizing the data nor increasing the outer iteration cap addresses the measured failure. It happens before any outer update, and the initial FE solve already passed. The command times in the stopped report are failures, not successful fast estimates.

## The other PPML failures are different

**Pyfixest:** eight complete-task runs stop with `Demeaning failed after 10000 iterations`; two one-thread runs time out. In the pinned source, the default Rust MAP demeaning routine returns an unsuccessful projection status, which the Python wrapper converts into that exception. The routine is called inside each Poisson weighted-regression step, with weights determined by the current fitted means. The 10,000 limit belongs to the inner fixed-effect projection, not to the separate 25-step outer Poisson limit. This is a projection convergence failure, distinct from feppml's reconstruction rejection. Sparse mobility and changing weights are plausible sources of difficulty, but the saved traces do not identify the failed outer iteration, its conditioning, or a proven remedy. See the pinned `pyfixest/demeaners.py`, `estimation/internals/demean_.py:70`, and `estimation/internals/fit_glm_.py:206` paths recorded in the stopped analysis.

**Gglm:** every complete-task PPML run reaches the 2,400-second command cap. That establishes an unacceptable runtime under the tested protocol. It does not establish nonexistence, divergence, or which internal operation dominates: no stage-level profile was captured.

**Ppmlhdfe and R fixest:** the completed PPML runs converge. The stopped report retains their numerical comparisons and the small R coefficient discrepancy against the strict comparison threshold. Their success does not imply identical algorithms or stopping rules, but shows that the data can support a converged fit.

## Repair direction and limits

The first repair target is the numerical construction and certification of the initial combined predictor. A candidate should retain or form the combined predictor in a stable way and certify reconstruction with an explicit floating-point error analysis. The existing ordinary PPML path already has a combined-predictor representation worth examining. Its suitability for initial and final canonical checkpoints still needs validation.

Simply ignoring the failed flag, multiplying its cutoff until Veneto passes, relaxing the PPML score tolerance, or deleting extreme observations would not be an adequate fix. Any repair must retain the intended reconstruction guarantee, independent FE and finite score checks, rank decisions, and final-Hessian synchronization. A focused cancellation fixture with an independent arithmetic oracle should precede a bounded full-panel verification against the preserved ppmlhdfe fit. A successful full-panel fit would still need its coefficients, covariance, sample, objective, and convergence certificates checked.

No repair has been implemented in production, and no downstream convergence claim is made. This addendum establishes a concrete numerical cause and a focused repair target without restarting the benchmark campaign.

## Experiment audit

- **7535359:** original frozen binary, four threads, exact input. Native failure captured in 10.73 seconds. The controller expected a different automatic Stata log filename and stopped before the planned fourteen-thread call. Scheduler `failed=0`, process exit 1; this controller error is distinct from the captured application `r(430)`.
- **7535388:** the isolated build controller initially called the build script outside its required working directory. Cargo reported a missing `Cargo.toml`; no estimator ran. Exit 101. Evidence was preserved, and the working-directory correction was made in a separate attempt.
- **7535389:** corrected build plus four- and fourteen-thread diagnostic calls. Both application sentinels and the final job sentinel are present; scheduler `failed=0`, exit 0. Both fits intentionally retain their native `r(430)` outcomes. The Linux build's binary audit and actual-plugin ABI/self-test pass.

All jobs are terminal. Total scheduler wall time across these three fourteen-slot jobs is 129 seconds, or approximately 0.50 reserved core-hours; queue time is additional. These figures describe diagnostic resource use, not estimator performance. Detailed [accounting](accounting.json), [original result](original-result.json), [submission](certificate-buildfix-submission.json), source hashes, and private-log hashes are preserved. Raw Stata logs remain under the ignored local work directory and protected SCC run directories.

Frozen PPML input SHA-256: `92062a6bccaa7b55c3a213e288cf0d22e68668e21f454119ac168dbd938cb08d`.
Original plugin SHA-256: `74d3ce3bd0c01332aaf3a38aea7ee96b7b31012eb6ef0d00782e1d29a6895df9`.
Instrumented plugin SHA-256: `831bfd3f7106761646d531661c6cd1e7f2db699c27d2c2fb45ea290f85aaa8b9`.
