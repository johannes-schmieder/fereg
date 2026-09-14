# ADR 0067: guarded PPML Schur checkpoints

**Status:** Accepted for local development
**Date:** 2026-09-14

## Decision

Extend ADR 0027's canonical checkpoint mechanism with the existing two-FE weighted
Schur accelerator. The canonical session and its independent certificates remain
authoritative. ADR 0029's ordinary auto/LSMR selection and joint engine are unchanged.

Only an already admitted checkpoint-LSMR session may use this extension. It must
have exactly two categorical intercept effects, an existing Schur workspace, at
least 16384 observations, and 16–64 actual RHSs. The existing eight-step three-column
operator probe is evaluated with current normalized weights. Contraction at most
0.01 permits a full attempt capped at 64 iterations within the remaining configured
budget. The probe is diagnostic routing evidence, not a scientific certificate.

Reuse the retained graph and fallibly copy authoritative checkpoint inputs into
one transient candidate. Existing checkpoint admission already charges larger
serial LSMR transport/output banks beside the retained design and ordinary engine.
The candidate must fit that admitted transient allowance. No second projected
design or stale weight-dependent numerical state is retained across frames.

The shared recurrence uses PPML's original raw reference norms and exact requested
canonical certificate tolerance. Do not import OLS rank or tolerance policy.
A finite capped/uncertified candidate is dropped before LSMR works on the untouched
original inputs; its tighter independent retry and MAP fallback remain. Allocation,
invalid input/data, internal and interruption errors are fatal. The shared Schur
entry clears its scalar recurrence banks, including after a capped attempt and
later MAP continuation. Final MAP fallback retains both Schur and LSMR diagnostics. Active-basis freeze,
strict rollback, final-Hessian synchronization, prediction, covariance, saved effects
and transactional publication remain unchanged.

Successful Schur work uses the existing MAP accelerator report identity. The
checkpoint family still identifies the admitted LSMR policy; the concrete ordinary
engine remains LSMR. Rejected Schur work adds a checkpoint retry and warning, and all
probe/copy/attempt costs remain in aggregate solver/command times. Private tracing
separates Schur outcomes. This extends ADR 0027's computation/reporting mechanism;
it does not add a public solver family or close P0's physical-attempt accounting.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-schur-checkpoint-adoption-local-2026-09-14/README.md)
records three-seed paired complete-command gains of 27.9%, 40.1% and 14.3% on unit,
FW and PW targets. Native peaks fall about 22–30%; the selected memory models are
unchanged. Candidate auto also beats accepted explicit MAP by 32.3–35.6% on all
three FW inputs. Weak/current-weight probe rejections, rank/slope/three-FE/low/small
controls, independent Poisson/covariance oracles, exact and one-byte-short public
admission, and original-row saved outputs pass. Largest protected added time is 6 ms.

The existing narrow, one-FE, three-FE, slope and low-memory routes are preserved.
No cross-frame Schur numerical cache, general memory reduction, external-estimator
speed advantage, release qualification or new CMG region is claimed.
