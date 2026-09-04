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
  certificates are scientific authority. It still owns the first full-width
  weighted-linear frame and active-basis freeze; on eligible large explicit
  LSMR/CMG fits, current-weight LSMR may compute that initial projection plus
  strict and final-Hessian projections before the canonical session
  independently certifies them. MAP owns Mu separation work and remains the
  checkpoint fallback.
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
  ADR-0025's MAP-computation requirement. Explicit LSMR/CMG fits with two or
  three categorical effects and at least 16,384 observations use an ephemeral
  full-Schwarz current-weight batch LSMR checkpoint projection for the initial
  response-plus-all-controls frame and later strict/final frames. Initial
  inputs come from the untouched prepared frame; later inputs are recomputed
  from the accepted PPML state. A candidate is accepted only after canonical
  FE projection and normal equations, all-width rank/status, bread, score,
  objective, reconstruction, and exact final `q_i * mu_i` certificates pass.
  Numerical rejection visibly falls back to MAP on untouched inputs; other
  errors remain fatal. The serial checkpoints share one exactly admitted
  full-width transient peak while the ordinary engine remains live.
- The joint LSMR engine fuses target, square-root-weight, right-hand-side, and
  finite-diagonal preparation; uses a measured narrow/wide adjoint policy;
  coarsens FE work ownership; and parallelizes independent certificate
  directions with canonical reductions. Its topology, buffers, preconditioner,
  predictor state, and warm solution remain fit-owned and reusable.
- Each certified ordinary joint-LSMR frame now retains its scheduled,
  first-pass, and effective recurrence tolerances; split iteration counts;
  recurrence and exact finite/FE certificate maxima; continuation cause; and
  maximum log Hessian-weight change. On eligible three-FE core-pair fits, only
  valid nearby certified evidence may tighten the next first pass. The same
  evidence proactively refreshes a long or stale retained two-way core at
  current weights, avoiding a recurrence that can only request a rebuild.
  Checkpoint commits, active-basis replacement, invalid transitions, and warm
  reset invalidate the predictor. Exact post-recurrence certificates remain
  authoritative.
- For three or more effects, joint LSMR retains one two-effect Schwarz core and
  couples it to a fresh current-weight positive diagonal action for the
  remaining FE coordinates through the symmetric block factor
  `T diag(C,D) T'`. The current-weight core/extra cross action is matrix-free
  over finalized memberships, so no additional pair graph is retained. Two
  core-width bridge buffers are fallibly preallocated and charged to the memory
  plan. The action is symmetric positive semidefinite; exact post-recurrence
  finite and per-level normal equations remain the authority.
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
- Commit `b545b2f` preserves the preceding green local optimization round. The
  current follow-on tree passes the universal arm64/x86_64 macOS build, binary
  audit, and actual-plugin ABI/panic-containment self-test; Rust formatting and
  workspace Clippy; 703 core unit tests plus the constrained
  memory, adversarial wire, roundtrip, and numerical-oracle suites; and 52
  focused development-harness parser, protocol, transaction, and adjudication
  tests. This is development evidence, not exact-SHA qualification.
- The current v3 local science run under
  `/private/tmp/fereg-ppml-next-final2-science` passed all eight topology scopes
  against both the independent explicit-dummy oracle and pinned comparator.
  The one-repetition local smoke under
  `/private/tmp/fereg-ppml-next-final2-smoke` passed all science and route checks
  for all nine eligible candidate arms.
- After the checkpoint change, the v3 local science profile under
  `/private/tmp/fereg-ppml-checkpoint-science-gated` again passed all eight
  topology scopes against the independent explicit-dummy oracle and pinned
  comparator with no route failure. The one-repetition local smoke under
  `/private/tmp/fereg-ppml-checkpoint-smoke` passed all science and route checks
  for all nine eligible MAP/LSMR/CMG arms.
