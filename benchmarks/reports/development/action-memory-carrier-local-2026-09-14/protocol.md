# P0 action memory reporting: prospective implementation/evaluation

Evidence HEAD d0c0e86; accepted native25bf610, isolated plugin e6978061.
Main and installed dist are not built. Preserve unrelated STATUS/untracked files.
Reuse modeled quantities at existing admission boundaries, carrying the maximum
across actual construction/solve/recovery/inference and discarded/fallback attempts.
Keep the base execution model distinguishable from the combined action model.
All arithmetic, routing, actual admission decisions and scientific output remain.

The previous u128 reporting prototype had incomplete coverage and recurrent
weak-wide costs. Evaluate a compact checked u64 report carrier that preserves
8-byte alignment, rather than assuming it cures the cost. No accounting is added
inside numerical iteration loops. Values beyond addressable u64 storage fail closed;
model calculations/admission retain checked u128 arithmetic. Record unmodeled
coverage explicitly until all production families are covered; do not publish a
partial sum as a complete model.

Validate meaningful report max/overflow/fallback/retained-overlap boundaries and
independent numerical oracles in isolated sources. Use the original three-FE
FW generated control from the OLS residual experiment and the earlier reporting
prototype's original weak-wide case, plus small/narrow, one-FE, CMG, slope,
low-memory/inference/recovery and changing-weight PPML controls as the changed
coverage requires. Fixed command/cap tests follow complete source coverage.
Before timing, bind requests and acceptable costs: preserve exact scientific and
non-time solver fields; report changed memory fields separately. Investigate any
cost above both5%and10ms; preserve every failure/timeout/retry. No tests/builds/
oracles/generation/compression overlap measurements. No speed gain is required
for correct accounting, but recurring weak-graph regressions are not accepted.

P0's pass/copy/build/physical-attempt reporting remains part of the full objective,
not closed by the memory carrier alone. R1–R5 stay excluded; independent reviews,
upstream assessment and SCC follow completion of P0–P7.
