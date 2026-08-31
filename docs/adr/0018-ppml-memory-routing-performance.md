# ADR 0018: PPML memory, routing, and performance contract

**Status:** Accepted
**Date:** 2026-08-31
**Decision owner:** Johannes Schmieder

## Context

PPML repeatedly solves weighted fixed-effect least-squares problems whose
weights change with the current mean. The stale implementation allocates new
working vectors and an observation-by-parameter transformed design during
iterations, copies candidate columns, runs a separate legacy rank solve, and
performs another profiling pass after convergence. That design prevents the
main `fereg` optimizations from carrying over and makes memory use proportional
to both problem size and avoidable iteration churn.

ADR 0015 provides a reusable certified-linear session, but reuse is safe only
with explicit lifetime, invalidation, routing, and fallback rules. PPML also
needs its own performance gate: matching the comparator scientifically while
silently taking a slow duplicated path is not completion.

## Decision

### Fit-lifetime storage

1. One PPML fit owns one checked allocation plan covering simultaneous peak
   storage for:

   - prepared outcome, finite-source recipes, prior weights, offset, stable row
     map, FE topology, and cluster topology;
   - separation proposals, certificates, and the largest permitted detector
     workspace;
   - eta, mu, Hessian weight, working response, accepted/candidate state,
     score, and line-search buffers;
   - weighted transformed design or its low-memory scratch representation;
   - moment, rank-solve, FE-solver, preconditioner, and certificate workspaces;
   - final sandwich, cluster-subset, saved-effect, requested-output, and
     private publication buffers; and
   - the bounded iteration diagnostic record.

2. All data-sized arithmetic uses checked dimensions and fallible reservation.
   `memory(fast|low|auto)` is resolved before the first large fit workspace is
   allocated. If no certified route fits the applicable limit, return `r(909)`
   without partial output.
3. Fast mode may retain a reusable in-memory transformed design and the
   qualified dense reduction workspaces. Low mode streams bounded column/RHS
   pools and may use the same validated private scratch-file discipline as
   OLS. Auto prefers the qualified faster plan that fits and otherwise selects
   low. Fast and low must produce the same scientific result and statuses.
4. Scratch names, bounds, offsets, finite values, reads, and writes are
   validated; cleanup occurs on success, ordinary error, panic unwinding, and
   interruption.

### Reuse and allocation discipline

5. Eta, mu, current Hessian weights, working response, candidate eta/mu,
   scores, and line-search storage are allocated once and swapped or
   overwritten. Steady-state IRLS performs no data-sized allocation. A bounded
   diagnostic append is the only permitted iteration-dependent allocation.
6. The canonical prepared design and topology remain available for a fresh
   recomputation. A warm residualized design is an optimization, never the
   only source of truth.
7. Weight, response, sample, topology, and kept-basis generations obey ADR
   0015. A weight refresh reprojects and restandardizes every product required
   by the new Hessian frame. A failed warm projection, moment, rank, or bread
   certificate falls back visibly to the canonical fresh path.
8. The strict final solve reuses the session workspace and canonical inputs.
   It does not allocate a second observation-by-parameter design or run the
   stale duplicate final profiling path.
9. Saving predictions or fixed effects may retain only the outputs requested
   by the transaction. Output requests are included in preflight and cannot
   alter the estimation path.

### PPML-specific route policy

10. PPML shares the certified MAP, CG, LSMR, and CMG implementations, but not
    OLS automatic route thresholds. Route choice is a PPML fit-lifetime policy
    based on final sample topology, RHS width, changing-weight cost,
    conditioning diagnostics, memory mode, and requested products.
11. The initial automatic PPML policy uses only routes with dedicated
    changing-weight and end-to-end qualification. CMG remains explicit and
    diagnostic until its hierarchy reuse, weighted convergence, memory model,
    and fresh-versus-warm results pass the PPML calibration matrix. It cannot
    enter PPML auto merely because OLS auto selects it.
12. A preconditioner, Krylov warm start, projected design, or factorization is
    tagged by sample/topology/weight/basis generation. Generation mismatch
    forces rebuild. Any route-specific certificate failure follows the
    registered fallback chain or fails closed; fallback is recorded and never
    hidden by a later successful step.