- The current joint-LSMR predictor candidate passes the v3 local science gate
  under `/private/tmp/fereg-joint-predictor-v3-science-final` and the
  three-repetition local smoke under
  `/private/tmp/fereg-joint-predictor-v3-smoke-final`; both report
  `science_pass=true` and `route_pass=true`. Its universal arm64/x86_64 macOS
  plugin passes binary audit and the actual-plugin ABI/panic-containment test.
  Focused joint-LSMR, prepared-IRLS, and constrained-memory tests, Rust
  formatting, and strict workspace Clippy also pass.
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
- ADR-0027's independently certified current-weight LSMR checkpoint removed
  the former strict/final floor. The next matched five-run N=50,000, K=10
  sparse-3FE round compares `b545b2f` with the current implementation on the
  same data and seven threads. Median total time fell from 4.255 to 2.763
  seconds. Initial MAP solve time fell from 2.820 to 1.498 seconds at the same
  410 iterations; ordinary joint LSMR fell from 1.001 to 0.862 seconds and
  from 1,601 to 1,101 iterations. It used seven frames and five tighter
  retries rather than four. The computed checkpoint used 40 iterations,
  about 0.030 seconds setup, 0.099 seconds solve, and 0.009 seconds independent
  certification; complete strict/final time was 0.240 seconds. FE-projection
  phase time fell from 3.840 to 2.382 seconds. Pinned `ppmlhdfe` has a
  66.228-second median on the same data, so the current local diagnostic is
  about 24.0x faster. Every public certificate passed with no fallback.
- Extending that same certified batch projection to the initial full-width
  frame clears the remaining checkpoint floor. Against an exact clean
  `26ae660` five-run baseline on the identical N=50,000, K=10 sparse-3FE data
  and seven threads, median wall time fell from 2.776 to 1.201 seconds (56.7%)
  and native total time from 2.757 to 1.180 seconds. The prior initial MAP row
  used 444 iterations and 1.691 seconds of solve time. It is now absent: all
  three checkpoint projections are on the LSMR row and together use 51
  iterations, 0.047 seconds setup, 0.131 seconds solve, and 0.014 seconds
  independent certification. FE-projection phase time fell from 2.441 to
  0.866 seconds. Ordinary joint LSMR changed from 896 iterations/four retries
  and 0.727 seconds solve to 949 iterations/four retries and 0.787 seconds;
  the material total improvement therefore comes from the intended initial
  checkpoint boundary rather than a favorable ordinary-frame fluctuation.
  Likelihood and deviance differ from the baseline by at most 3.0e-9 and
  6.0e-9, coefficients by 3.9e-14, covariance cells by 1.8e-18, and every
  public certificate passes with no fallback. The admitted selected peak is
  unchanged at 148,353,390 bytes, including the same 75,940,704-byte solver
  workspace. Relative to the existing 66.228-second pinned-reference median,
  this local diagnostic is about 55.1x faster.
- An instrumented reproduction of exact-`0bd1ced` policy on the same
  N=50,000, K=10 sparse-3FE case showed
  that its four ordinary-LSMR retries were preconditioner-staleness
  continuations, not certificate-driven tolerance retries: first passes used
  200 iterations and the rebuilt continuations used 749, while every effective
  tolerance remained at its scheduled value. Tightening every first pass by
  10x or 100x increased total iterations and was removed. With the retained
  certified-frame predictor, median seven-thread wall/native time over matched
  local repeats fell from 1.180/1.160 to 1.057/1.036 seconds. Ordinary solve
  time fell from 0.769 to 0.647 seconds (15.8%); setup changed from 0.0119 to
  0.0129 seconds and certification from 0.00635 to 0.00630 seconds. The five
  frames used 944 first-pass and zero continuation iterations, versus 949
  total iterations and four continuations. Likelihood and deviance were
  identical at reported precision; maximum coefficient, covariance-cell, and
  public-certificate differences were `9.1e-17`, `1.8e-19`, and `1.1e-10`,
  with unchanged rank and no fallback. The 128-record diagnostic allowance
  raises the modeled peak by 4,096 bytes and does not change memory routing.
- Guard diagnostics preserve the two-FE path exactly at 61 ordinary iterations
  and one retry. Balanced 3FE K=10 at one/seven threads and K=50 at seven
  threads retained identical iterations and scientific results, with wall
  times changing by at most 0.006 seconds. Sparse-3FE K=0 at seven threads
  changed from 10.655 to 10.374 seconds and from 1,270 to 1,268 iterations;
  likelihood/deviance moved by `1.47e-7`/`2.93e-7` (about `6.1e-12` relative),
  all independent certificates passed, and rank and fallback remained
  unchanged. These timings are diagnostic, not qualification evidence.
