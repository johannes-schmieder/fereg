# fereg 0.5.0 beta status

**Last updated:** 2026-09-04

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

- The canonical retained linear session, rank policy, and independent
  certificates are scientific authority. MAP still owns the first
  weighted-linear frame, active-basis freeze, Mu separation work, and the
  checkpoint fallback. On eligible large explicit LSMR/CMG fits, current-weight
  LSMR may compute strict and final-Hessian projections before the canonical
  session independently certifies them.
- `solver(lsmr)` is a public family backed by one reusable joint categorical
  engine after the active basis freezes. With no active finite controls it
  retains the public `lsmr_fused` concrete identity; with active controls it
  reports `lsmr_joint`. Both routes use fit-lifetime warm state, bounded retry,
  exact normal-equation certificates, and the requested thread team. They
  never silently fall back to MAP.
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
  surface. Saved effects are recovered once from the accepted canonical
  final-Hessian state.
- `e(feppml_solver)` reports fixed rows for MAP, fused LSMR, joint LSMR, and
  prepared CMG. Requested, selected, concrete, effective, checkpoint, and
  fallback identities are also published as named eclass macros/scalars.
  Failed CMG attempt work is retained before fallback.

## Performance implementation

- The MAP ordinary-frame path uses a PPML-specific certified frozen solve and
  reconstructs the candidate predictor from response minus within residual,
  avoiding redundant observation-by-control predictor work.
- Multi-FE MAP projection fuses wider right-hand-side blocks and uses
  deterministic level-owned parallel accumulation from 16,384 retained rows.
  Difficult two-FE PCG likewise applies the Schur operator in deterministic
  level-owned chunks and fuses its accepted solution/residual update.
- Strict checkpoints project the retained design and response together. A
  final-Hessian checkpoint reuses that already-aligned retained design only
  when every raw weight and scale field is bitwise unchanged; it still absorbs
  the final response and performs the solve, predictor reconstruction, bread,
  and full numerical certification.
- [ADR 0027](docs/adr/0027-canonical-ppml-checkpoint-projection.md) supersedes
  only ADR-0025's MAP-computation requirement. Explicit LSMR/CMG fits with two
  or three categorical effects and at least 16,384 observations use an
  ephemeral full-Schwarz current-weight batch LSMR checkpoint projection. Its
  inputs are recomputed from the accepted PPML state and remain untouched; the
  candidate is accepted only after the canonical FE projection and normal
  equations, rank, bread, score, objective, reconstruction, and exact final
  `q_i * mu_i` certificates pass. Numerical rejection visibly falls back to
  MAP on those untouched inputs; other errors remain fatal. The memory plan
  charges the ordinary engine and checkpoint workspace together.
- The joint LSMR engine fuses target, square-root-weight, right-hand-side, and
  finite-diagonal preparation; uses a measured narrow/wide adjoint policy;
  coarsens FE work ownership; and parallelizes independent certificate
  directions with canonical reductions. Its topology, buffers, preconditioner,
  predictor state, and warm solution remain fit-owned and reusable.
- For three or more effects, joint LSMR retains one two-effect Schwarz core and
  applies a fresh current-weight positive diagonal action to all remaining FE
  coordinates. This replaces the poorly conditioned diagonal-only route
  without restoring the rejected all-pairs Schwarz policy. The memory preflight
  charges exactly that retained topology plus the full diagonal and recurrence
  state; exact post-recurrence finite and per-level normal equations remain the
  authority.
- Prepared CMG binds exact topology, preflights before observation-scale
  preparation, reuses fereg-owned thread and numeric buffers, and uses direct
  row-to-nuisance lookup for three-FE Schur work. The generic OLS CMG builder,
  owned-result path, CG polish/fallback, inference reuse, and auto routing are
  unchanged.
- Prepared CMG carries forward an independently observed insufficient
  recurrence scale to the next changing-weight frame. Only the failed columns
  are retried on the current frame, and the publication certificate tolerance
  is unchanged.
