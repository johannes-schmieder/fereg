# ADR 0015: shared prepared weighted-linear boundary

**Status:** Accepted
**Date:** 2026-08-31
**Decision owner:** Johannes Schmieder

## Context

`fereg` now has a mature prepared OLS path with compact fixed-effect topology,
certified weighted projection, guarded finite solving, low-memory execution,
deterministic reductions, and independently checked inference. The stale PPML
prototype predates most of that work. It rebuilds weighted design state during
IRLS and duplicates finite WLS and covariance machinery that the OLS path has
since replaced.

Calling the complete OLS estimator once per IRLS iteration would reuse code at
the wrong level. It would repeatedly apply OLS sample, rank, DoF, reporting,
and result policies, retain products PPML does not need, and make changing
weights an accidental edge case. A callback-oriented generic GLM framework
would also put abstraction and dispatch into the dominant observation loops.

The useful common boundary is lower: an estimator supplies an explicit
response and positive solver-weight frame to the already prepared model, then
consumes a certified weighted-linear result. The estimator continues to own
the scientific meaning of those inputs and the result assembled from them.

## Decision

### Prepared ownership

1. A prepared-model view is the shared owner or borrower of immutable
   observation identity, numeric and categorical sources, regressor recipes,
   compact fixed-effect topology, cluster topology, thread policy, and memory
   policy.
2. A stable sample ledger records original-row identity, inclusion, and one
   authoritative drop reason. A sample change is applied transactionally to
   every row-scoped source; no estimator may subset outcome, regressors,
   weights, fixed effects, clusters, offsets, or output mappings separately.
3. Estimator adapters own candidate-domain rules, singleton and separation
   ordering, and the decision to rebase the prepared model. Shared preparation
   does not silently apply OLS policy to PPML or vice versa.
4. The existing prepared OLS view remains the compatibility seam. Extraction
   proceeds through narrow views and wrappers rather than a wholesale storage
   rewrite or a new crate.

### Explicit linear frame

5. A weighted-linear request supplies borrowed, already validated numerical
   inputs equivalent to:

   - the current response;
   - strictly positive solver weights;
   - the estimator's bread/reference scale;
   - route, tolerance, memory, retained-product, and certificate policies; and
   - generation identifiers for sample, topology, weights, response, and
     kept-column basis.

   It does not call estimator code from a row loop.
6. The certified kernel owns the common work: weighted FE projection, design
   standardization, row-major moment construction, guarded Cholesky and
   rank-revealing fallback, fitted decomposition, bread inputs, and direct
   projection, score, rank, reconstruction, and bread certificates requested
   by the caller.
7. The returned linear fit distinguishes canonical regressor status, kept
   order, weighted within-design state, finite coefficients, FE contribution,
   residual state, solver reports, and certified retained products. It does
   not compute estimator-level DoF, VCE corrections, likelihood statistics,
   public residual definitions, or `e()` names.
8. The implementation uses concrete or statically dispatched views. No trait
   object, virtual call, closure callback, or per-observation branch is added
   to an existing OLS hot loop merely to support PPML.

### One-shot and reusable operation

9. `fereg` uses the one-shot operation exactly once. Its response, weights,
   scales, route policy, certificates, and OLS result assembly remain the
   existing ones.
10. PPML may place the same concrete kernel inside a reusable linear session.
    The session retains immutable prepared state and reusable response,
    weight, transformed-design, moment, solver, and certificate workspaces.
    It never changes the scientific request implicitly.
11. Session state is generation checked:

    - a sample or topology change invalidates every row-, FE-, cluster-, rank-,
      score-, and warm-solver product;
    - a weight change invalidates weighted projection, standardization,
      moments, rank validation, bread, and route state;
    - a response change invalidates response projection, fitted values,
      residuals, and score state but may retain a certified weighted design;
    - a kept-basis change invalidates rank-dependent inference and saved-effect
      state.

12. A previous residualized design may be used as the starting point under a
    new positive weight frame because its removed component remains in the
    unchanged FE span. The refreshed projection must pass a direct certificate
    under the new weights. Failure falls back to the canonical prepared
    design; it never becomes a warning-only approximation.
13. The final publication solve is fresh or strictified at the estimator's
    publication tolerance. A warm optimization cannot be the only evidence
    for coefficients, rank, bread, FE orthogonality, or reconstruction.

### Shared inference reduction

14. Sandwich reductions share only their expensive mechanism: a certified
    within design, bread, cluster topology, and an explicit scalar score
    multiplier per observation. Production and independent certificate
    accumulators retain the separate, contiguous, deterministic reductions of
    ADR 0014.
15. OLS supplies its existing residual/weight score and bread inputs. PPML
    supplies its final Poisson score and Hessian inputs. Each estimator applies
    its own DoF, small-sample correction, reported test distribution, and
    covariance policy outside the common reduction.

### OLS zero-cost boundary

16. The OLS path constructs no PPML driver, iteration record, separation
    workspace, changing-weight session, mean vector, line-search buffer, or
    PPML result object. No new data-sized allocation is permitted on an
    unchanged `fereg` route.
17. Pure extraction checkpoints must be bit-identical on deterministic OLS
    fixtures and retain all existing allocation, solver-route, certificate,
    failure-atomicity, and wire behavior. Any later intentional OLS numerical
    change requires a separate ADR and qualification; it cannot be bundled
    with PPML reuse.
18. The common kernel is internal to `fereg-core`. It is not a public plugin
    action, a public Rust stability promise, or authority to broaden either
    command's accepted syntax.

## Consequences

- Future improvements to prepared topology, projection, finite moments,
  rank-solving, threading, memory planning, and sandwich reductions can
  benefit both estimators when their explicit inputs satisfy the same
  mathematical contract.
- Estimator policy remains visible at adapters and drivers rather than hidden
  behind a generic solver interface.
- PPML can reuse allocation-heavy state across IRLS iterations without making
  one-shot OLS pay for session support.
- The stale PPML WLS and VCE implementations are migration evidence and test
  material, not parallel production engines. They are removed once the shared
  path is authoritative and covered.
- A proposed reuse boundary is rejected if OLS and PPML require conflicting
  sample, rank, weight, DoF, or correction semantics, or if a zero-cost
  concrete OLS representation cannot be demonstrated.

## Acceptance

The boundary is accepted in production only when the complete `fereg`
scientific suite remains exact-green, allocation tests prove that PPML session
state is unreachable from OLS, and the direct `fereg`-versus-F0 gates in ADR
0019 pass. PPML use additionally requires fresh-versus-warm equivalence,
generation-invalidation, certificate-fallback, fast/low-memory, and
deterministic-thread tests.
