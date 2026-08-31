# ADR 0019: additive PPML transaction and fereg regression boundary

**Status:** Accepted
**Date:** 2026-08-31
**Decision owner:** Johannes Schmieder

## Context

Public `fereg` is a live estimator with a versioned OLS request/result
transaction and accepted performance evidence. `feppml` is currently a
fail-closed `r(498)` stub. Developing PPML inside the existing OLS action or
replacing the OLS wire with a generic estimator wire would make an unfinished
estimator capable of changing `fereg` parsing, transport, allocation, or
publication behavior.

The safe staging boundary is an additive PPML action with an independent
request/result contract. It can be exercised end to end while the public
command still fails before a substantive plugin call. Activation then becomes
a small Stata-control-plane change after all numerical and transaction code is
already qualified.

The architectural extraction also needs a direct internal baseline. External
comparators cannot detect a slowdown or semantic drift shared by both the
candidate and comparator environment, and historical receipts from older OLS
sources do not bind a new candidate.

## Decision

### Additive protocol

1. PPML estimation uses a distinct `estimate-ppml` action and independently
   versioned PPML request and result wires. Their magic, dimensions, role
   codes, tables, receipt, validator, and serializer are not aliases of the OLS
   result wire.
2. Existing `estimate-ols` action semantics, OLS request/result layout, model
   interpretation, field order, validation, and completion receipt remain
   unchanged. A PPML protocol addition cannot repurpose a reserved OLS field
   or make an older OLS caller infer new defaults.
3. Shared model metadata may be referenced or embedded by the PPML request,
   but PPML-only outcome policy, offset/exposure binding, separation sequence,
   convergence policy, PPML VCE/DoF policy, saved outputs, and diagnostics live
   in the PPML contract. Unknown versions, roles, flags, or newer minors fail
   before observation reads.
4. The generic C bridge continues to adapt the Stata plugin ABI to Rust. It
   gains no PPML scientific logic, allocation plan, data matrix, or estimator
   branch beyond forwarding the action arguments.

### One guarded PPML estimation call

5. Stata remains the control plane. The hidden and eventual public PPML
   adapter parses syntax and metadata, constructs the candidate marker,
   allocates private shadows, invokes exactly one substantive plugin action,
   validates the complete private result, and publishes in a final protected
   block.
6. Rust remains the data and computation plane. It validates the complete
   request, identities, dimensions, nonaliasing output destinations, initially
   incomplete receipt, memory/thread options, and source bindings before
   scientific computation. SPI observation access stays on Stata's main
   thread; workers use Rust-owned state only.
7. The one native action owns sample construction, separation, topology
   rebasing, IRLS, inference, certificates, requested prediction/saved-effect
   preparation, and complete result validation. Ado or Mata does not
   materialize factor expansions, FE identifiers, the IRLS loop, or a large
   design matrix.
8. Rust computes and certifies the complete result in owned memory before the
   first store. Every scalar, N/K/K-squared table, stripe/status position,
   original-row output mapping, and destination extent is revalidated before
   stores begin.
9. The PPML `COMPLETE` receipt cell is the final native operation. No
   allocation, polling, display, computation, or SPI store follows it. An
   incomplete or mismatched receipt makes every private shadow logically
   invalid.

### Stata transaction and postestimation

10. The ado transaction preserves prior `e()`, named/null estimates, data,
    sort order, and requested output names until the complete receipt and all
    identities validate. Requested variables are privately named and receive
    final public names only in the protected commit block.
11. Syntax/domain error, unsupported option, protocol mismatch, allocation
    failure, interruption, nonconvergence, failed certificate, injected store
    failure, or caught panic restores prior public state and exposes no partial
    sample, result, or requested variable.
12. Panic containment remains at the Rust FFI boundary; no unwind crosses C.
    Break polling uses the Stata thread and propagates the registered
    interruption return code.
13. Public `feppml` postestimation uses the posted coefficient/status
    metadata, final sample, offset binding, and explicitly saved absorbed
    contribution required by ADR 0016. It does not rerun estimation or make a
    second substantive plugin call.

### Hidden qualification before activation

14. The PPML action is first reachable only through a developer adapter and
    private integration fixtures excluded from release archives. Public
    `feppml` continues to return `r(498)` before any plugin call during this
    stage.
15. Hidden end-to-end qualification covers malformed versions and dimensions,
    aliasing destinations, source-role disagreement, output conflicts,
    allocation/store failure at every phase, panic containment, Break,
    nonconvergence, rank/certificate failures, and receipt-last validation on
    macOS and Linux licensed paths plus all supported build/ABI targets.
