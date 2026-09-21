# ADR 0092: compact Gram storage and ordered serial slope passes

**Status:** Accepted for local development under the approved optimization plan
**Date:** 2026-09-21
**Decision owner:** Johannes Schmieder

## Decision

The ADR-0087 single-partition OLS slope cache stores each group's actual
channel-square Gram matrix in one packed vector. Projection copies each small
matrix into the existing bounded elimination workspace. Construction, retained
and absorption estimates charge the actual representation and capacities.
The rank rule and independent QR basis remain unchanged.

When this cache is built and used with one worker and its validated group codes
are nondecreasing, projection and reconstruction process one group at a time.
The resulting traversal has the original observation order, so residual norm
accumulations preserve their exact arithmetic. Reconstruction also records
fresh weighted residual maxima for each group, using the existing coefficient
bank. The subsequent independent QR and compensated raw-moment checks consume
those maxima rather than rescanning the residual for them.

Only maxima and norms of the current residual are reused. Earlier residual
certificates are never reused. Provided-weight norms retain their raw scale
inside the slope solver and are not handed to OLS's normalized-weight frame.
The separate norm/certificate path remains available for shuffled inputs and
multiple workers. A changed worker team cannot activate the ordered route
unless both construction and absorption meet its serial guard.

No new observation-scale owner, solver-family choice, tolerance, rank threshold,
sample rule or public option is introduced. The existing output/coefficients/
check-bank reservation remains conservative for the fused route. Errors and
failed certification preserve the untouched original input for the established
fallback. Eligibility outside ADR 0087 is unchanged.

## Evidence boundary

The [slope checkpoint](../../benchmarks/reports/development/serial-slope-fusion-local-2026-09-21/README.md)
records the source-bound fixtures, independent certificates, 87 paired-screen
calls and fourteen diagnostic calls. Thirty native pairs have exact reported
results and all eighteen saved-output pairs match exactly. It also links the
earlier compact-Gram experiment. The serial K10 gain is modest and the measured
serial weighted/reference gaps remain open. This decision does not close the
broader graph/routing milestone, final blocks, allocation-proof gaps or Linux
confirmation, and is not release qualification.
