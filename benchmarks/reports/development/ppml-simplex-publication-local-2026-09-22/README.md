# PPML simplex/IR publication repair

The public command now accepts a memory-skipped simplex attempt followed by
certified comprehensive IR, as ADR 0017 already requires. The simplex method's
certificate flag remains false. [ADR 0104](../../../../docs/adr/0104-simplex-publication-certificate-state.md)
records the validator boundary and evidence limits.

The source baseline is `cc6e40fa`. Candidate `separation1` overlays only
`stata/_fereg_estimate_ppml.ado` and the focused validator fixture among the
523 captured inputs. All Rust/C/ABI inputs and the native plugin are unchanged;
the plugin is `357118cece7f4eaf1813e5e7ab25458ca999ae20fb0bed8afc7dd4d6354d58ea`.
The new public fixture is bound separately. This does not rebuild or relabel
the earlier immutable artifacts.

## Failure and repair

An isolated ado insertion dumps the matrices immediately before the original
validator rejection without bypassing it. The original 7,680-row warmup has
6 executions and final generation 1. FE separation drops 60 rows. Simplex's
trace row is 5, status mask 16 (`SkippedMemory`), generation 1 and certificate
flag 0. IR's trace row is 6, status mask 2 (`NoSeparation`), generation 1 and
certificate flag 1. Every execution bound is valid. The failing predicate
incorrectly requires the simplex per-method flag to equal one.

The corrected validator checks status/certificate consistency and allows
unresolved statuses only for simplex with terminal certified IR evidence.
Nonconverged iterations, false success flags, absent/unexecuted/uncertified IR,
generation mismatch and earlier IR chronology remain rejected. Native full-trace
validation still proves the exact same-generation chain; the Stata summary
check is not a replacement for that certificate.

## Scientific checks

The new `feppml_simplex_ir_publication` fixture uses 8,192 observations, two
numeric regressors, 32 FE levels and a deliberately zero-only final level.
The remaining zero boundary exceeds the unchanged default simplex cap.
For unit/FW/PW weights and T1/T4 it verifies exactly 256 FE drops, the final
sample, missing requested means on excluded rows, the truthful simplex/IR
flags and the final estimator certificate. Six independent dummy Poisson fits
check coefficients and means; directly formed Hessian sandwiches check V.
Full rank of the positive-row explicit design independently proves that no
additional separation is possible. A simplex-only request still returns
r(909), restores the preceding regression and publishes no requested output.
The original artifact fails this fixture with r(3499).

The extended `estimate_ppml_runtime_validator` fixture accepts the three
documented unresolved simplex statuses and rejects seven corruptions for each.
Its stale 1.2 receipt minor was corrected to the active 1.3 result wire.
Both licensed fixtures have explicit zero return and terminal sentinels.
Twenty-one release-mode Rust tests with the `simplex` filter pass, including
same-generation closure, cross-generation restart, memory bounds, full model
span and certificate checks. Rust computation is unchanged from the tested
`cc6e40fa` artifact.

A separately predeclared fresh case (seed 202609224, N7,680/K12/T4) has exactly
50 zero-only FE drops. The independent checker forms that sample without
native results, proves full finite rank after demeaning positive rows within
retained FE levels, and fits the remaining 7,630 observations with explicit
SciPy dummies. Native and stock sample/beta/V/objective checks all pass. The
checker and numeric receipt are archived. This is bounded development evidence,
not full PPML/F0 qualification.

## Public-command measurements

The original case is rerun at seed 202609223 alongside a positive control,
with three alternating native baseline/candidate pairs and stock fixest in
rotated order. No compilation, profiling, archiving, independent oracle or
competing fit ran alongside timing. No timeout, overlap or monitor error was
detected. There are 18 attempts: 15 successes and three expected original
artifact warmup failures. The fresh two-call science screen also completes.

| Case | Baseline / candidate / stock median, ms | Distribution and science |
|---|---:|---|
| Zero outcomes, N24576/K12/T4 | failed / 115 / 37 | Candidate 112–116; stock 37–40. All three candidate/stock sample, beta, V, likelihood and deviance checks pass. |
| Positive outcomes, N49152/K12/T4 | 146 / 147 / 53 | Paired candidate/baseline min/median/max 1.007/1.034/1.050; all scientific exports identical. |
| Fresh zeros, N7680/K12/T4, one diagnostic call | — / 50 / 21 | Independent separation and fit oracle passes both. |

The repaired target retains 24,395 of 24,576 observations; its sample matches
stock. The positive control's small measured loss remains visible. This is a
correctness repair, not a speedup. Native allocation and computational paths
are unchanged; the positive control's tracked native peak is identical.
PPML remains substantially slower than stock in these cells.

## Receipts and next work

The first isolated copy stopped on the newly edited validator fixture, leaving
an incomplete artifact. Its validator test passed, but the public test failed
r(601) because the plugin had not yet been copied. A complete artifact was
created separately and both fixtures passed. The first screen stopped before
any fits when Python wrote `__pycache__` into its controller capture. The
harness now disables bytecode writes for captured controllers; the successful
screen uses a fresh root. These failures and original log hashes are retained.

The 450-entry `evidence.zip` contains exact identities, source overlays,
diagnostic matrix output, plans/controllers, all numeric exports and failures,
independent oracles, Rust results and fixture receipts. It excludes data,
binaries and raw licensed logs. The existing allocation-inventory drift,
constrained-fit/CG issues and deferred Linux work are unaffected.

Continue with PPML strict-checkpoint assembly and residual projection. Earlier
diagnostics identify repeated raw predictor/mean work during retained-fit
assembly; profile that boundary before generalizing any one-shot OLS cache.
