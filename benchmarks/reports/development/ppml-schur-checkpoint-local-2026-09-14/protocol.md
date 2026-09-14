# PPML bounded Schur checkpoint prototype

Baseline evidence HEAD 7996a5e; accepted native 7739e8e and plugin 2689faf0.
Existing screen inputs, seeds, weights, commands, timeouts and scientific
comparisons from ${ROUTE_SCREEN_ROOT} remain authoritative.
No installed dist replacement, main build, concurrent timing work or cluster.

Prototype: only admitted checkpoint-LSMR sessions, exactly two categorical
intercept effects, N>=16384 and actual width 16..64. Reuse the retained graph
for the existing 8-step, three-column actual-weight operator probe (contraction
at most 0.01). Fallibly copy the authoritative matrix within the already
admitted checkpoint LSMR transient envelope. Reuse the shared Schur recurrence
with a 64-iteration cap and exact PPML reference norms/certificate tolerance.
Keep ordinary joint LSMR unchanged. Drop finite capped/uncertified candidates
before original-input LSMR/tighter retry/MAP fallback. Fatal errors propagate.
No N-state retained across frames or OLS rank/tolerance policy copied into PPML.

Acceptance: independent projection and full Poisson/Hessian/sandwich oracles;
original three-route inputs first, then three seeds and protected weak, tiny
weighted cut, bottleneck, disconnected, unbalanced, small/narrow, low memory,
rank-deficient, slopes and three-FE cases. Require repeated complete-command
5% and 10 ms gains vs accepted auto, compare best existing explicit route.
Review every protected added cost exceeding both 5% and 10 ms. Retain failures,
retries, input copies, probe/attempt cost, native models/peaks and sampled RSS.
Traces are separate; no traced or failed command supplies passing speed evidence.
Prototype remains unadopted until those checks and full reporting review pass.
