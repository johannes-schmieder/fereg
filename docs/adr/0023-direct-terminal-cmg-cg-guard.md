# ADR 0023: Direct-terminal automatic CMG-to-CG guard

**Status:** Accepted for development; qualification pending
**Date:** 2026-09-01
**Decision owner:** Johannes Schmieder

## Context

ADR-0022 removed the measured iterative-terminal CMG hierarchy cliffs but
deliberately retained every direct terminal. A subsequent RHS/thread
calibration found material direct-terminal CMG losses, especially with serial
multi-RHS work and nine nuisance columns. Its first proposed destination,
direct Schwarz-LSMR, was not reliable: requested-tolerance LSMR fell back to
CG in 14 common holdout cells, and tightening its tolerance merely moved the
runtime cliff.

Private per-RHS diagnostics reconciled LSMR's internal stopping metric with
fereg's publication certificate. In one representative 17-RHS solve, two
columns missed the fereg tolerance by only 9--11 percent at `1e-10`; `1e-11`
fixed that case with two additional iterations. A different ordinary five-RHS
one-thread case then exceeded the bounded diagnostic runtime at `1e-11`.
Tighter LSMR is therefore not a safe deterministic destination for this
guard. Direct CG certified without fallback throughout the relevant
calibration and holdout populations.

## Decision

1. Keep the ADR-0021 cheap categorical AKM eligibility screen and the ADR-0022
   iterative-terminal hierarchy guard unchanged.
2. For an auto-eligible hierarchy whose terminal reason is `Direct`, compute
   the locked deterministic cost score from hierarchy depth, repeat-weighted
   work, observations, nuisance columns, initial RHS width, effective threads,
   and terminal-factor presence.
3. Prefer direct CG only when the cost score rejects CMG, hierarchy
   depth-work exceeds 200 per observation or there are at least nine nuisance
   columns, and execution is serial or all initial RHS columns fit within the
   requested thread width. Retain CMG otherwise.
4. Release the rejected CMG graph and hierarchy before nuisance-factor or PCG
   work. Charge the already-spent graph/hierarchy time to solver setup.
5. A planned direct-CG selection is not fallback: retain requested solver
   `auto`, report used solver `cg`, emit no warning, run no MAP pilot, and set
   no fallback flag. Direct CG uses the unchanged requested tolerance and
   iteration budget and must pass the ordinary independent certificate.
6. Explicit `solver(cmg)`, explicit `solver(cg)`, setup-failure behavior, and
   all scientific, memory, coefficient-recovery, and publication contracts
   remain unchanged.

This is intentionally the conservative locked candidate. A more elaborate
serial 17-RHS clause improved aggregate calibration time further but was not
adopted because it combined three additional thresholds and had less support.

## Development evidence

- The reliable-destination calibration contained 61 datasets and 366 recorded
  runs; the fresh holdout contained 54 datasets and 324 runs. Both used exactly
  three repetitions per solver cell. Direct CMG and direct CG certified in
  every run without fallback.
- The locked primary rule selected CG in 20 calibration and 15 holdout cells.
  Before implementation it displaced no CMG win above five percent in either
  population. On the fresh holdout its aggregate command time was 1.376 times
  faster than retaining CMG everywhere.
- The post-change replay contained 54 datasets and 486 recorded auto/CMG/CG
  runs. Observed auto routes matched all 54 locked decisions: 15 CG and 39
  CMG. All runs certified, with zero auto fallback cells. Aggregate
  current-CMG time divided by observed-auto time was 1.339.
- One no-control topology had high eight-thread timing dispersion in the main
  replay. A bounded 18-run paired recheck at one and eight threads again
  matched both locked CG decisions, certified without fallback, and favored
  CG in both cells; observed auto regret was at most 1.055 and aggregate
  current-CMG/auto time was 1.300.
- The post-change universal plugin SHA-256 was
  `debb95dcaf648aa11c439deb2861971aeebb34fd707ff5608c6f371b39f7c458`
  and passed binary audit plus the actual-plugin ABI/self-test harness.

Source, raw runs, medians, locked decisions, post-change replay, and paired
recheck are under
`benchmarks/reports/development/cmg-direct-terminal-conclusion-v1/`.

## Consequences

- Auto avoids a conservative, well-replicated subset of direct-terminal CMG
  losses without introducing routine fallback or another LSMR tolerance cliff.
- The rule deliberately leaves some large CG opportunities on the table. The
  fresh holdout still contains five routed cells above twofold oracle regret,
  all from retained CMG. These are performance misses, not regressions against
  the previous all-CMG direct-terminal policy.
- Further threshold mining is paused. The next material performance work
  should improve solver implementations or reuse—not add another routing
  clause—unless a new representative workload demonstrates a reproducible
  common regression.
- This remains development evidence. Frozen, cross-platform, and release
  qualification are pending.
