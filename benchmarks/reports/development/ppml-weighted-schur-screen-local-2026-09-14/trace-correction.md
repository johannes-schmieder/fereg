# Checkpoint trace activation correction

The six original trace calls pass exact science. They contain Schur phase lines,
but no checkpoint-attempt lines: the shared harness strips inherited FEREG flags
and enables only FEREG_PHASE_TIMING, while checkpoint lines require the separate
FEREG_SOLVER_TRACE flag. Absence of those lines is not absence of LSMR execution.
The complete-command solver matrices independently report two LSMR checkpoints
per auto/CMG fit and six ordinary joint-LSMR frames per auto fit.

Run three additional auto calls on the identical full/warm data. A local wrapper
adds FEREG_SOLVER_TRACE=1 to the harness's post-sanitization thread environment;
all thread settings remain untouched. Require actual checkpoint-attempt lines
and exact full scientific/solver equality with each original untraced auto fit.
Retain the original six captures, with no performance pooling or source edits.
A separate read-only solver display initially failed to parse whitespace-padded
Stata missing values; strip whitespace before identifying unavailable values.
No measurement, tolerance, scientific rule or input is changed by that display.
