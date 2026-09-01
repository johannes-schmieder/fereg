# ADR 0010: topology-aware CMG automatic routing

**Status:** Accepted
**Date:** 2026-08-27
**Decision owner:** Johannes Schmieder
**Superseded in part by:** ADR 0021 for the development auto selector and setup fallback

## Context

Explicit CMG can materially outperform direct Schwarz-LSMR on long,
high-cardinality AKM panels because one graph hierarchy is reused for every
outcome and regressor right-hand side. It is not uniformly faster. Once setup
is charged, two-period panels can be more than an order of magnitude slower;
near-balanced graph partitions and nuisance Schur columns can also erase or
reverse the gain. Selecting CMG from a solver failure or from MAP contraction
would pay avoidable pilot work and would not identify these setup-dominated
topologies.

Automatic routing must therefore be deterministic, effectively free on
rejected cases, independently certified, memory-safe, and observably
reversible when CMG setup or solve fails.

## Decision

1. For categorical intercept-only designs, `solver(auto)` computes a small
   shape profile from already prepared metadata before the four-sweep MAP
   pilot. The profile sorts effects by level count and performs no additional
   observation-scale scan.
2. Version 1 selects CMG only when all of the following hold:
   - there are at least two absorbed intercept effects and no absorbed slope;
   - the additional effects contribute zero non-base nuisance columns;
   - observations are at least eight times the highest core level count; and
   - the highest core level count is at least ten times the second core level
     count.
3. Selected cases skip the MAP pilot. The report retains requested solver
   `auto`, records used solver `cmg` (code 7), and records zero pilot
   iterations. The rule is identical in fast and low execution.
4. Explicit `solver(cmg)` remains broader and continues to support up to 64
   nuisance non-base columns. Automatic rejection is a performance decision,
   not a scientific or API limitation.
5. One hierarchy and workspace pool live for the complete prepared fit and are
   reused across outcome and regressor blocks. The final OLS residual is
   normally already within-transformed; it goes directly to the independent
   certificate instead of launching PCG on a near-zero graph right-hand side.
   The normal certified CMG path runs only if that check fails. Graph PCG uses
   one-hundredth of the public FE tolerance so the downstream OLS score
   certificate does not require a separate post-QR correction.
6. CMG setup failure, including memory preflight failure, returns to the
   existing MAP-pilot auto route with a deterministic warning. A CMG solve
   failure certifies with CG and permanently marks the session for CG on later
   calls. No partially updated matrix is published.
7. The upstream CMG dependency is pinned to
   `ce126358a82cc1a461cfe7b9f9318b4c3c1c1316`. Its additive public memory API
   provides checked conservative build/retained/workspace estimates and exact
   retained reports without changing existing solver entry points. fereg adds
   its graph-vector, nuisance, and retained-coefficient storage to the joint
   execution/solver budget and verifies that CG fallback also fits.
8. The permanent matrix and analyzer are
   `benchmarks/manifests/ols_cmg_auto_calibration_v1.json` and
   `scripts/cmg_auto_calibration.py`. Setup is included. A selected cell may
   not be more than five percent slower than explicit Schwarz-LSMR, and the
   selected aggregate must be faster. Saved-output overhead is capped at the
   larger of 0.05 seconds or five percent.

## Calibration evidence

The 100,000-row crossover and guardrail runs covered periods 2, 3, 5, 10, and
20; mobility from 0.02 to 0.50; one and eight threads; narrow and 11-regressor
right-hand sides; unit and heterogeneous analytic weights; 5:1, 20:1, and
100:1 core cardinality ratios; and 0, 8, 32, and 64 nuisance columns.

- Two-period low-mobility CMG was up to 13.1 times slower than LSMR.
- Three- and five-period cases were mixed or too close for a stable route.
- At ten periods, 32 and 64 nuisance columns made CMG 2.58 to 4.98 times
  slower; eight columns was not a stable eight-thread win.
- At a 5:1 core ratio, ten-period CMG ranged from roughly parity to 17 percent
  slower in guardrail cells.
- Every calibrated unit/weighted, narrow/wide, one/eight-thread guardrail cell
  admitted by the frozen rule favored CMG.

The concise receipt is `benchmarks/expected/cmg_auto_route_v1.json`. It is a
route-freeze receipt, not a substitute for the remaining 1m/10m,
cross-platform, packaging, or license qualification.

## Consequences

- Favorable AKM topologies avoid both the MAP pilot and a second hierarchy
  build while retaining the unchanged independent FE and OLS certificates.
- Borderline, nuisance-heavy, slope-bearing, and nearly balanced designs keep
  the already qualified ADR-0009 route and pay only a few metadata operations.
- The selector is intentionally conservative. Expanding it requires a new
  frozen calibration receipt and must pass the same per-cell and aggregate
  slowdown gates.
- fereg is MIT while the current CMG dependency is GPL-3.0-only. Source and
  binary distribution compatibility must be resolved before a public release;
  this ADR does not waive that release blocker.
