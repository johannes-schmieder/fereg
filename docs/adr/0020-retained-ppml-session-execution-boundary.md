# ADR 0020: retained PPML session execution boundary

**Status:** Accepted
**Date:** 2026-09-01
**Decision owner:** Johannes Schmieder

## Context

ADRs 0015, 0016, and 0018 require PPML to reuse the certified weighted-linear
machinery without imposing PPML storage, rank, or routing policy on one-shot
OLS. Implementing that boundary exposed several choices that those ADRs did
not settle at sufficient precision.

Calling the complete OLS fit for PPML's fresh and strict solves would retain
OLS's width-dependent QR policy, construct a compact rank-selected design
beside the projected candidate design, and invite a duplicate profiling pass
before inference. Selecting `memory(fast|low)` only after canonical numeric
compaction would also permit an over-limit request to allocate the very
observation-sized state that preflight is meant to prevent. Finally, a strict
rank decision at apparent convergence must be able to replace the current
basis and resume IRLS without leaving the previously accepted state
unrecoverable.

This ADR fixes the private PPML retained-session execution boundary. It
refines the implementation of ADR 0015's reusable session, ADR 0016's strict
publication checks, and ADR 0018's memory plan. It does not activate the
public `feppml` command or change the accepted `fereg` surface.

## Decision

### Generation-zero admission and frozen envelope

1. PPML first builds borrowed source views and model-sized role metadata.
   After `if`/`in` membership is known, one allocation-free authoritative row
   classifier determines the original-row count, candidate count, canonical
   active count, and positive-weighted-outcome feasibility. Classification and
   later materialization use the same row-policy function and error precedence;
   candidate-excluded values cannot create scientific domain errors.
2. `candidate_observations` is the fixed number of rows admitted by `if`/`in`
   before numeric, missing-value, weight, offset, or exposure exclusions. It is
   distinct from both original N and canonical active N, and

   ```text
   active N <= candidate N <= original N.
   ```

   Candidate capacity charges preparation staging and the candidate-capacity
   retained-row map. The raw probability-weight vector contains only the
   generation-zero canonical active rows: missing or invalid candidate rows
   have no reusable raw weight to retain. Both capacities remain fixed when
   separation or later canonical rebasing removes rows.
3. The generation-zero envelope selects `Fast` or `Low` exactly once, before
   the preparation allocation phase opens and before any observation-sized
   canonical compaction. An over-limit request therefore fails without a
   partially prepared model, result shadow, or receipt.
4. Materialization reruns the canonical row-policy seam and must exactly match
   the classified original, candidate, and active shape. Final prepared
   dimensions bind to the already selected plan; they do not recompute a
   memory estimate or reselect a route.
5. Final binding may reduce observations, loaded numeric columns, generated
   finite columns, coefficient/report slots, FE dimensions and levels,
   cluster subset groups, and publication cells. Original N, candidate N,
   retained generation-zero active pweight capacity, cluster arity, separation-workspace
   policy, requested output policy, saved-FE policy, thread policy, and
   allocation-audit policy must remain identical. Growth or a change to a
   stable policy requires a new preflight rather than an in-session reroute.

### One complete retained projected design

6. After the final separation rebase, one PPML linear session owns one physical
   projected representation of every generated finite candidate that can
   receive a rank status. Syntax-only/base omissions remain metadata; the
   structurally normalized absorbed intercept does not create a duplicate
   stored direction. Stored generated-column identities, active basis
   positions, raw and within weighted norms, generation tags, and
   certificates travel with the representation.
7. A response generation projects only the changing response. A positive
   weight-generation change reprojects and restandardizes the complete retained
   design in place under the new Hessian weights, then independently
   recertifies it. A sample or topology change invalidates the session. No warm
   certificate can make stale weighted products authoritative.
8. Fast mode forms one combined response-plus-all-columns projection with
   logical shape `N x (K + 1)`. It removes the response column by compacting
   the projected finite columns forward in the same allocation and retains
   that allocation as the session design. It never creates a packed
   `N x rank` design beside it. Memory telemetry continues to charge the
   retained `N x (K + 1)` capacity even after logical compaction.
9. Low mode projects bounded column blocks directly into one row-major
   `N x K` scratch artifact. That artifact is the sole retained projected
   design for frozen solves and inference; there is no column-oriented scratch
   followed by a second transposed scratch, no resident `N x K` copy, and no
   post-fit reprofiling pass. Its resident projection block is bounded by
   `N x poolsize`.
10. Frozen finite moments stream selected active positions from the complete
    retained representation. Fast mode needs no retained moment partials. Low
    mode may retain at most `min(effective_threads, K) x K` row workspace.
    Fresh, frozen, and strict solves reuse seven checked `K x K` workspace
    banks and explicitly charged N-, K-, and metadata-sized buffers. During a
    strict transaction, one already accepted/result square remains live beside
    those workspaces, so the registered strict peak is eight simultaneous
    `K x K` banks. Inference buffers are charged separately but consume the
    same retained design. A strict checkpoint never makes a second `N x K`
    representation live.
