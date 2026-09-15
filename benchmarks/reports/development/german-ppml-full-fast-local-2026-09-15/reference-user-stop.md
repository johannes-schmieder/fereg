# Full reference stopped at user request

The user stopped the full pinned ppmlhdfe reference because it was taking too
long. The verified reference Stata process received SIGTERM; Stata, its time
wrapper and the runner exited. The runner records process_rc=-15, no Stata
completion receipt and accepted=false. The monitor was deleted; no automatic
restart is scheduled. Partial logs and the stop request remain preserved outside
Git. See [termination receipt](reference-user-stop.json).

Elapsed process time was 17,269.615 s (4h 47m 50s). The last logged outer iteration
was four. Peak RSS was 106,370,842,624 bytes; peak memory footprint was
115,052,055,352 bytes, with zero reported swaps. There is no completed command
time or full reference b/V/prediction export. This is a censored run stopped by
the user, not an observed convergence failure or a completed timing benchmark.

The earlier native application completion remains unchanged. Independent full
reference comparison and native model-space acceptance remain unresolved, as
do the separately recorded 5% comparison and OLS memory-model gaps. No new
numerical audit, reference retry or estimator change accompanies this stop.