- The pinned upstream CMG API still owns `ParallelExecutor` by value, so each
  changing-weight frame constructs a fresh executor and preconditioner. Local
  ownership profiling put executor construction at only 0.01--0.40% of total
  elapsed time and hierarchy construction below the five-percent promotion
  threshold. No upstream CMG API expansion is currently justified.

## Active checkpoint: public PPML solver families

The exact four-name PPML interface and explicit MAP, LSMR, and prepared-CMG
engines are implemented. `solver(auto)` deliberately remains MAP-only. The
ADR-0026 harness transition and the current local optimization round are
implemented; no SCC rerun, auto-routing calibration, or release qualification
has been launched.

## Development harness and validation

- The live PPML solver development harness is manifest v3, result table 2.6.
  It uses only public MAP, LSMR, and CMG arms plus the pinned default
  `ppmlhdfe` comparator. The reference remains on its observed
  CG/symmetric-Kaczmarz route; the harness does not force `ppmlhdfe` LSMR.
  Historical reports are preserved; superseded live manifests and obsolete
  generated manual fereg/feppml benchmark analyses are removed.
- The v3 harness has local science and smoke gates, then a bounded SCC lane of
  16 parallel tasks on comparable 28-core Broadwell nodes. It records the
  complete public 4-by-22 solver matrix for every successful fereg arm, so
  requested, selected, concrete, effective, checkpoint, retry, fallback,
  timing, iteration, and certificate fields fail closed if malformed.
- The exact product commit `2ff872b` passed the universal arm64/x86_64 macOS
  build, licensed Stata quick profile, local science and smoke profiles, strict
  formatting and Clippy, 691 core tests, 74 normal plugin tests, and 89 FFI
  plugin tests. The final development-harness patch passes all 758 repository
  tests plus the portable shell, Python, TOML, C, and mock-SPI checks.
- The current working tree derived from `5f9d0f7` passes the universal
  arm64/x86_64 macOS build and actual-plugin ABI/self-test; Rust 1.85.1
  formatting and Clippy; 699 core tests, the complete workspace oracle suite,
  and 89 FFI plugin tests; all 758 repository tests; and the portable shell,
  Python, TOML, C, mock-SPI, allocation-source, parser, protocol, and
  transaction checks. This is development evidence, not exact-SHA
  qualification.
- The final v3 local science run under
  `/private/tmp/fereg-ppml-final-science` passed all eight topology scopes
  against both the independent explicit-dummy oracle and pinned comparator.
  The one-repetition local smoke under `/private/tmp/fereg-ppml-final-smoke`
  passed all science and route checks for all nine eligible candidate arms.
- After the checkpoint change, the v3 local science profile under
  `/private/tmp/fereg-ppml-checkpoint-science-gated` again passed all eight
  topology scopes against the independent explicit-dummy oracle and pinned
  comparator with no route failure. The one-repetition local smoke under
  `/private/tmp/fereg-ppml-checkpoint-smoke` passed all science and route checks
  for all nine eligible MAP/LSMR/CMG arms.
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
- [ADR 0026](docs/adr/0026-ppml-comparator-objective-adjudication.md) is encoded
  in manifest v3, its parser, adjudicator, and focused negative tests. Pinned `ppmlhdfe`
  `e(ll)` and `e(deviance)` are the authoritative reference objective, while
  prediction replay, KKT, and model-space projection remain diagnostics. A
  predicted state is an additional objective witness only when independently
  certified feasible and stationary; a projected state alone is never an
  objective witness. The reported objective pair must be finite, valid, and
  Poisson-identity consistent. Fereg still requires full public-state replay,
  feasibility, stationarity, and solver certification. Only the previously
  accepted narrow coefficient stopping adjudication can resolve a mismatch;
  all other scientific mismatches fail closed. The failed table-2.4 run above
  remains failed evidence under its original harness and is not reclassified.

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
- In the failed-run sparse high-cardinality three-FE shards, ordinary
  projection dominated MAP and joint LSMR exposed the then-MAP-owned
  strict/final checkpoint as its dominant floor. These shards remain
  diagnostic, not a performance claim or qualification receipt.
