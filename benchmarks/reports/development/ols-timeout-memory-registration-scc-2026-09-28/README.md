# Large degree-two OLS: memory-routing diagnostic registration

This source-bound audit identifies a concrete admission hypothesis for the
current large degree-two OLS timeout and registers a bounded test. It does not
repair the estimator, establish the timeout's cause, or change any benchmark
classification. Runtime diagnostic results are pending.

## Observed discrepancy

Accepted package **161b9491 / c09eeab3…**, Linux plugin **59d0341a…**, public
`fereg y x1-x10, absorb(g1 g2) threads(4)`, N3,932,160. The frozen current sweep
**7766045** records:

| Warm seed | Node | Result | Command seconds | Peak process RSS |
|---|---|---|---:|---:|
| 104729 | scc-wj2 | completed, LSMR reported | 14.682 | 3.99 GiB native scalar |
| 8675309 | scc-wd2 | command timeout | >720 | 2.65 GiB process-tree sampler |

These are different seeds and nodes, not an alternating timing comparison.
The RSS observations use different reported measures and are diagnostic only.
The first receipt reports 535,570,939,904 available bytes. The timed-out fit has
no published solver/memory result, so its actual admission frame is unknown.
`observed-receipts.json` preserves the exact exported receipt and table hashes.
The timed-out input SHA is
`68eaf6440d99d4c0a4c61ad4205a2483942dbae71cba0eb2ffd00da2c31f1c80`.

## Source and scalar attribution

The generator fixes this graph independently of the seed: 1,966,080 worker
levels and 49,152 firm levels. Seeds change the numerical columns/outcome.
At T4 and width 11, the OLS Schur guard is excluded; degree two also excludes
the CMG topology preference requiring at least three rows per largest FE level.
The public path therefore requests the automatic direct LSMR route, subject to
its complete memory admission. A refusal continues to certified CG.

The current Schwarz model uses the accepted ADR0228 bound `T + R`, with
`R = pair_level_incidence / 10002`. It still combines that worst-case component
count with global maximum-size scratch banks. For these dimensions:

| Quantity | Current estimate |
|---|---:|
| Pair-level incidences | 2,015,232 |
| Potential yielding components R | 201 |
| Banks per apply, T + R | 205 |
| One apply | 22,651,320,224 bytes |
| Four retained pools plus eleven live RHS applies | 339,769,803,360 bytes / 316.435 GiB |

This is a **lower bound on the full estimate**, omitting recurrence, operator,
execution and other storage. It alone exceeds the usable budget of a 256-GiB
host, even before the existing 20% headroom is applied. It fits within the
headroom-adjusted 512-GiB frame, but a lower bound cannot prove full admission.
The successful fit's actual peak is much smaller than this conservative bound.
Source-bound scalar reproduction is in `harness/admission_lower_bound.py` and
`admission-lower-bound.json`; no estimator, matrix construction or fit ran for
this calculation. The 259 accepted computational inputs still match exactly.

Thus a memory-induced route change is a specific explanation worth testing.
It is not yet measured attribution: seed-specific convergence, the exact failed
fit's available memory and the fallback's behavior still need observation.
Do not remove admission or weaken its safety margin on this evidence.

## Registered test

**7767269**, held behind **7766045, 7766146 and 7767016**. Root:
`/projectnb/welfgr/fereg/dev/20260928-ols-timeout-memory-01`.
Four slots, 6G/core, 15-minute wall limit. The `mem_total=480G` filter supplies a
host large enough for the controlled admission experiment; it is not a memory
reservation or queue preference. Actual consumption is reserved at 24G, above
the observed whole-fit footprint, with a separate 20-GiB process-tree RSS stop.

Two fresh Stata processes use the exact timed-out seed/input and unchanged
accepted artifact, sequentially on the same host/core subset:

1. Developer admission limit **448 GiB**.
2. Developer admission limit **192 GiB**.

Each limit must be no larger than freshly observed Linux `MemAvailable` after
input generation; otherwise the diagnostic fails before launching that fit.
This only restricts the current physical-memory frame. Both retain the native
headroom policy, automatic solver, public tolerance, inference and all scientific
checks. Each process is limited to **180 seconds including loading/extraction**;
these are not 180-second command limits. Expected or unexpected timeouts remain
evidence. There is no automatic retry, new stock fit or compilation.

Phase/solver traces, scalar/matrix logs, samples, available-memory observations,
RSS peaks, exact source/plugin/input identities and process outcomes are retained.
Diagnostic clocks are explicitly ineligible for headline comparisons. The two
budgets alter the scientific computation route, so any successful diagnostic
output remains a diagnostic until separately validated; this is not a candidate
acceptance or qualification job.

Require clean scheduler accounting, the diagnostic terminal marker, both
per-attempt records and input hashes, and the actual traces before deciding the
cause. A completed harness does not imply both fits succeeded. If a larger
budget admits fast LSMR while the smaller one enters a slow fallback, the next
implementation should tighten ownership/topology bounds or bound actual live
RHS scratch, with independent memory and numerical checks. It must not simply
increase the admitted memory. Hold any further headline work until this final
diagnostic dependency finishes if the sweep leaves pending groups.

## Harness validation

Seven focused local tests pass: inherited overrides are removed; budgets cannot
exceed physical availability or use malformed values; available-memory units
are checked; ordinary completion is preserved; timeout and descendant-RSS stops
kill/reap the full process group; monitor errors also clean up. Exact staged
Python AST and shell syntax checks pass. The job reruns these seven tests in the
pinned SCC runtime before the two fits. No licensed Stata test, Rust build or
new performance result has run in this milestone.

Deployment SHA:
`0bf6569ab1a8c0a20617a20df46b4a8a17aea7a21a97632cc07451a3ef784cf4`.
The frozen sweep controller, estimator source, original timeouts and 77/16/111
definitions are unchanged. The broader speed, core-scaling, memory and scientific
issues remain open.
