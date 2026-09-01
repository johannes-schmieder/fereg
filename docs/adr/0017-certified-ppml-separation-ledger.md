# ADR 0017: certified PPML separation and sample ledger

**Status:** Accepted
**Date:** 2026-08-31
**Decision owner:** Johannes Schmieder

## Context

Statistical separation is a sample-defining part of PPML, not an IRLS
convergence heuristic. A detector that mutates arrays as it runs can leave
outcome, regressors, weights, fixed effects, clusters, offsets, and output row
maps on different samples. A detector that drops observations merely because
their fitted means are small can also turn a numerical symptom into an
uncertified scientific decision.

The stale separation modules contain useful method implementations and tests,
but their one-pass coordinator is not the production contract. The new PPML
path needs one owner for row identity and mutation, independently certified
detector proposals, and a fixed interaction between separation, singleton
pruning, topology rebasing, and finite-regressor rank.

## Decision

### Ownership and detector interface

1. The PPML sample ledger is the sole owner of active-row state. It maps every
   compact row to a stable original observation and records candidate
   exclusion, missing source, invalid-domain failure, singleton deletion, and
   separation deletion as distinct events.
2. Separation detectors receive an immutable prepared view. A detector may
   return only:

   - sorted unique original-row identifiers proposed for deletion;
   - method name, status, iteration count, and numerical diagnostics;
   - a detector-specific recession-direction or exact combinatorial
     certificate; and
   - an explicit no-separation, skipped, nonconverged, or uncertified status.

   It cannot mutate the production mask, topology, sources, or warm IRLS
   state.
3. The coordinator validates proposal bounds, uniqueness, active-row
   membership, method eligibility, sign convention, and certificate before
   accepting any row. One invalid row rejects the complete proposal.
4. Accepted rows are unioned deterministically in original-row order. If more
   than one method proves a row in the same round, the first certified reason
   in the registered method order is authoritative. Detector-internal order is
   never public behavior.

### Recession-direction certificate

5. Use one fixed sign convention. For a direction `gamma` in the model span,
   its observation-space value `z` must be zero on every positive outcome,
   nonpositive on every zero outcome, and strictly negative on every proposed
   separated observation.
6. A numerical certificate independently checks:

   - relative distance of `z` from the finite-plus-FE model span;
   - equality on the positive-outcome interior;
   - absence of positive boundary violations;
   - a strictly negative, scale-normalized margin for every proposed row; and
   - finite values and nonzero direction scale.

   Exact FE zero-level deletion uses an exact combinatorial certificate rather
   than manufacturing a numerical direction.
7. Positive prior weights and offset/exposure do not alter the homogeneous
   separation cone. They still take part in initial sample formation and all
   arrays are rebased after accepted drops.
8. Heuristic low means, extreme eta, deviance stagnation, or an uncertified
   optimization direction are not deletion evidence.

### Registered sequence and fixed point

9. Default separation methods are exact fixed effects, simplex, and iterated
   rectifier (`fe simplex ir`). `mu` is optional and is not part of the
   default. Explicit requests may select a subset or disable proactive
   detection, but no requested method may be silently substituted.
10. Each separation round uses this order:

    1. iterative singleton pruning unless `keepsingletons`;
    2. exact zero-only intercept-FE deletion to a fixed point;
    3. bounded full-model simplex over the finite-plus-FE model span;
    4. comprehensive iterated rectifier; and
    5. optional `mu` detection when its required fitted start exists.

    A slope-only FE channel does not prove exact zero-level separation.
11. After every certified direct-method deletion, the coordinator:

    - applies the complete resulting singleton cascade;
    - reruns the exact zero-only FE fixed point;
    - commits the union as one sample-ledger generation;
    - compacts every row-scoped source together;
    - rebuilds/rebases FE and cluster topology once; and
    - restarts the requested method chain from the beginning.

    The full process ends only when a complete chain accepts no further drop.
12. Singleton status follows the registered weight experiment: fweights use
    exact literal multiplicity, while unweighted and pweighted samples use
    physical retained rows. Singleton and separated counts and reasons remain
    separate even when a separation drop causes a later singleton cascade.

### Method failure and memory behavior

13. Exact FE deletion is deterministic and cannot return an approximate
    success. A numerical simplex or IR success requires its independent
    certificate.
