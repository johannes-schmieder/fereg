# ADR 0021: AKM automatic solver-routing recalibration

**Status:** Accepted for development; qualification pending
**Date:** 2026-09-01
**Decision owner:** Johannes Schmieder

## Context

The ADR-0009 MAP-pilot route created a sharp performance cliff on a sparse
worker--firm--county design. Auto ran four MAP sweeps, solved with tight
Schwarz-LSMR, and sometimes discarded a numerically adequate LSMR candidate
because it was certified against the post-pilot scale before restarting CG.
ADR-0010 was also too conservative for ordinary AKM panels with a small third
fixed effect: explicit CMG frequently won even though nuisance columns made it
ineligible for auto.

The development calibration generated each dataset once, rotated solver order,
and recorded exactly three repetitions per arm. It covered 10,000--100,000
observations, two- and three-FE AKM shapes, nuisance and cardinality boundaries,
right-hand-side width, and one/eight threads. A separate 90-case, three-seed
holdout tested the proposed rule before implementation.

## Decision

1. This decision applies to OLS categorical intercept-only designs. Absorbed
   slopes and other generic FE operators retain the ADR-0009 MAP-pilot policy.
2. The existing order-invariant topology profile selects the two effects with
   the largest level counts as CMG core dimensions. Remaining effects are the
   nuisance block.
3. Auto selects CMG before any MAP pilot when there are at least two effects,
   observations are at least three times the largest level count, the largest
   level count is at least ten times the second largest, and the nuisance block
   has at most nine non-base columns.
4. Other categorical intercept-only designs go directly to the certified auto
   LSMR path. Common AKM sizes resolve its automatic preconditioner to Schwarz
   and use `max(1e-14, requested_tolerance * 1e-2)`. They do not pay for a MAP
   pilot whose contraction cannot alter the topology decision.
5. One effect still uses exact one-pass demeaning. A CMG setup failure goes to
   direct LSMR with a warning; a CMG solve failure retains the existing
   certified CG safety behavior.
6. LSMR continuation uses the original raw-RHS certificate scale. If a finite
   LSMR result does not certify, CG continues from that candidate, the shared
   iteration budget is reduced by the attempted LSMR iterations, and setup,
   solve, iteration, preconditioner, warning, and coefficient-product
   accounting include both stages. Fallback remains a visible safety event.
7. No FE, OLS-score, reconstruction, memory, tolerance, or publication
   certificate is relaxed. Explicit solver eligibility is unchanged.
8. This is a development implementation, not qualification. ADR-0009 and
   ADR-0010 receipts remain immutable evidence for their exact historical
   sources. This decision supersedes ADR-0010 items 2 and 6 and ADR-0009 items
   4 and 5 only for categorical intercept-only OLS routing.

## Development evidence

- The initial matrix recorded 1,650 runs in 550 solver cells. It localized the
  cliff to post-pilot LSMR certification scaling and showed that the 0.008 MAP
  contraction boundary did not separate solver winners.
- The independent 90-case holdout recorded 1,080 runs. The adopted AKM guard
  chose CMG in 69 cases and LSMR in 21; median and 90th-percentile regret were
  zero against the fastest certified explicit arm. The five large misses were
  deliberately excluded 2:1 or 5:1 designs.
- The post-change matrix recorded 312 runs across 26 cases. All 78 auto runs
  certified with zero errors, fallbacks, and MAP pilot sweeps. The sparse
  10,000--30,000 cliff timings were 0.016, 0.018, 0.020, 0.023, 0.026, 0.030,
  0.033, 0.036, and 0.037 seconds despite the intended CMG-to-LSMR transition.
- The complete Rust library suite passed 637 tests. The focused licensed Stata
  route fixture passed the CMG/LSMR, nuisance, panel-depth, and absorb-order
  boundaries.

The run-level data and summaries are under
`benchmarks/reports/development/solver-routing-calibration-v1/`,
`benchmarks/reports/development/akm-routing-validation-v1/`, and
`benchmarks/reports/development/solver-routing-auto-akm-v2/`.

## Consequences

- Common worker--firm and worker--firm--year designs avoid both an uninformative
  pilot and normal LSMR-to-CG fallback.
- The rule remains intentionally conservative outside the AKM cardinality
  region. Eight-period 2:1 and 5:1 holdouts favored CMG, while a five-period
  5:1 holdout made CMG materially slower. Those shapes need a separate
  topology diagnostic before any wider CMG rule.
- Formal release, platform, frozen-matrix, and licensing qualification remain
  open and are not implied by this development evidence.
