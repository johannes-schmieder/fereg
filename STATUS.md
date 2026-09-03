# fereg 0.5.0 beta status

**Last updated:** 2026-09-02

**Branch:** `main`

**Mode:** prototype/development

**Release state:** public OLS and PPML testing surfaces are active; no release
tag or source-qualified release has been published

This is the sole mutable progress and resume record. Historical benchmark and
qualification evidence remains immutable under `benchmarks/reports/`; accepted
architecture decisions remain under `docs/adr/`.

## Current product state

- `fereg` remains the public OLS command and retains its existing solver,
  inference, result-wire, and auto-routing contracts.
- `feppml` is a public eclass command implemented as a one-call Stata control
  plane over the Rust computation plane. It preserves the estimation sample,
  transaction atomicity, interruption handling, separation ledger, inference,
  prediction, and saved-fixed-effect contracts already accepted for PPML.
- Public PPML solver syntax is exactly
  `solver(auto|map|lsmr|cmg)`. Omission means `auto`.
- `solver(auto)` remains conservatively MAP-only. No LSMR or CMG auto region is
  enabled until the dedicated calibration campaign is accepted.
- PPML request/result/action wires are 1.5/1.2/1.5. Retired private solver
  spellings and request fields are not aliases and fail closed.

## Public PPML solver behavior

- MAP is the canonical engine. It owns the first weighted-linear frame, active
  basis freeze, Mu separation work, strict checkpoints, final-Hessian
  certification, rank, inference, and saved-FE recovery.
- `solver(lsmr)` is a public family with an internal concrete engine. The fused
  FWL engine is selected after the active basis freezes with no active finite
  controls; the joint incremental engine is selected when active controls
  remain. Both use fit-lifetime warm state, bounded retry, exact normal-equation
  certificates, and the requested thread team. They never silently fall back
  to MAP.
- `solver(cmg)` means only the prepared fused-FWL CMG engine. It prepares the
  categorical topology once, assembles the changing-weight numeric system per
  ordinary frame, selectively retries failed projections, and certifies the
  combined finite/FE system. Only typed numerical nonconvergence falls back to
  same-frame MAP; the fallback is permanent and visible. Input, topology,
  memory, interruption, and internal failures remain fatal.
- LSMR accepts at least two categorical intercept-only FE dimensions. CMG
  accepts exactly two or three; three-FE nuisance width is capped at 64.
  Absorbed slopes and unsupported FE counts return `r(498)`.
- LSMR and CMG require the fast memory route. `memory(auto)` may select fast;
  explicit `memory(low)` and allocation auditing return `r(498)`. A supported
  route that does not fit its checked memory plan returns `r(909)`.
- Weights, offsets/exposure, separation, robust or clustered inference,
  prediction outputs, and saved fixed effects retain the ordinary PPML public
  surface. Saved effects are recovered once from the final MAP-owned Hessian.
- `e(feppml_solver)` reports fixed rows for MAP, fused LSMR, joint LSMR, and
  prepared CMG. Requested, selected, concrete, effective, checkpoint, and
  fallback identities are also published as named eclass macros/scalars.
  Failed CMG attempt work is retained before fallback.

## Performance implementation

- The MAP ordinary-frame path uses a PPML-specific certified frozen solve and
  reconstructs the candidate predictor from response minus within residual,
  avoiding redundant observation-by-control predictor work.
- The canonical MAP refresh is lazy on specialized ordinary frames and remains
  mandatory at fallback, strict, and final checkpoints.
- LSMR certificates use overflow-safe norms, bind exact categorical topology,
  reuse warm state, skip redundant diagonal/preconditioner work, and execute
  inside the configured retained thread team.
- Prepared CMG binds exact topology, preflights before observation-scale
  preparation, reuses fereg-owned thread and numeric buffers, and uses direct
  row-to-nuisance lookup for three-FE Schur work. The generic OLS CMG builder,
  owned-result path, CG polish/fallback, inference reuse, and auto routing are
  unchanged.
- The pinned upstream CMG API owns `ParallelExecutor` by value. Consequently a
  changing-weight CMG frame must still construct a fresh upstream executor.
  Removing that cost requires an upstream shared-executor or recovery API; it
  is the only known implementation-level performance blocker.

## Active checkpoint: public PPML solver families

The exact four-name PPML interface and explicit MAP, LSMR, and prepared-CMG
engines are implemented. `solver(auto)` deliberately remains MAP-only. The
current task is validation and diagnostic performance measurement, not auto
routing calibration or release qualification.

## Development harness and validation

- The live PPML solver development harness is manifest v2. It uses only public
  MAP, LSMR, and CMG arms plus pinned default and forced-LSMR `ppmlhdfe`
  comparators. Historical reports and qualification manifests are preserved;
  the obsolete live v1 solver manifest was removed by explicit approval.
- The v2 harness has local science and smoke gates, then a bounded SCC lane of
  16 parallel tasks on comparable 28-core Broadwell nodes. It records the
  effective public family and public solver fallback count.
- The complete local repository entry point is green on the implementation
  worktree: repository checks, 744 Python tests, portable/static and mock-SPI
  checks, strict formatting and Clippy, 677 core tests, integration oracles,
  74 normal plugin tests, and 89 FFI plugin tests.
- A universal macOS build previously passed during integration. The exact
  committed candidate still needs a fresh universal build and licensed Stata
  quick run before the development benchmark campaign.

## Completed, evidence-backed baseline

Accepted historical OLS, PPML, solver, memory, and platform evidence is
preserved under `benchmarks/reports/` and in the accepted ADRs. It applies only
to the exact sources and artifacts named in those records. The current public
solver candidate has the green local worktree evidence listed above; it does
not inherit release or cross-platform qualification from earlier sources.

## Current blockers

- The exact committed candidate has not yet completed its licensed Stata quick
  gate or the approved v2 local/SCC solver campaign.
- The upstream CMG executor ownership noted above prevents complete
  cross-frame executor reuse without an upstream API change.

## Development plan

1. Commit the coherent public-solver candidate on `main`.
2. Build the exact commit with `./scripts/build_macos.sh` and run
   `./ci/run_stata_ci.sh quick`.
3. Run the v2 local science and smoke profiles against the exact baseline and
   pinned reference closure.
4. If both local gates pass, run the approved 16-task, 28-core SCC development
   campaign and inspect every scheduler/application/science result.
5. Fix measured solver defects if any. Keep `solver(auto)` MAP-only; auto
   calibration is the next separate campaign.

## 0.5.0 beta exit gate

The beta remains open. A release candidate must bind one exact source and
artifact to the required public scientific, transaction, memory, platform,
and performance evidence. Default `solver(auto)` must be calibrated and then
pass its frozen representative comparator matrix with matching sample,
coefficients, inference, degrees of freedom, requested outputs, and visible
route/fallback evidence. Windows and the formal cross-platform aggregate remain
deferred; no historical or development receipt is promoted across source
changes.

No broad qualification, cross-platform aggregate, tag, release publication,
or push is authorized by this work.
