# Existing-route Schur/checkpoint attribution

All nine untraced route calls passed their science comparisons and native memory
admission checks. Auto/MAP/CMG seconds are 2.342/2.978/4.142 dense unit,
3.289/2.878/7.203 strong FW, and 2.263/2.451/3.109 dense PW. These are screens.
MAP's lower checkpoint costs motivate testing guarded Schur checkpoint use while
retaining ordinary joint LSMR, rather than blindly replacing the ordinary engine.

Before any implementation, run the same three requests under auto and MAP using
only existing native tracing. Preserve each route's exact scientific exports and
solver decisions against its untraced command. Capture Schur phase records and
checkpoint LSMR attempts; separate warm-up from full calls where existing labels
permit it. Do not present incomplete trace counters as full command pass/build
counts. No new diagnostic binary or public timing aggregate is created here.
