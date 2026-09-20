# Cost of useful full-width OLS pilot work

Local diagnostic measurements on 2026-09-20 on M2 Ultra, Stata/MP 19.0
(8 licensed processors), with 16 requested native workers. The embedded build
date is September 19. Production remains `3fa869fa`; no installed package or
remote state changed. This experiment does not implement state reuse or count
as a performance win.

The isolated `fullpilotcost` artifact uses base `3fa869fa` plus a generated
one-file diagnostic overlay. For 3–8 RHS columns, a separate probe advances
all columns and accumulates their solutions, then still discards that state.
Admission uses the original selected columns 0, 1 and width/2. Invalid
nonselected step denominators are recorded; none occurs in these eight calls.
The original three-column shared/PPML function is byte-for-byte unchanged.
The generated overlay replays exactly; the other 515 source files match.
Plugin `c9f1837fa142cc05ad9e9902b6af4b70f7577a2e4a36fe37201f58fa93665e5a`
passes the actual-plugin ABI/panic check. The parent is `fusedreconstruction`.

## Diagnostic timings

One paired trace per cell, in milliseconds. These are pilot costs including
its graph allocation, initialization and eight steps, not final retained-state
speedups. The final ordinary solve still initializes and runs separately.

| Cell | Three-column pilot | Full-width pilot |
|---|---:|---:|
| Shuffled unit K=5, N=983040 | 23.120 | 39.237 |
| Shuffled AW K=6, N=983040 | 34.703 | 46.414 |
| Degree-three unit K=6, N=245760 | 6.969 | 9.458 |
| Degree-three AW K=6, N=245760 | 6.922 | 9.378 |
| Degree-six unit K=6, N=245760 | 6.155 | 6.646 |
| Degree-six AW K=6, N=245760 | 7.669 | 8.257 |
| Unbalanced unit K=6, N=245760 | 6.327 | 9.593 |
| Unbalanced AW K=6, N=245760 | 7.283 | 9.749 |

The roughly 0.6–2.5 ms weighted small-graph premium is promising because the
current full solve repeats initialization and all eight pilot steps. The
shuffled premium is larger; profitable reuse there is unproven. All total
application times are preserved, including the slower diagnostic cases.
No default-fixest arm was run for this internal cost question.

All sixteen application calls pass their explicit result/sentinel checks.
All eight native pairs preserve b, V, status, DoF, certificate and fit files
bitwise. Every non-timing solver field also matches exactly, including the
full-precision probe contraction and selected family. No native allocation
peak increases. This validates the measured cases only: near-absorbed extra
columns, breakdown, rejection, warm continuation, certificates and iteration
charging still need dedicated tests before production adoption.

Next build an opaque same-call pilot handoff that owns the original values,
full graph and recurrence scalars. An accepted, valid state must enter the
existing certified solver without resetting its initialization or eight useful
steps. Additional columns that cannot safely continue must preserve the old
selected-column decision and use a certified fallback. Rejecting the probe
must return untouched values, release the larger workspace, and report its
actual cost and peak. Charge every completed step once and retain the original
shared total budget; do not silently cut the former eight-plus-64 allowance.
Measure the resulting candidate on the existing shuffled and small-graph cells
before admitting it, including weighted cuts, low memory and protected routes.
The 111 controls, sixteen fresh holdouts and two final five-pair primary blocks
remain outstanding.
