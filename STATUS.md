# fereg 0.5.0 beta status

**Last updated:** 2026-09-03

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

- The live PPML solver development harness is manifest v2, result table 2.5. It
  uses only public MAP, LSMR, and CMG arms plus the pinned default `ppmlhdfe`
  comparator. The reference remains on its observed CG/symmetric-Kaczmarz
  route; the harness does not force `ppmlhdfe` LSMR.
  Historical reports and qualification manifests are preserved;
  the obsolete live v1 solver manifest was removed by explicit approval.
- The v2 harness has local science and smoke gates, then a bounded SCC lane of
  16 parallel tasks on comparable 28-core Broadwell nodes. It records the
  effective public family and public solver fallback count.
- The exact product commit `2ff872b` passed the universal arm64/x86_64 macOS
  build, licensed Stata quick profile, local science and smoke profiles, strict
  formatting and Clippy, 691 core tests, 74 normal plugin tests, and 89 FFI
  plugin tests. The final development-harness patch passes all 758 repository
  tests plus the portable shell, Python, TOML, C, and mock-SPI checks.
- The SCC development run rooted at
  `/projectnb/welfgr/fereg/dev/20260903T170000Z-public-ppml-solvers-2ff872b`
  used result table 2.4 and the v1 public-state KKT audit. It completed all
  scheduler work; fifteen of sixteen task receipts passed. Task 8 failed that
  scientific gate and the collector correctly refused to create a merged
  success receipt. This run is retained as failed development evidence, not
  rewritten or promoted.
- Subsequent result-table 2.5 tooling at `d237cc1` and focused reanalysis
  localized the task-8 discrepancy to the reference public prediction. The
  default and strict `ppmlhdfe` predicted log-means could not be certified
  within the declared control-plus-FE model space at the registered tolerance.
  The independent matrix-free scaled-CGLS audit classified those states as
  indeterminate; it did not infer infeasibility from a small normal residual
  or use an uncertified predicted objective as an oracle. This later diagnosis
  did not reclassify the result-table 2.4 campaign.
- [ADR 0026](docs/adr/0026-ppml-comparator-objective-adjudication.md) now
  resolves the future comparator policy: pinned `ppmlhdfe`
  `e(ll)` and `e(deviance)` are the authoritative reference objective, while
  prediction replay, KKT, and model-space projection remain diagnostics. A
  predicted state is an additional objective witness only when independently
  certified feasible and stationary. Fereg still requires full public-state
  replay, model-space, stationarity, and solver certification. The failed run
  above remains failed evidence under its original harness; it is not
  retroactively reclassified.
- The current result-table 2.5 adjudicator still encodes the superseded
  all-reference-state fail-closed rule. Before any new evidence run, its
  manifest, parser, audit logic, and focused tests must be versioned together
  to implement ADR 0026. No new campaign is authorized by the policy decision.

## Diagnostic performance evidence

- These figures are recomputed from the per-scope solver summaries in the 15
  shard `analysis.json` files bound by task receipts under the exact SCC run
  root above; all three task-8 scopes are excluded because task 8 produced no
  receipt. Across the remaining 53 scopes, the geometric-mean fereg/reference
  runtime ratios were 0.840 for MAP and 0.551 for LSMR. LSMR was the fastest
  fereg family in 40 scopes and beat MAP in all 53. Prepared CMG's ratio was
  0.480 on its 45 eligible scopes, but LSMR was faster in 32 of those common
  scopes; hindsight CMG routing improved the aggregate over always-LSMR by only
  about one percent.
- Two-FE results were already strong: reference/fereg geometric-mean speedups
  were about 1.50x for MAP, 2.50x for LSMR, and 2.36x for CMG. Across all eight
  green sparse-three-FE scopes, MAP and LSMR speedups were 0.45x and 0.68x.
  Restricting to the six high-cardinality scopes, they fell to 0.38x and 0.60x;
  CMG was ineligible there. Its 1.05x sparse-three-FE result covers only the two
  low-cardinality eligible scopes.
- On sparse high-cardinality three-FE designs, ordinary projection dominates
  MAP. Joint LSMR removes much of that cost, exposing the mandatory MAP-owned
  strict/final checkpoint as its dominant remaining floor. The next round must
  profile those phases separately before changing assembly, projection,
  copying, warm-state, or certification work. These failed-run shards are
  diagnostic, not a performance claim or qualification receipt.

## Completed, evidence-backed baseline

Accepted historical OLS, PPML, solver, memory, and platform evidence is
preserved under `benchmarks/reports/` and in the accepted ADRs. It applies only
to the exact sources and artifacts named in those records. The current public
solver candidate has the green local worktree evidence listed above; it does
not inherit release or cross-platform qualification from earlier sources.

## Current blockers

- ADR 0026 is accepted but not yet encoded in a new development manifest and
  adjudicator version. That local transition is required before another
  evidence campaign; the existing failed run remains immutable.
- Sparse, high-cardinality three-FE projection is the dominant MAP cost; after
  LSMR accelerates ordinary frames, strict/final MAP checkpoints become its
  dominant remaining floor.
- The upstream CMG executor ownership noted above prevents complete
  cross-frame executor reuse without an upstream API change.

## Development plan

1. Encode ADR 0026 in a new manifest/adjudicator version with focused local
   parser, scientific, and negative tests. Do not rerun the SCC campaign yet.
2. Profile and optimize the explicit PPML families locally, prioritizing
   sparse three-FE MAP ordinary projection and LSMR strict/final MAP
   checkpoints. Establish duplication or avoidable work by measurement before
   changing weight assembly, projection, allocation, warm state, or
   certification.
3. Preserve full numerical, transaction, memory, and result certification.
   Keep `solver(auto)` MAP-only and defer all route calibration.
4. After the optimization round is complete, request separate authorization
   for a new small SCC evidence campaign. Never rewrite the failed run.

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