- The current eight-scope local science profile and three-scope licensed smoke
  both pass their science and route adjudicators with no failures. In the
  one-thread smoke, balanced-2FE MAP/LSMR/CMG took 0.164/0.164/0.158 seconds,
  difficult-chain 2FE took 13.439/0.195/0.941 seconds, and balanced-3FE took
  0.208/0.206/0.219 seconds. Every specialized fit retained its requested
  effective engine with no fallback. Focused seven-thread Stata checks also
  passed complete certificates at K=0 (balanced-2FE LSMR/CMG 0.099/0.094
  seconds) and K=50 (balanced-2FE 0.400/0.501 and balanced-3FE 0.438/0.604
  seconds). These remain diagnostic local timings, not qualification or
  auto-routing evidence.
- In the current v3 one-thread difficult-chain smoke, MAP/LSMR/CMG took
  12.885/1.624/2.348 seconds versus 71.725 seconds for pinned `ppmlhdfe`.
  LSMR and CMG are therefore about 44.2x and 30.5x faster in this diagnostic.
  A separate five-run seven-thread LSMR repeat had a 1.027-second median: the
  initial MAP solve used 2,004 iterations and 0.641 seconds, strict/final used
  0.034 seconds, and six ordinary joint-LSMR frames used 61 iterations, one
  retry, 0.004 seconds setup, 0.026 seconds solve, and 0.005 seconds
  certification.
- On the difficult N=25,000 two-FE chain, certificate-aware CMG continuation
  reduced RHS retries from 66 to 11, CMG iterations from 382 to 237, CMG solve
  time from 1.192 to 0.696 seconds, and total time from 6.042 to 5.419 seconds.
  MAP and LSMR behavior on that scope was unchanged within local timing noise.
- In the current one-repetition comparator smoke, candidate/reference ratios
  for balanced 2FE, difficult-chain 2FE, and balanced 3FE respectively were
  0.357/0.180/0.379 for MAP, 0.361/0.0226/0.369 for LSMR, and
  0.334/0.0327/0.392 for CMG. The three-scope geometric means were about 0.290,
  0.145, and 0.162. These are diagnostic local timings, not a replacement for
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
- Current-plugin checks under `/private/tmp/fereg-final-shapes2` exercised all
  three families on three-FE K=0 and difficult two-FE K=50 data. All six fits
  converged to identical within-shape objectives with complete public
  certificates and expected ordinary-engine identities. The K=50 LSMR and CMG
  fits each visibly used one certified checkpoint fallback; neither changed
  its effective ordinary engine.

## Completed, evidence-backed baseline

Accepted historical OLS, PPML, solver, memory, and platform evidence is
preserved under `benchmarks/reports/` and in the accepted ADRs. It applies only
to the exact sources and artifacts named in those records. The current public
solver candidate has the green local worktree evidence listed above; it does
not inherit release or cross-platform qualification from earlier sources.

## Current blockers

- Sparse-three-FE ordinary joint LSMR remains the specialized-fit floor. The
  matched promoted run now uses about 0.65 seconds of solve time and 944
  iterations out of a 1.06-second total, with no continuation. Its scheduled
  tolerances already predict the independent certificates; the remaining work
  is conditioning and operator cost rather than retry policy.
- Explicit MAP, and therefore `solver(auto)`, remains weak on poorly connected
  graphs. The difficult-chain MAP smoke needs 20,842 total inner iterations
  and about 13 seconds even after its per-pass kernels were accelerated.
- Prepared CMG still performs 11 certificate-driven RHS retries on the hard
  chain. Executor construction remains below the threshold for upstream API
  work.

## Development plan

1. Improve the three-FE core-pair preconditioner or operator locality so the
   944 certified first-pass iterations fall materially, without restoring the
   slow retained all-pairs Schwarz policy. The new frame evidence should
   distinguish conditioning gains from retry-count changes.
2. Reduce CMG's remaining hard-chain RHS retries through certificate-aware
   continuation or recycled block work, without changing the publication
   tolerance or cadence.
3. Improve explicit MAP on poorly connected graphs with a topology-aware
   coarse correction or gauge-reduced preconditioner while preserving its
   canonical projection certificates and `solver(auto)` behavior.
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
