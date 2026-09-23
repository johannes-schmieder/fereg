# ADR 0120: Close the direct PPML slope extrapolation probe

Status: rejected for adoption after bounded local development, 2026-09-23.

## Decision

Do not add the tested per-RHS Irons–Tuck extension to the reusable generic
PPML slope MAP path. It reduces projection work, but fresh baseline/candidate
certification absorbs almost all of the savings on the two representative
full-size cases. A 0.7% paired median loss and a 2.8% gain do not justify the
additional storage and a new fit-lifetime owner. The small single-run pilot
gains do not justify a fitted row-count guard. Do not tune the clamp, pilot,
acceptance rules or seeds to rescue this probe.

The isolated experiment used the existing categorical extrapolation helper
after four ordinary sweeps. Each candidate required a strictly smaller fresh
relative two-norm and a nonincreasing fresh infinity norm. The first degenerate
or nonimproving proposal resumed ordinary scheduled MAP for the rest of that
call. Every requested final certificate, rank, score, objective, Hessian and
publication check remained mandatory. Temporary buffers were deliberately
not a production admission or allocation-free implementation. No state was
reused across weight frames. Generic CG was not substituted: its known
nonretaining slope model-space boundary remains unresolved.

Production remains at ADR 0119. Keep the experiment and builder in immutable
receipts rather than adding a new selectable development route to main.
Retain the useful certificate phase instrumentation. All diagnostic builds
stay separate from stock and production timing claims.

## Evidence and next boundary

The [local report](../../benchmarks/reports/development/ppml-slope-extrapolation-local-2026-09-23/README.md)
records all 28 application calls, two independent explicit-design pilots,
public-state audits, repeat checks, build corrections and exact identities.
Samples, beta, covariance, means, objectives and public scores/model space
pass; the experiment is rejected on practical cost, not a scientific failure.

Projection counts fall 1368→768 and 1140→760 in isolated profiles, but
certificate counts rise 134→170 and 73→94. Current-weight basis normalization
and QR consume about .224/.091 seconds on the baseline cases; raw moments
consume .132/.092 seconds. Fine timers have measurable overhead and are not
headline speed evidence.

Next investigate certificate geometry scoped to one absorption invocation:
construct it using that call's weights, retain only admitted capacity between
calls, and invalidate weighted geometry unconditionally at the next call.
Every residual norm, moment, span projection and final certificate must still
be freshly computed. Establish independent changing-weight/rank tests and a
memory fallback before adoption. This is a new geometry-lifetime investigation,
not permission to reuse an OLS cache across PPML generations. Materially lower
certificate cost could justify a later, separately declared acceleration
experiment; the present probe is closed.
