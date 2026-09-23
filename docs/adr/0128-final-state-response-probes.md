# ADR 0128: reject broader exact-final-state response probes

Status: rejected for production, 2026-09-23.

Two source-captured probes extended ADR 0127's strict-confirmed point retention
from profiled one-FE fits to other publication PPML engines. The first retained
the accepted point and its exact weights; the second also reused a certified
response projection as an initial representative for a same-weight response.
The response history had explicit one-use generation authority, revoked on
unsolved refresh, weight/config/basis changes, rollback, rebuild and errors.
Original-response finite, score, reconstruction and final-Hessian checks remained.

Both pass their focused science. Nevertheless, a cold response projection can
offset the avoided control projection, and correcting that consequence yields
small, mixed public gains. In the second eight-cell screen, candidate/baseline
medians range from 0.968 to 1.011. Low memory is unchanged and the wider slope
case loses. Do not adopt the additional state machine or continue threshold
tuning. Restore accepted ADR 0127 behavior; captured source includes both probes
and their boundary tests.

This rejection does not establish that exact-state factor reuse is ineffective.
These probes did not remove repeated finite-system construction. A successor
should change that construction or the ordinary joint solve materially and
preserve independently fresh acceptance. See the
[source-bound report](../../benchmarks/reports/development/ppml-final-state-probes-local-2026-09-23/README.md).
