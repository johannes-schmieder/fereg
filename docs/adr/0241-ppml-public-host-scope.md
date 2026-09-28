# ADR 0241: Scope Stata processors over the complete public PPML call

Status: accepted development decision, 2026-09-28.

## Decision

The `feppml` wrapper saves `c(processors)` and temporarily sets it to one before
calling the existing `_fereg_ppml` syntax/transaction adapter. It restores the
saved count after success, syntax/domain failure, native panic/allocation failure
and Break, then propagates the captured return code. Setting and restoration
are protected by `nobreak`; the estimator call explicitly permits Break.
The wrapper captures the caller's quiet state and uses `capture noisily break`
with an optional `quietly` prefix, preserving normal-output suppression and
error visibility. A one-processor caller avoids both setting commands.

Stata remains the control plane with one substantive plugin call. The wrapper
neither reparses syntax nor posts results; the existing inner transaction owns
rollback, e(), samples and observation outputs. Rust continues to resolve its
own requested/effective worker count from native availability and policy,
independently of the temporary host setting. Default `threads(0)`, explicit
requests and the existing three-FE ceiling retain their meaning. No persistent
processor or environment setting, numerical buffer, solver route, tolerance,
certificate schedule, rank, weight, inference or sample policy changes.

## Evidence and tradeoffs

[The source-bound report](../../benchmarks/reports/development/ppml-public-host-scc-2026-09-28/README.md)
binds candidate 14c753ed and baseline 7a245969 to identical native artifacts.
Mac and Linux cover real public error/success paths, quiet/noisy behavior,
private injected failures, saved outputs and independent scientific oracles.
Both original native and public-scope genuine PTY Break modes pass. The latter
uses a test-only syntax adapter for private injections; the actual public scope,
native computation and guarded transaction execute unchanged.

All 222 timed applications and 74 native/74 stock comparisons pass. All 50
same-thread native coefficient/covariance pairs are byte-identical; recorded
work counts and PPML memory plans agree. Fresh one-FE T4 and low medians improve
23.8% and 17.6%, with repeated gains in the initial block. Small two-FE improves
11.1% fresh but is flat initially. This inexpensive boundary change is retained
without favorable-case guards. Fresh three-FE loses 2.4%; the initial larger
case loses 5.9%, which does not repeat fresh. Full ranges and losses remain in
the report. The previous native-only scope remains rejected under ADR0240.

Direct scaling improves in seven of eight medians, but one-FE T1→T2 loses 8.0%.
Requested three-FE T8→T16 still means effective 8→8. Universal monotonic scaling,
1.20 stock ceilings and 0.90 stock targets are not established. Small one-FE,
small two-FE and unchanged OLS controls retain substantial stock losses.

Host-worker interference is supported by the preceding isolated boundary
profile and the useful full-command gains. Exact worker activation and the
residual difference from the repeated-fit diagnostic are not isolated. Do not
weaken benchmark import audits or assume an undocumented runtime wait policy.
No native memory admission is reduced; tracked three-FE peak variation retains
its existing workspace-overlap explanation. This is development evidence,
not physical-cap or full release qualification. OLS carryover requires its own
boundary validation and measurement before adoption.