13. Inner projection tolerance tightens deterministically with outer deviance
    and score progress. It may save work early but cannot be looser than the
    bound required to make the outer certificate meaningful. The final strict
    tolerance is governed by ADR 0016.
14. Requested threads create one isolated bounded native team per fit. Worker
    threads see Rust-owned data only. Fixed reduction partitions and merge
    orders preserve same-architecture, same-route, same-thread-count
    reproducibility.

### Instrumentation

15. Private diagnostics separately record preparation, separation by method,
    weight/response refresh, FE projection, finite moments/rank solve,
    objective and line search, strict final solve, inference, FE recovery, and
    publication time.
16. The result records requested/effective threads and route, fallbacks,
    outer/inner iterations, step halvings, fast/low estimates, selected bytes,
    scratch peak, allocator start/current/peak/delta, and process RSS where the
    platform exposes it. Instrumentation is observational and cannot certify
    its own scientific result.
17. Allocation-failure and store-failure injection covers every fit-lifetime
    class. Repeated calls must return allocator current state to baseline and
    leave no scratch or requested Stata variable behind.

### Benchmark population

18. The frozen PPML matrix crosses:

    - 100 thousand, one million, and governing ten-million-row scales;
    - zero, moderate, and wide finite-regressor counts;
    - no FE and one-, two-, and three-way categorical FEs;
    - slope-bearing effects, standard and low-mobility graphs, and disconnected
      components;
    - low/high zero shares, offset/exposure scales, separated shares, and easy
      versus long IRLS paths;
    - full-rank, collinear, and ill-conditioned finite designs;
    - unweighted, fweight, and pweight models;
    - robust and one- through five-way clustered inference;
    - threads 1, 2, 4, 8, and 16 where licensed capacity permits;
    - fast, low, and auto memory; and
    - fit-only, predictions, total absorbed effect, and individual saved-FE
      publication.

19. Scientific preflight is mandatory before timing. The comparator is the
    exact pinned `ppmlhdfe` closure, supplemented by the independent oracle in
    ADR 0016. A timing arm with sample, status, coefficient, VCE, DoF,
    objective, prediction, saved-effect, or certificate failure is inadmissible.
20. Governing runs use the same immutable generated data, same node/job, same
    requested application threads when controllable, fresh isolated command
    environments, one cold run, and at least five warm repetitions. Reports
    bind source, tree, package, plugin, comparator, manifest, generator, data,
    OS, CPU, Stata, toolchain, thread, and scheduler identities.

### Acceptance gates

21. On the approved ten-million-row population after scientific parity:

    - the equal-cell geometric-mean `feppml/ppmlhdfe` warm-time ratio is no
      greater than `0.40`;
    - at least 80 percent of approved cells are at least twice as fast;
    - registered single-thread comparisons are at least 1.5 times faster; and
    - no unexplained cell is more than 25 percent slower.

    Ratios are computed only within the same job/node. Development-scale runs
    diagnose regressions but cannot satisfy the release gate.
22. Native allocation must be within 25 percent of the checked route model;
    repeated calls must not leak; and constrained-memory fixtures must prove a
    case where low succeeds while fast fails preflight and a case where no
    route fits and fails before large allocation.
23. Scaling fixtures must show no per-iteration growth proportional to N, K,
    FE levels, or cluster groups, and no unexplained superlinear iteration
    cost. Saved-output overhead is reported separately and remains bounded by
    the larger of 0.05 seconds or five percent on registered fit/save pairs.
24. These PPML gates are additive. Every shared-engine checkpoint and the
    final candidate must also pass the `fereg` direct non-regression contract
    in ADR 0019 and retain the existing external `fereg` gates.

## Consequences

- Changing weights reuse topology and bounded workspace without treating
  stale weighted products as valid.
- Fast and low memory are complete scientific routes rather than a silent
  dense path plus an unqualified fallback.
- Solver improvements can carry across estimators, while automatic policy is
  qualified separately for PPML's changing weights.
- A PPML optimization that misses its scientific, allocation, or performance
  gate is removed without changing the certified one-shot OLS path.
- Public activation remains blocked until the full exact-source matrix and
  platform/transaction gates pass.
