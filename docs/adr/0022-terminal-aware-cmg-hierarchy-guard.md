# ADR 0022: Terminal-aware automatic CMG hierarchy guard

**Status:** Accepted for development; qualification pending
**Date:** 2026-09-01
**Decision owner:** Johannes Schmieder
**Extended by:** ADR 0023 direct-terminal CMG-to-CG guard

## Context

ADR-0021 made common categorical AKM designs eligible for CMG before a MAP
pilot. Later controlled tests found a discontinuous CMG cost when one material
connected component coexisted with hundreds of tiny components. Component
count or largest-component share could not safely identify the cliff because
graphs made entirely of thousands of tiny components remained strong CMG
cases.

Private post-hierarchy diagnostics localized the cost to recursive reuse of
retained hierarchy levels. Define

`W = sum_l visits_l * (vertices_l + matrix_nonzeros_l) /
     (vertices_0 + matrix_nonzeros_0)`,

where the visit count is the product of upstream effective recursive repeat
counts. This quantity is deterministic and available after graph and
preconditioner construction, before nuisance-factor construction or PCG.

## Decision

1. Keep the ADR-0021 cheap topology screen unchanged.
2. When that screen admits automatic CMG, construct the graph and CMG
   preconditioner and inspect its terminal reason and `W`.
3. Retain every direct-terminal hierarchy. Retain an iterative-terminal
   hierarchy only when `W <= 4`.
4. When an iterative hierarchy exceeds that boundary, release the CMG graph
   and preconditioner before LSMR setup and use the existing direct automatic
   Schwarz-LSMR route with certified CG continuation.
5. This veto is a planned automatic route, not a numerical fallback. It emits
   no warning, does not set the fallback flag, does not run a MAP pilot, and
   reports the ultimate used solver. Time spent building the rejected graph
   and hierarchy is included in solver setup time.
6. Explicit `solver(cmg)` is unchanged. CMG setup failures and solve failures
   retain their existing visible safety behavior. No tolerance, iteration,
   memory, scientific-certificate, or publication rule is relaxed.
7. Direct-terminal RHS/thread/nuisance costs are outside this hierarchy guard
   and require a separate calibrated cost model.

This decision adds a post-setup safety screen to ADR-0021 item 3; it does not
replace ADR-0021's categorical/generic split or cheap eligibility thresholds.

## Development evidence

- The first calibration joined 109 directly comparable certified CMG/LSMR
  cells. Every iterative-terminal CMG win had `W <= 2.30`; the first loss had
  `W = 22.44`. A terminal-aware guard rejected 17 costly cases without
  rejecting a measured CMG win.
- A fresh 32-dataset holdout varied graph wiring, panel depth, 0/4/16 controls,
  and 1/8 threads. Its 288 recorded runs used exactly three repetitions per
  solver cell. All runs succeeded and certified. The fresh first
  iterative-terminal loss occurred at `W = 5.68`, while no fresh iterative
  cell favored CMG by more than five percent. Combining the two populations
  leaves a measured gap from 2.30 to 5.68, motivating the conservative round
  cutoff 4 rather than the earlier provisional cutoff 8.
- The post-change rerun recorded another 288 runs. All succeeded and
  certified. Auto used CMG in 90 runs and LSMR in the six guarded runs, with
  zero auto fallbacks, warnings, or MAP pilots. Three deterministic fallback
  events belonged to an explicit LSMR comparator cell, not auto.
- An additional 18-run control rerun showed the high-work mixed graph using
  LSMR and the low-work all-tiny iterative graph retaining CMG in all three
  auto repetitions. Explicit CMG remained available in both cases.
- The post-change development plugin SHA-256 was
  `ff8592ad6d0b3ad9b00027e5de898ab08bceb6d87a1a4d73e8344d341afbc680`.
  The focused licensed Stata route fixture, including both hierarchy sides and
  explicit CMG, passed.

The calibration, fresh holdout, and post-change run-level evidence are under
`benchmarks/reports/development/cmg-hierarchy-guard-calibration-v1/`,
`cmg-hierarchy-guard-holdout-v1/`, and
`cmg-hierarchy-guard-postchange-v1/`.

## Consequences

- Auto no longer enters the measured repeat-amplified iterative CMG cliffs.
- The guard pays a small, measured graph/hierarchy setup cost before routing
  to LSMR; this cost is visible in timing diagnostics.
- All-tiny disconnected graphs remain eligible because terminal reason and
  hierarchy work, not component count, govern the decision.
- Direct-terminal costs are governed separately by ADR-0023; this
  iterative-terminal safety rule remains unchanged.
- This remains development evidence. Frozen, cross-platform, and release
  qualification are pending.