- A local matched three-scope smoke, with the exact starting commit
  `5f9d0f77494fc2d58894a832093ab67e91dff05f` built in a detached worktree,
  found candidate/baseline geometric-mean ratios of 0.909 for MAP, 0.861 for
  LSMR, and 0.894 for CMG. Every arm passed the v3 science and route checks.
  The scopes were balanced two-FE, difficult two-FE chain, and balanced
  three-FE at N=25,000, K=10, one thread; this is local diagnostic evidence,
  not a frozen performance claim.
- Focused kernels explain those gains. Wide 3FE MAP projection at N=65,536 was
  61--63% faster; an eight-sweep plus certificate study at N=25,000--50,000
  was 54--79% faster at 7 or 14 threads after lowering the parallel threshold
  to 16,384 rows. A 25,000-row difficult 2FE Schur operator was 47% faster and
  reduced its full instrumented solve by 5.7%, with unchanged iterations.
  Joint-LSMR frame time at N=200,000 improved by 15% at K=10 and 82% at K=50.
- A final one-repetition N=50,000, K=10 sparse-3FE diagnostic at seven threads
  converged with fresh public certificates: MAP took 73.646 seconds and 11,972
  inner iterations; LSMR took 53.292 seconds. The LSMR ordinary engine used
  18.007 seconds, while its required MAP checkpoint used 27.190 seconds and
  4,846 iterations. This confirms that iteration count and the certified MAP
  checkpoint, not LSMR setup, remain the principal floor.
- Against the current-worktree sparse-3FE baseline at the same N=50,000, K=10,
  seed, and seven threads, the two-effect-core LSMR route reduced total time
  from 51.610 to 36.455 seconds. Ordinary LSMR fell from 41,433 to 1,480
  iterations and from 16.549 to 0.857 seconds; setup rose from 0.0015 to
  0.0105 seconds, certification fell from 0.0125 to 0.0063 seconds, and retries
  fell from five to four. The selected modeled requirement rose from 81.8 MB
  to 96.8 MB.
  All public certificates passed with no fallback. The unchanged three MAP
  checkpoints still consumed 4,858 iterations and 27.790 seconds; the complete
  strict/final phase was 32.514 seconds, about 89% of reported elapsed time.
- A candidate-only checkpoint-seed experiment preserved all MAP certificates
  but changed the difficult-chain checkpoint count by only two iterations
  (6,381 to 6,379) and changed none of the 4,846 sparse-3FE checkpoint
  iterations. That path was removed rather than retained as unproductive
  compatibility state. Strict candidates still project design and response
  together, use RAII rollback, and reset specialized warm state only after the
  certified candidate commits.
- ADR-0027's independently certified current-weight LSMR checkpoint removes
  that floor. On the current plugin, the three-run N=50,000, K=10 sparse-3FE
  median is 4.729 seconds versus the preceding 36.455-second worktree
  baseline. Strict/final time fell from 32.514 to 0.264 seconds; the computed
  checkpoint uses 41 LSMR iterations, about 0.032 seconds setup, 0.104 seconds
  solve, and 0.009 seconds independent certification. The first MAP frame now
  dominates at 410 iterations and about 3.19 seconds; seven ordinary LSMR
  frames use 1,601 iterations, four retries, about 0.013 seconds setup, 1.11
  seconds solve, and 0.009 seconds certification. Pinned `ppmlhdfe` has a
  66.228-second median on the same data, making this local diagnostic about
  14.0x faster. The 148.2 MB modeled peak covers the observed 102.3--111.1 MB
  peak delta.
- In the current v3 difficult-chain smoke, MAP/LSMR/CMG took
  13.187/1.602/2.302 seconds versus 73.765 seconds for pinned `ppmlhdfe`.
  LSMR and CMG are therefore about 46.0x and 32.0x faster in this diagnostic.
  Their strict/final phase is about 0.049 seconds; the one computed checkpoint
  projection needs one LSMR iteration, while the initial MAP frame still needs
  2,004 iterations and about 1.18 seconds. CMG ordinary work still has 11
  certificate-driven retries; LSMR has one.