14. Nonconverged or uncertified default IR is fatal. The comprehensive method
    cannot be treated as an optional best-effort warning when it was requested
    or is part of the default.
15. Before materializing simplex storage, checked arithmetic constructs the
    full-model width from the finite columns plus every finalized absorbed
    level and channel. Intercept effects contribute one indicator per level;
    slope channels contribute the corresponding level indicator times the raw
    slope source. The preflight bounds the jointly live explicit design,
    topology/FE-code construction scratch, null-space work, phase-I tableau,
    and certificate/result buffers in eight-byte cells. If that peak exceeds
    the configured cap, the detector reports `SkippedMemory` without
    allocating its N-by-P design or tableau.
16. A memory planner may skip simplex only when a later requested
    comprehensive IR run completes on the same ledger generation and
    certifies the final no-unhandled-separation state. A simplex-only request
    that cannot fit returns allocation failure (`r(909)`) rather than silently
    continuing. An IR drop restarts the complete method chain; it does not
    retroactively certify a skipped simplex on the prior generation.
17. Optional `mu` may use a fitted predictor to propose rows. Any accepted
    `mu` proposal invalidates the complete fitted session, triggers the same
    ledger/rebase transaction, and restarts separation and PPML from canonical
    prepared inputs. It must supply the same model-span and sign certificate as
    other numerical methods.

### Final-sample handoff

18. When the separation fixed point is reached, the coordinator verifies that
    at least one observation and positive weighted outcome remain. Failure
    returns `r(2001)` atomically.
19. The final prepared model is built at a new sample/topology generation. No
    projected design, FE warm state, cluster subset, rank decision, bread,
    score, or saved-effect product from a prior generation is reusable.
20. Canonical finite-regressor status and rank are recalculated once on the
    final sample as required by ADR 0016. Separation can turn a previously
    identified regressor into an omitted or FE-collinear column; the
    post-separation decision is authoritative.
21. IRLS begins from a fresh certified-linear session. A successful finite
    fit and final score certificate are still required; a separation
    certificate does not certify the estimator.

### Reporting and atomicity

22. Public results report physical candidate count, final physical and
    effective N, singleton count, separated count, requested/executed method
    sequence, method statuses, and aggregate iterations. The fixed-size result
    evidence retains all 14 reason counts and bounded per-method aggregates.
    The receipt binds the exact execution count, initial and final generation
    hashes, and versioned deterministic digests over the complete ordered
    scientific execution trace and every generation hash. Timing and process
    observations are excluded from ledger identity. Full execution and hash
    vectors remain core-internal and test-only; no observation-sized
    diagnostic matrix crosses the Stata result boundary.
23. `e(sample)` and every prediction or saved-output mapping derive from the
    final ledger. Dropped rows are missing in requested outputs.
24. Detection, union, cascade, compaction, or rebase failure leaves the prior
    private generation logically invalid and commits no partial public sample
    or output.

## Required evidence

Qualification covers each method alone; every accepted order and combination;
multiple direct-drop and singleton rounds; no-separation identity; fweight and
pweight samples; zero-heavy and all-zero outcomes; categorical and
slope-bearing one/multiway FEs; offsets and exposure; rank changes after
drops; optional `mu`; constrained simplex memory; and deterministic original-
row results across thread counts.

Each accepted fixture must agree with an independently implemented
explicit-design recession-cone oracle and, where the public semantics overlap,
the exact pinned `ppmlhdfe` closure. Simplex evidence must include finite-only,
FE-only, mixed finite/FE, absorbed-slope, full-positive-rank, degenerate-pivot,
and over-budget cases, and must bind the reported peak to the full explicit
model width rather than only the finite regressors. Tests must prove that an
invalid detector proposal cannot partially mutate production state and that
final arrays reconstruct exactly through stable original-row identities.

## Consequences

- Method implementations can be improved or replaced without giving them
  sample ownership.
- Every production deletion has auditable evidence and a stable reason.
- A sample change invalidates all weighted-linear reuse in one place, avoiding
  stale topology, rank, cluster, or saved-effect state.
- Separation remains estimator-specific even though its projections use the
  shared certified linear mechanisms of ADR 0015.