11. Inference exposes only the committed active-position view. Physical
    compaction, if useful after the final basis commits, must be in place and
    preserve the charged resident capacity or the identity of the single
    scratch artifact. No compaction or selected-column copy may precede the
    rank transaction's commit.

### PPML all-width hybrid rank policy

12. OLS and PPML share statically dispatched moment, factor, solve, inverse,
    and certificate primitives, but the estimator adapters own rank policy.
    Existing one-shot OLS keeps its accepted fixed-block and wide-QR routes,
    thresholds, arithmetic order, fallbacks, statuses, and allocation profile.
    PPML does not call the OLS canonical rank policy.
13. PPML fresh and strict solves apply one policy at every finite width,
    including `K >= 65`. After structural and fixed-effect-collinearity
    statuses are assigned, they standardize every remaining retained column
    by its raw reference-weight norm and construct the complete Gram/RHS. The
    relative column-norm threshold is

    ```text
    rank_tolerance_multiplier * machine_epsilon * max(N, K, 1),
    ```

    where K is the complete generated finite width before numerical-rank
    drops. A guarded Cholesky attempt is only a fast path for a clearly stable,
    complete full-rank system: every pivot must exceed the strong guard based
    on `max_j G_jj * sqrt(relative_threshold)`, the solve and inverse must
    certify, and all K columns must remain selected. A partial Cholesky basis
    is never authoritative. Any guard miss or deficiency transfers to the
    observation-space route. After a complete factor is accepted, a nonfinite
    solve/inverse or failed independent certificate fails closed.
14. The observation-space route streams canonical rows of weighted,
    standardized `[X | y]` through stable Givens rotations into one bounded
    `K x K` upper factor and transformed RHS. It then applies deterministic
    column-pivoted Householder QR to that factor, recomputing every trailing
    norm rather than downdating it. Pivot norms are tied when

    ```text
    abs(norm_a - norm_b) <= 64 * machine_epsilon * max(norm_a, norm_b),
    ```

    and a tie prefers the smaller canonical generated-column identity. Let
    `initial_norm` be the largest initial standardized column norm and let
    `rho` be a trailing norm divided by `initial_norm`. A rank decision is
    ambiguous when

    ```text
    rho in relative_threshold * (1 +/- 64 * machine_epsilon).
    ```

    The factor's largest dropped trailing norm must pass that upper bound, but
    it is not the independent certificate. The solver forms
    `A = R11^-1 R12`, rescans the actual retained Fast or Low observation rows,
    and stably measures every weighted standardized residual
    `X_dropped - X_kept A`. That direct dropped-span certificate must also be
    below the upper bound and outside the ambiguity band. Coefficients and
    inverse/bread come from the accepted leading R, not from normal equations;
    the independently accumulated Gram/RHS certify normal equations and both
    inverse sides. The condition proxy is the ratio of the largest to smallest
    accepted R diagonal. K=0 has rank zero, maximum dropped norm zero, and
    condition proxy one. Ambiguity, nonfinite arithmetic, invalid identities,
    a failed direct span/score/inverse certificate, or a noninvertible leading
    factor fails closed. A clear dropped direction receives
    numerical-rank-deficiency status, and selected outputs are returned in
    canonical generated-column order.
15. The first successful fresh solve freezes the complete coefficient-slot
    status and active-position identity for ordinary IRLS iterations. Frozen
    iterations standardize the active view, use the same guarded complete
    Cholesky fast path, and fall back to retained-row QR whenever the guard or
    a solve/inverse check is weak. A fallback must recover the complete frozen
    rank; it cannot make a new omission decision from changing numerical
    noise. Both routes report the factor-diagonal condition proxy.

    FE iteration counts, stop labels, and roundoff-floor residual magnitudes
    describe work actually performed by each storage route. In particular,
    Low one-shot OLS may perform one real cleanup sweep after reversing
    destructive Householder storage, while the retained row stream can already
    certify at iteration zero. Equivalence requires identical coefficient
    statuses, rank, coefficients, bread, final residual science, and passing
    independent FE/score/inverse certificates under the registered tolerances;
    it does not fabricate identical work diagnostics. Retained sessions drop
    diagnostic-only warning strings before persistence so speculative strict
    overlap has an exact finite memory bound. One-shot `fereg` diagnostics are
    unchanged.

### Strict basis transaction

16. Apparent convergence authorizes a speculative strict all-width rank
    decision. Strict FE projection and linear certification use ADR 0016's
    strict inner/publication accuracy, while rank selection uses exactly the
    same registered threshold as every fresh and final-Hessian rank decision
    in paragraph 13. Before mutating basis authority, the session revalidates
    the prior active view under the current weight frame and prepares a
    concrete current-weight certificate for the prior active basis. Projection,
    candidate/prior solves, thread-team acquisition, and every reserve or copy
    needed to construct rollback state complete before prior authority moves.
    The session then installs the candidate under an RAII transaction guard;
    any remaining fallible validation is protected by Drop restoration. The
    guard owns the speculative candidate and the already charged prior state;
    it cannot deep-copy or newly allocate an observation-sized design. Guard
    rollback is allocation-free and infallible.