- On the difficult N=25,000 two-FE chain, certificate-aware CMG continuation
  reduced RHS retries from 66 to 11, CMG iterations from 382 to 237, CMG solve
  time from 1.192 to 0.696 seconds, and total time from 6.042 to 5.419 seconds.
  MAP and LSMR behavior on that scope was unchanged within local timing noise.
- In the final one-repetition comparator smoke, candidate/reference ratios for
  balanced 2FE, difficult-chain 2FE, and balanced 3FE respectively were
  0.359/0.184/0.376 for MAP, 0.314/0.070/0.372 for LSMR, and
  0.303/0.080/0.427 for CMG. The three-scope geometric means were 0.292,
  0.201, and 0.218. These are diagnostic local timings, not a replacement for
  the failed SCC evidence or an auto-routing calibration.
- A five-repetition matched N=50,000, K=0 two-FE check reproduced the starting
  commit's likelihood, deviance, rank, and absorbed DoF exactly. Candidate
  median ratios were 0.708 for MAP, 0.771 for LSMR, and 0.788 for CMG, while
  the requested LSMR route retained its public `lsmr_fused` identity.
- Focused current-plugin K=0 chain, K=0 sparse-3FE, and K=50 balanced 2FE/3FE
  checks all converged with complete public certificates, expected concrete
  routes, and no fallback. Their diagnostic record is under
  `/private/tmp/fereg-ppml-final-shapes`; it intentionally contains no
  comparator adjudication.
- A post-ADR-0027 N=25,000 balanced-2FE check under
  `/private/tmp/fereg-ppml-checkpoint-shapes` again exercised MAP, LSMR, and
  CMG at K=0 and K=50. Every arm converged to the same objective with the
  expected concrete route and no fallback.

## Completed, evidence-backed baseline

Accepted historical OLS, PPML, solver, memory, and platform evidence is
preserved under `benchmarks/reports/` and in the accepted ADRs. It applies only
to the exact sources and artifacts named in those records. The current public
solver candidate has the green local worktree evidence listed above; it does
not inherit release or cross-platform qualification from earlier sources.

## Current blockers

- The required initial MAP frame is now the dominant specialized-fit cost:
  about 3.19 of 4.71 native seconds in the sparse-3FE diagnostic and 1.18 of
  1.59 seconds on the difficult chain. This frame still owns active-basis
  freeze and is intentionally outside ADR-0027.
- Sparse-three-FE ordinary joint LSMR is the next floor at about 1.11 seconds,
  1,601 iterations, and four tighter retries. Its two-effect core plus diagonal
  extra-effect action is much better than the former diagonal-only policy but
  leaves conditioning and certificate-target prediction work.
- Explicit MAP, and therefore `solver(auto)`, remains weak on poorly connected
  graphs. The difficult-chain MAP smoke needs 20,842 total inner iterations
  and 13.187 seconds even after its per-pass kernels were accelerated.
- Prepared CMG still performs 11 certificate-driven RHS retries on the hard
  chain. Executor construction remains below the threshold for upstream API
  work.

## Development plan

1. Profile and improve the initial MAP solve on poorly connected two- and
   three-FE graphs without changing its basis-freeze authority. A materially
   different initial-frame algorithm would require another explicit ADR;
   kernel, ordering, gauge, or certified preconditioner improvements do not.
2. Improve the three-FE joint-LSMR extra-effect correction and make its first
   recurrence tolerance predict the independent certificate more accurately.
   Preserve the exact post-recurrence normal equations and do not restore the
   rejected retained all-pairs Schwarz policy.
3. Reduce CMG's remaining hard-chain RHS retries through certificate-aware
   continuation or recycled block work, without changing the publication
   tolerance or cadence.
4. Reconsider an upstream CMG shared-executor API only if a future profile puts
   executor construction above the five-percent promotion threshold. A minimal
   candidate would accept an `Arc<ParallelExecutor>` while still rebuilding
   changing-weight numeric graphs and hierarchies.
5. Keep `solver(auto)` MAP-only. Never rewrite or promote the failed table-2.4
   campaign. Any future SCC run
   requires separate authorization and a new exact-source receipt root.

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