16. Public activation is one separately revertible change that replaces the
    stub with the already-qualified adapter and updates PPML help, package,
    installation, and postestimation tests. That change contains no shared
    numerical refactor, solver-policy change, memory-threshold change, OLS
    wire change, or `fereg` syntax change.

### Frozen fereg baseline

17. Before accepting a shared-engine checkpoint, an exact tracked source is
    sealed as `F0` with its Git commit/tree, `Cargo.lock`, references, package
    files, action/model/OLS-result versions, platform plugin hashes, datasets,
    manifests, toolchains, licensed Stata identities, scientific receipts,
    allocation/RSS evidence, and raw timings. The current candidate is
    `2dee5d8c4cd99b51bbaca600348f66f325b58ab4`; it becomes authoritative only
    when the dedicated identity receipt closes the required evidence.
18. F0 and a candidate are staged as complete, version-matched, isolated Stata
    bundles. Merely swapping two plugins under the candidate ado tree is not a
    valid protocol or behavior comparison.

### Required fereg identity

19. Shared-engine extraction leaves these `fereg` fields unchanged:

    - syntax, defaults, option errors, utility modes, help examples, and
      package contents;
    - candidate sample, missing/domain rules, weights, singleton cascade,
      clusters, factor/time-series materialization, and observation order;
    - coefficient stripes, status and kept order, absorbed rank, nesting,
      DoF, cluster counts, and VCE corrections;
    - coefficients, covariance, fit statistics, fitted values, residuals,
      predictions, total/individual saved effects, and every posted `e()`
      name/type/dimension;
    - requested/used solver and memory modes, auto routing, fallback,
      tolerances, iterations, threads, timers, and certificates;
    - interruption, allocation preflight, scratch cleanup, panic containment,
      rollback, and receipt-last behavior; and
    - OLS action/model/result wires and fixed-input deterministic ordering.

20. Pure extraction commits are canonical-hex identical on deterministic
    fixtures. A separately approved numerical improvement may use only the
    existing registered OLS tolerances and must carry its own ADR and evidence;
    PPML work is not authority to widen a tolerance.

### Direct performance and memory gate

21. Scientific comparison passes before any paired timing is admissible. Each
    short local cell uses at least nine alternating warm F0/candidate pairs in
    fresh isolated processes after registered cold/stabilizing calls. For pair
    `i`, analyze

    ```text
    d_i = log(candidate_time_i / F0_time_i).
    ```

22. The equal-cell geometric-mean candidate/F0 ratio must be no greater than
    `1.02`. A cell above `1.05`, or whose paired 95 percent interval excludes
    parity in the slower direction, receives an independent same-node
    nine-pair confirmation. A confirmed ratio above `1.05` or confirmed
    interval excluding parity blocks the checkpoint.
23. Performance-sensitive checkpoints are confirmed with exact F0 and
    candidate artifacts in the same SCC job/node. Timing or RSS ratios are
    never pooled across machines. Existing external `fereg` versus Greg and
    pinned `reghdfe` gates remain independently binding.
24. An unchanged OLS route gains no candidate-only O(N), O(NK), FE-level, or
    cluster-level allocation and constructs no PPML session or workspace.
    Modeled fast/low/selected bytes, constrained-memory behavior, scratch
    bounds, and leak freedom remain unchanged.
25. Matched candidate peak RSS may not exceed
    `F0 + max(0.02 * F0, 64 MiB)`. Structural allocation tests and matched
    scaling cells remain authoritative where RSS could hide offsetting
    allocations.

### Final gate

26. Before public activation, rerun the complete `fereg` scientific,
    behavioral, failure-atomicity, memory, performance, packaging, and platform
    matrix against F0 on the final pre-activation source. Earlier extraction
    receipts cannot be inferred forward.
27. Activation is blocked by any unresolved F0 regression, PPML scientific or
    transaction failure, identity mismatch, missing exact-source receipt, or
    platform waiver not already allowed by the release policy.

## Consequences

- PPML can be tested through the real SPI and publication transaction without
  exposing an unfinished public estimator.
- An OLS wire or public-behavior regression cannot be justified as necessary
  protocol generalization.
- The final public switch is small enough to audit and revert independently of
  the numerical implementation.
- Exact internal comparisons protect `fereg` even when external comparator
  results happen to move in the same direction.
- Until the hidden and final gates close, documentation must describe PPML as
  a target architecture and the installed public command as fail-closed.