17. A strict candidate is complete: it includes its new basis/status map,
    coefficients, projected response decomposition, predictor state, bread,
    and certificates. A clear certified rank or pivot-basis change is not a
    numerical error. At the first apparent-convergence checkpoint the guard
    remains live while the registered joint eta/coefficient line search
    evaluates the candidate. An accepted candidate is committed and used as
    the next IRLS state, as required by ADR 0016; an explicit rejection rolls
    back. Any error, early return, interruption, or unwind drops the guard and
    automatically restores the prior state. An ambiguous, nonfinite,
    noninvertible, or otherwise uncertified rank decision fails closed before
    returning an actionable candidate.
18. The caller never owns detached rollback state. Candidate access
    borrows the RAII guard; commit or explicit rollback consumes it, and Drop
    supplies the default rollback. Commit advances basis generation and makes
    candidate active positions authoritative. Rollback uses only moves and
    swaps to discard candidate products and restore the prevalidated prior
    basis, generation, response workspace, predictors, fit, and frozen step
    without rebuilding or copying an observation-by-parameter design.
19. The subsequent final-Hessian synchronization has a different role: it
    reruns the complete all-width policy on the full retained design under
    exactly the final `q * mu` weights, then compares the complete kept,
    dropped, coefficient-status, and active-position identity. Merely checking
    that the committed active submatrix remains full rank is insufficient. A
    basis or rank change there blocks publication rather than silently changing
    statuses. Coefficient, eta, objective, score, bread, projection, and
    reconstruction checks remain governed by ADR 0016.

### Thread-team reuse

20. Preflight freezes one resolved thread count for the fit. Every PPML
    retained-session projection, moment-layout derivation, moment reduction,
    finite solve, predictor reconstruction, and final sandwich reduction
    executes under a `ThreadTeam` installed at that count. Moment-layout
    derivation and moment fill share one install scope because layout reads the
    active Rayon worker count. Sandwich work may use a separate install scope,
    but not a different team policy or ambient pool size.
21. `ThreadTeam` reuses one bounded process-local Rayon pool while the resolved
    count is unchanged and replaces it when a later action requests a
    different count. Repeated session phases therefore do not construct new
    registries or worker sets. The cache retains no estimator, sample, design,
    coefficient, or certificate state and gives no authority to reuse
    scientific products across fits.
22. Correctness cannot depend on Rayon's global pool. Qualification installs
    a deliberately different ambient/global pool and observes the exact
    installed Rayon pool width (`rayon::current_num_threads()`) inside retained
    projection/moment work and PPML sandwich work. The number of workers Rayon
    happened to schedule is diagnostic telemetry, not a correctness gate. SPI
    access and Break polling remain on Stata's main thread; workers see only
    Rust-owned data. Registered deterministic partition and merge orders
    remain binding.

## Consequences

- PPML pays for one complete projected finite design because strict rank
  recertification and final inference need every candidate direction, but it
  does not pay for a second compact design or duplicate final profiling pass.
- Fast and low are storage representations of the same scientific session and
  use the same hybrid rank/status policy at every K.
- A strict basis change can safely resume IRLS, while a rejected candidate can
  restore the last accepted state without an allocation or a second design.
- Improvements to shared static numerical primitives can carry to both
  estimators. Estimator-specific rank, routing, DoF, convergence, and
  publication policies remain separate.
- The generation-zero envelope makes memory refusal earlier and more honest;
  later sample shrinkage cannot opportunistically select a different route.
- Public `feppml` remains fail-closed until ADR 0019's exact-source activation
  gate closes.

## Acceptance

The retained-session boundary is production-eligible only when tests prove:

- classification/materialization identity and domain-error precedence, plus a
  constrained request that fails before any preparation-phase allocation or
  store;
- exact candidate-capacity accounting, allowed final-dimension shrinkage,
  rejection of growth or stable-policy drift, and no route reselection;
- Fast pointer/capacity preservation from the combined projection, one Low
  scratch identity, no packed `N x rank` copy, no duplicate projection,
  modeled seven-square workspace ownership, and the exact eight-square strict
  overlap plus N/K/slot metadata;
- Fast/Low equivalence for K=0, K=1, K>=65, well-conditioned, exactly
  deficient, signed linear-combination, extreme-scale, and weak-but-full-rank
  designs; deterministic canonical identity under row/column permutations;
  guarded-Cholesky fallback to retained-row QR; and both sides of the
  near-threshold ambiguity band failing closed;
- R-based solve/bread plus independent Gram normal-equation/inverse and direct
  observation-row dropped-span certificates, including malformed/nonfinite
  rejection and frozen QR fallback;
- truthful route-specific FE work diagnostics, zero-capacity persistent
  warning banks, and unchanged one-shot `fereg` diagnostics;
- first-checkpoint certified basis-change continuation, final-Hessian
  basis-change rejection, and error/interruption/unwind injection proving that
  RAII rollback restores exact pointers, capacities, generation, and a usable
  frozen solve without allocation;
- exact installed Rayon pool widths in retained and sandwich kernels despite a
  conflicting ambient pool, together with same-route deterministic evidence;
  and
- exact `fereg` scientific, route, allocation, protocol, and paired F0
  non-regression gates required by ADR 0019.
