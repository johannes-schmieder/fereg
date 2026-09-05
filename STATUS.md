# fereg 0.5.0 beta status

**Last updated:** 2026-09-05

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
  weighted-linear frame and active-basis freeze; on eligible explicit
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
  three categorical effects at every positive retained sample size use an ephemeral
  full-Schwarz current-weight batch LSMR checkpoint projection for the initial
  response-plus-all-controls frame and later strict/final frames. Initial
  inputs come from the untouched prepared frame; later inputs are recomputed
  from the accepted PPML state. A candidate is accepted only after canonical
  FE projection and normal equations, all-width rank/status, bread, score,
  objective, reconstruction, and exact final `q_i * mu_i` certificates pass.
  A failed fresh canonical certificate permits one tighter independent batch
  on untouched inputs, with unchanged canonical tolerance. A second numerical
  rejection visibly falls back to MAP; other errors remain fatal. All attempt
  work and retry counts are reported. The serial checkpoints share one admitted
  full-width transient peak while the ordinary engine remains live.
- [ADR 0028](docs/adr/0028-one-effect-ppml-initialization.md) seeds cold one-FE
  publication fits at the group-intercept Poisson optimum for zero finite
  slopes. Group-constant offsets use compensated weighted means when raw
  moments are representable; otherwise stable log-sum-exp handles the sums.
  An unusable seed restores the complete global initialization. Warm starts,
  Mu, slopes, and multi-FE fits are unchanged; rank freezing, both convergence
  checks, and every strict/final certificate remain mandatory, including K=0.
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
- The joint-LSMR Schwarz core for three or more categorical effects is now the
  two largest retained level dimensions, with original absorb position as the
  deterministic tie-breaker. The bridge gathers and scatters those possibly
  noncontiguous blocks without changing canonical FE or result order. Prepared
  CMG applies the same ranking: the largest dimension is the left graph side,
  the second largest is the right side, and any remaining eligible dimension
  is nuisance. Reversing graph orientation is algebraically equivalent, but
  fixing the larger side first makes hierarchy construction independent of
  absorb syntax order. Memory accounting charges the selected pair, while
  certificates still cover every FE block.
- Prepared CMG binds exact topology, preflights before observation-scale
  preparation, reuses fereg-owned thread and numeric buffers, and uses direct
  row-to-nuisance lookup for three-FE Schur work. The generic OLS CMG builder,
  owned-result path, CG polish/fallback, inference reuse, and auto routing are
  unchanged.
- CMG finite moments use deterministic 4,096-row partitions, capped at 64
  partial matrices as N grows. Fresh raw finite normal-equation certificates
  run across the admitted team, reusing finished K-wide finite-solve buffers
  and preserving each direction's compensated row order. The workspace plan
  uses the same partition geometry. Successful-frame diagnostics now include
  previously omitted per-RHS certification and fereg-owned preparation time.
- PPML CMG graph batches now cover the larger of the admitted response/control
  width and nuisance Schur width, bounded by the existing pool size. Narrow
  finite designs can therefore solve nuisance directions across the thread
  team. The complete early memory plan fixes the capacity used by ordinary
  engine construction and strict-basis replacement; private frame tracing
  reports that capacity. Fresh numeric construction, recurrence tolerances,
  exact certificates, and the 64-indicator envelope are unchanged.
- Ordinary PPML CMG omits a third effect from its projection recipe only when
  exact retained membership proves that it is a function of a selected core
  effect. The full model still owns admission, the nuisance cap, canonical
  rank/DoF/checkpoints, every independent FE certificate, and saved effects.
  This avoids inverting an identically zero Schur block without a numerical
  rank decision. Private trace records the original redundant-effect position
  and actual nuisance solve width. Near-nesting does not qualify.
- Joint and checkpoint LSMR preflight includes the pinned Schwarz builder's
  dense cross-tabulation dispatch, including concurrent pair tables. Sparse
  observation graphs can still take that dense construction path. This fixes
  a measured underestimate exposed by small three-FE checkpoints; it changes
  admission, not the solver or scientific tolerances.
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
September 5 local breadth and CMG nuisance-batching rounds below are implemented
and validated. The
ADR-0026 harness transition and the
exact-source FE-role-selection SCC development campaign are implemented. The
campaign is scientifically green but route-red for pre-existing matched
candidate/baseline checkpoint fallbacks; it is development evidence, not a
merged success receipt or qualification. No auto-routing calibration or
release qualification has been launched.

## Development harness and validation

- The live PPML solver development harness is manifest v4, result table 2.6.
  It uses only public MAP, LSMR, and CMG arms plus the pinned default
  `ppmlhdfe` comparator. The reference remains on its observed
  CG/symmetric-Kaczmarz route for multiple effects and observed unaccelerated
  one-FE projection; the harness does not force `ppmlhdfe` LSMR.
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
- Exact commit `5b7a7ca2f8bb960a85d11a1dd7c9a94d9f6c0ad4` implements deterministic
  largest-dimension FE roles against baseline
  `a888e9799d35fa775c41c4510a71261b1513bbe2`. Focused tests cover all six
  three-FE permutations for LSMR and CMG, noncontiguous bridge algebra, CMG
  bipartition reversal, fitted predictors and means, objective values, score
  certificates, deterministic ties, and permutation-invariant memory charges.
  The complete no-default-features workspace suite passed with 715 core tests
  plus all plugin, testkit, integration, numerical-oracle, and constrained
  memory tests. Rust formatting and strict workspace Clippy passed. The
  universal arm64/x86_64 macOS plugin passed binary audit and the actual-plugin
  ABI/panic-containment self-test. Exact-source v3 local science and smoke runs
  under `/private/tmp/fereg-role-selection-5b7a7ca-science` and
  `/private/tmp/fereg-role-selection-5b7a7ca-smoke` both passed science and
  route checks.
- The exact-source SCC v3 development run is rooted at
  `/projectnb/welfgr/fereg/dev/20260904T191500Z-role-select-5b7a7ca` with setup,
  28-slot array, and collector jobs `7445383`, `7445384`, and `7445385`.
  Thirteen tasks produced green task receipts. Tasks 10 and 12 completed their
  measurements and were scientifically green but route-red because both
  candidate and baseline used the same visible numerical checkpoint fallback
  on the easy difficult-chain and sparse-three-FE scopes. Task 8 exhausted the
  original three-hour limit during its untimed public-state KKT audit after all
  timed arms had completed; the collector correctly refused a merged receipt.
  The exact-source task-8 retry under
  `/projectnb/welfgr/fereg/dev/20260904T223000Z-role-select-task8-retry` used jobs
  `7458532`, `7458533`, and `7458534`. Task `7458533.8` ran on 28 Broadwell
  slots, exited zero, and produced a digest-matched task receipt with
  `science_pass=true`, `route_pass=true`, and twelve accepted independent KKT
  adjudications. Its collector correctly failed closed because the retry root
  intentionally contains no tasks 1--7 or 9--16. Across the sixteen immutable
  shard analyses, all 56 scopes are scientifically green; the combined route
  result remains red only for tasks 10 and 12. No synthetic merged or close
  receipt was created.
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

## Local breadth optimization: September 5

The baseline is clean `a355b5d6d714f6a43f69c08d798994dcadba3c22`, independently
built in a detached worktree. The [immutable local diagnostic summary](benchmarks/reports/development/ppml-local-breadth-2026-09-05.json)
binds each stage's dirty source snapshot, artifact, receipt digest, scope,
repetitions, scientific differences, engine timings, split recurrence counts,
effective tolerances, and measured/modelled allocation. These are local macOS
diagnostics, not qualification or SCC evidence. Earlier-stage artifacts are
identified separately; no result is relabelled as testing a later source.

| Scope | Arm / threads | Baseline s | Candidate s | ppmlhdfe s |
| --- | --- | ---: | ---: | ---: |
| Skewed 1FE, N=100,000, K=10 | MAP / 7 | 0.585 | 0.383 | 0.873 |
| Balanced 1FE, N=1m, K=0 | MAP / 1 | 1.223 | 1.019 | 1.551 |
| Balanced 1FE, N=1m, K=10 | MAP / 7 | 3.756 | 3.744 | 8.416 |
| Difficult 2FE chain, N=8,000, K=10 | LSMR / 7 | 1.234 | 0.092 | 5.703 |
| Difficult 2FE chain, N=8,000, K=10 | CMG / 7 | 1.438 | 0.290 | 5.703 |
| Sparse 3FE, N=8,000, K=10 | LSMR / 7 | 12.223 | 0.588 | 9.753 |
| Sparse 3FE, N=10,000, K=0 | LSMR / 7 | 9.823 | 0.649 | — |
| Sparse 3FE, N=10,000, K=50 | LSMR / 7 | 69.295 | 2.668 | — |
| Balanced 2FE, N=50,000, K=50 | CMG / 7 | 0.857 | 0.753 | 3.845 |
| Balanced 3FE, N=50,000, K=50 | CMG / 7 | 1.042 | 0.923 | 4.481 |

- The N=8,000 sparse candidate is the corrected-memory rerun; all scientific
  fields and iterations exactly reproduce the earlier 0.574-second candidate.
  Checkpoint work falls from 3,972 MAP sweeps and 9.585 seconds of solve time
  to 269 batch-LSMR iterations and 0.182 seconds. Ordinary LSMR uses 394 first
  plus 315 continuation iterations, 0.273 seconds solve, and 0.002 seconds
  certification. Its setup is 0.008 seconds; checkpoint setup/certification
  are 0.027/0.008 seconds. Neither route falls back.
- At sparse N=10,000, K=50, checkpoint solve time falls from 64.326 to 1.122
  seconds and iterations from 5,320 to 337. Ordinary work is essentially
  unchanged: 1,273+1,230 baseline versus 1,275+1,231 candidate first/continuation
  iterations, five retries each, 1.204 versus 1.188 seconds solve. Candidate
  ordinary setup/certification are 0.008/0.007 seconds; checkpoint
  setup/certification are 0.039/0.038 seconds. At one thread total runtime
  falls from 73.888 to 8.129 seconds, with the same iteration counts and no
  fallback. K=0 ordinary work stays exactly 808+460 iterations; its checkpoint
  count falls from 8,241 to 529. Public checkpoint counts sum each frame's
  maximum RHS recurrence count, not total column-operator applications.
- The final one-FE guard has parity at small balanced K=10 (0.058 seconds)
  and balanced N=100,000, K=50 (1.687 versus 1.688). The skewed K=10 fit drops
  from six ordinary frames to two, retaining three canonical checkpoints.
  A first million-row K=10 screen exposed initialization overhead; the
  representable constant-offset weighted-mean calculation removes it. The
  final five-repeat million-row comparison is in the table.
- Wide CMG finite work drops from about 32 to 9 ms per ordinary frame, and
  its exact combined certificate from about 17 to 5 ms, with unchanged
  iterations/retries. One-thread balanced K=0/K=50 checks remain at parity.
  The 64-column envelope means non-base third-FE indicators, independently
  of finite K; K=0/10/50 and nuisance widths 0/32/63/64 passed locally. The
  licensed boundary fixture accepts 64 and rejects 65 while preserving prior
  public coefficients and covariance. Successful-frame certificate telemetry
  now charges previously omitted work, so old/new certificate timer totals
  alone are not comparable.
- Memory is a real tradeoff: the small sparse checkpoint's native peak delta
  is 72,755,017 bytes. Its former 30,435,854-byte model was unacceptable.
  Charging the pinned dense/sparse cross-tab rule raises the model to
  85,507,414 bytes; all three corrected repetitions have allocation/model
  ratio 0.851. New tests cover both dispatch boundaries, pair concurrency,
  FE-order invariance, overflow, and preflight rejection. No tolerance or
  memory allowance was relaxed.
- Sample, rank, DoF, and result names match the baseline exactly. Across the
  difficult sparse-width checks, maximum absolute coefficient/covariance
  differences are 1.32e-11/7.76e-15; likelihood/deviance differences are
  8.13e-8/1.63e-7. Raw certificate cells can differ by 5.49e-7, while every
  independent normalized certificate passes its unchanged tolerance.
- The N=50,000, K=10, seven-thread primary remains at parity: 0.988 versus
  0.985 seconds, with exactly identical scientific fields and 850 ordinary
  iterations, zero continuation. Ordinary setup/solve/certification are
  0.013/0.581/0.006 seconds; checkpoints use 52 iterations and
  0.048/0.128/0.014 seconds. The 1% and 10% mixing cases retain respectively
  806+689 and 1,158+1,420 ordinary iterations, three/four retries, and totals
  1.491/2.472 seconds. No new ordinary-LSMR policy is promoted.
- Removed experiments: a four-sweep checkpoint MAP pilot did not fix the
  focused fallback; a direct bridge lookup changed ordinary time by only
  about 2%; a 48-direction aggregate coarse correction reduced 850 to 834
  and 2,578 to 2,462 iterations, insufficient for its added work/state. The
  coarse screen overlapped other checks and is explicitly not timing evidence.
  None of these implementation paths or tests remains dormant in production.
- Validation: the full repository run passed 765 Python tests and all static,
  portable, oracle, constrained-memory, and plugin checks. After the final
  one-FE refinement, workspace tests passed 723 core and 74 normal plugin
  tests plus integration/oracle/memory suites; 89 FFI plugin tests, Rust/fuzz
  formatting, strict Clippy, universal macOS build and binary/ABI audit, and
  licensed Stata quick all passed. Closing v3 science (eight scopes), v3
  smoke (three scopes, all nine arms), and independent one-FE science (four
  scopes) have `science_pass=true` and `route_pass=true`. The closing smoke
  is a one-repeat gate, additional to the earlier three-repeat matched smoke;
  timing comparisons above use three or five repetitions.
- Harness v4 adds directly runnable local breadth, difficulty, checkpoint,
  nuisance/control-width, one-FE, scaling, large-easy, and held-out profiles
  plus explicit scope selection. Only the named subsets above were run.
  Frozen v3 and SCC evidence remain unchanged. The new allocation source
  companion v3 reconciles reviewed earlier PPML/shared-source changes and
  this report bookkeeping; its policy, route boundaries, and F0 pins are
  unchanged, and historical v2 is preserved. No F0 qualification, SCC job,
  auto calibration, push, tag, or release was run.

## Local CMG nuisance batching: September 5

This focused follow-on starts from clean `87a46415484d05ec14f1e34812dc1090fbe71e0a`,
independently built in a detached worktree. The [immutable diagnostic summary](benchmarks/reports/development/ppml-cmg-nuisance-batches-2026-09-05.json)
binds the final product-source hashes, plugin artifacts, manifest, pinned
references, source snapshots, receipt digests, timing distributions, split
iterations/tolerances, scientific differences, and allocation measurements.
Raw evidence is retained in local temporary bundle `fereg-cmg-schur.tH1r1H`.
These are local macOS diagnostics, not SCC, auto-calibration, or qualification
evidence.

The measured bottleneck was a width mismatch: K=0 capped CMG batches at one
RHS, serializing 64 independent nuisance solves per changing-weight frame.
The retained change sizes the graph pool from both widths and carries the
admitted capacity into execution and strict-basis replacement. It uses the
existing upstream batch policy without changing the numerical solver,
Schur factor, canonical checkpoint, tolerance, or certificate. No upstream
CMG API or OLS route changed, and no new ADR was needed.

Five-repeat medians at N=25,000 and seven threads:

| Nuisance / K | Baseline CMG s | Candidate CMG s | MAP s | LSMR s | ppmlhdfe s |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 / 0 | 0.157 | 0.156 | 0.126 | 0.095 | 0.166 |
| 32 / 0 | 1.145 | 0.644 | 0.104 | 0.093 | 0.170 |
| 64 / 0 | 2.216 | 1.049 | 0.100 | 0.102 | 0.197 |
| 64 / 10 | 1.183 | 1.184 | 0.265 | 0.172 | 0.655 |
| 64 / 50 | 2.238 | 2.236 | 0.927 | 0.537 | 4.579 |

- At width 64/K=0, ordinary CMG setup falls from 2.044899 to 0.880626 seconds;
  graph solve is 0.093527/0.093197 and certification 0.002713/0.002651.
  Four ordinary frames retain 72 first-pass iterations, zero continuation,
  zero retries, and zero fallback. Their scheduled/first/effective tolerances
  agree exactly, from `1.38949549437313835e-7` to
  `2.59936151136636528e-10`. Checkpoints retain 45 iterations over three
  frames; candidate setup/solve/certification is 0.012175/0.014276/0.002188
  seconds. Public iteration units sum frame maximum-RHS recurrence counts,
  not all column-operator applications; this gain is concurrent execution,
  not reduced recurrence work.
- At width 32/K=0, setup falls from 0.983084 to 0.481408 seconds, with 62
  first-pass iterations and no continuation/retry/fallback. Wide K retains
  its old batch capacity and is at parity. An earlier few-percent wide-K
  slowdown did not reproduce on the final five-repeat artifact.
- A reversed-order, five-repeat one-thread check is also at parity:
  width 64/K=0 is 4.067/4.063 seconds and K=50 is 7.904/7.909. Balanced 3FE
  N=50,000/K=2 is 0.154/0.151 with green science/route gates. LSMR guards
  retain 38 ordinary iterations on the difficult two-FE chain (0.221/0.223
  seconds total) and 850 on sparse 3FE (0.984/0.979), both without retries.
- Memory admission deliberately covers the wider concurrent workspace pool.
  For the primary case the conservative model rises from 93,336,374 to
  157,002,582 bytes; maximum observed native peak delta rises only from
  22,355,849 to 22,464,375 bytes. The one-thread model is 80,245,294/82,003,502
  bytes. No memory allowance was relaxed. The enlarged bound is tested at
  one byte below admission, before canonical or route-owned observation work.
- All 139 matched solver repetitions have identical recorded coefficients,
  covariance cells, likelihood, deviance, and public certificates, plus exact
  sample, rank, DoF, and result names. Independent local science remains green.
- The six-scope envelope run is **science-green but route-red**: width 8/K=0
  makes both exact baseline and candidate retry one CMG RHS and permanently
  fall back to MAP. Its projection relative-infinity maximum is 0.067615 in
  both arms. The complete failed receipt is preserved; this scope is excluded
  from CMG performance promotion. Every other envelope arm is route-green.
- Validation: 38 focused CMG tests; all 726 core and 74 normal plugin tests,
  integration/oracle/constrained-memory suites, and 89 FFI plugin tests;
  formatting, strict Clippy, repository checks, 32 solver-parser tests,
  protocol/transaction tests, and 31 allocation-source checks passed. The
  universal macOS build passed binary audit and actual-plugin ABI/panic checks.
  The unchanged v3 local science/smoke profiles, run through v4, are green
  for science and routing. Direct candidate-artifact Stata smoke, injected
  rollback, and public-command/64-versus-65 boundary fixtures all pass.
- Two stale public-fixture assumptions also reproduced on baseline. The live
  fixture now requires ADR-0027's LSMR checkpoint family (while preserving its
  exact legacy linear-route assertion), and restores/verifies its accepted
  model after successful saved-FE fits before testing rejected requests.
  No numerical expectation was loosened. The standard exact-commit quick
  wrapper passed on `87a4641`; that is baseline-only evidence, not a claim
  about the dirty candidate. Candidate licensed evidence is the directly
  hashed-artifact fixture run above.

## Local CMG exact nesting: September 5

This round starts from clean `467b4998210f927bc479b11f92b634e1cbcaa00c`,
independently built in a detached worktree. The [immutable diagnostic summary](benchmarks/reports/development/ppml-cmg-exact-nesting-2026-09-05.json)
binds the candidate product-source hashes and artifact, exact baseline,
pinned references, receipts, full timing distributions, split iterations and
tolerances, scientific differences, and modeled/observed memory. Raw evidence
is retained in local temporary bundle `fereg-cmg-nuisance.TGGyIE`. This is local
macOS diagnostic evidence, not qualification.

The width-8 fixture has exactly 800 distinct `(g1,g3)` pairs for 800 `g1`
levels: its nine-level third effect is a function of the first effect. Its
weighted nuisance Schur complement is therefore identically zero for every
positive Hessian weight. The former numeric Schur solve amplified error and
failed the fresh projection certificate (relative infinity 0.067615 at K=0).
The retained PPML-only change proves exact membership containment once per
engine, borrows a two-core projection recipe, and still certifies every original
FE block. It changes neither shared OLS nor upstream CMG, needs no new ADR,
and preserves the original full-topology memory admission and 64-column cap.

Five-repeat medians at N=25,000, nuisance width 8:

| K / threads | Baseline CMG s | Candidate CMG s | MAP s | LSMR s | ppmlhdfe s |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 / 7 | 0.258 | 0.163 | 0.109 | 0.097 | 0.166 |
| 10 / 7 | 0.456 | 0.390 | 0.300 | 0.188 | 0.642 |
| 50 / 7 | 1.352 | 1.214 | 0.997 | 0.549 | 4.747 |
| 0 / 1 | 0.500 | 0.167 | — | — | — |
| 50 / 1 | 4.046 | 3.573 | — | — | — |

- Baseline CMG falls back in all three nested-width cases after respectively
  1/11/51 retried RHSs. Candidate CMG retains four certified ordinary frames,
  65/54/47 first-pass iterations, zero continuation, and zero fallback. These
  are frame maximum-RHS recurrence units, not total graph operator applications;
  baseline failed frames retain total iterations but no certified-frame split
  trace. The matched envelope is science-green but remains route-red solely
  for the baseline failures. All candidate arms are science- and route-green.
- K=0 ordinary CMG setup/solve/certificate totals are 0.014560/0.065390/0.002829
  seconds, versus 0.087053/0.064805/0.000521 for the baseline's failed attempt
  alone. Baseline additionally performs 22 MAP ordinary iterations; candidate
  has none. Candidate checkpoints retain 45 iterations and
  0.011357/0.017759/0.002763 seconds setup/solve/certification. Each first pass
  uses its scheduled tolerance, from `1.38949549437313835e-7` through
  `8.56637091926202050e-11`; publication tolerances are unchanged.
- Non-nested width 32/K=0 is at parity (0.663/0.661 seconds), as are
  width 64/K=0, K=10, and K=50 (1.072/1.073, 1.211/1.212, 2.312/2.308).
  The reversed-order one-thread width-64/K=50 check is 8.377/8.365 seconds.
  Two noisy MAP comparisons return to parity in reversed-order repeats.
  LSMR difficult-chain and sparse-three-FE guards retain identical 38/850
  ordinary iterations and zero retries, at 0.225/0.226 and 1.036/1.024 seconds.
- All 188 matched repetitions retain identical recorded likelihood/deviance,
  sample, rank, DoF, and result names. Maximum coefficient/covariance-cell
  differences are `1.20e-16`/`2.51e-18`; raw public certificate cells differ
  by at most `1.60e-8`, with every independent normalized certificate passing.
  The nested K=0 seven-thread memory model grows by 104 bytes to 155,871,422;
  observed peak delta falls from 21,488,734 to 21,332,636 bytes. At one thread
  the observed peak grows from 20,019,696 to 21,011,300 bytes, still below the
  80,873,686-byte model. No memory allowance changes.
- Validation: 40 focused CMG tests, 765 Python tests, 728 core and 74 normal
  plugin tests plus all workspace oracle/constrained-memory suites, and 89 FFI
  tests pass. Formatting, strict Clippy, universal macOS build/binary/ABI audit,
  directly hashed-artifact Stata smoke and rollback fixtures, and public
  command tests pass. New focused coverage includes exact versus one-row-away
  nesting in either core, all role permutations, borrowed storage identity,
  weighted/offset K=0 and positive-K fits, and forced certified retry. The
  licensed fixture checks all six absorb orders, rank/DoF, objectives, means,
  and saved-effect reconstruction against MAP. The unchanged v3 local science
  (eight scopes) and smoke (three scopes, all nine arms), run through v4, are
  science- and route-green. No SCC, auto calibration, push, or release ran.

## Historical diagnostic performance evidence

- Across the 56 exact-source SCC scopes above, performance-eligible geometric
  mean candidate/`ppmlhdfe` runtime ratios are 0.614 for MAP, 0.379 for LSMR,
  and 0.317 for CMG: reference/candidate speedups of 1.63x, 2.64x, and 3.15x.
  Candidate/baseline ratios are 1.002, 1.003, and 0.995, respectively. Including
  the four scientifically successful but route-ineligible LSMR and CMG scopes
  changes their comparator ratios to 0.372 and 0.316. LSMR is the fastest
  candidate family in 42 scopes, MAP in eight, and CMG in six. On common green
  scopes LSMR beats MAP in 45 of 52 and CMG in 33 of 41.
- The two-FE performance-eligible comparator ratios are 0.475 for MAP, 0.276
  for LSMR, and 0.296 for CMG. The corresponding three-FE ratios are 0.914,
  0.632, and 0.408. The SCC generators already present every three-FE design in
  descending retained-cardinality order, so this tranche is deliberately a
  performance-regression check rather than a favorable reordered-input timing:
  the selected graph roles are identical in candidate and baseline. The local
  permutation tests are the evidence that arbitrary public absorb order reaches
  those same roles and scientific state.
- In the recovered breadth sparse-three-FE shard, candidate/baseline/reference
  medians were 32.469/32.570/6.141 seconds for K=0 MAP and
  21.863/21.796/6.141 for K=0 LSMR; 147.151/147.057/55.641 for K=10 MAP and
  52.742/52.723/55.641 for K=10 LSMR; and
  616.579/617.822/366.329 for K=50 MAP and
  190.926/190.458/366.329 for K=50 LSMR. Candidate and baseline used identical
  total iterations in every pair: 17,457/9,186, 16,146/6,773, and
  17,228/7,130 for MAP/LSMR at K=0/10/50, with no fallback. K=10 and K=50 joint
  LSMR ordinary work used 1,663/1,958 iterations, four retries, and
  1.152/3.904 seconds of solve time; their MAP checkpoints remained the floor
  at 5,110/5,172 iterations and 42.255/178.103 seconds because these 10,000-row
  scopes are below the current-weight LSMR checkpoint threshold.
- Across all 471 matched successful candidate/baseline solver repetitions,
  likelihood, deviance, sample, rank, absorbed DoF, and residual DoF are
  identical at recorded precision. Maximum coefficient, covariance-cell, and
  public-certificate differences are `9.97e-18`, `1.03e-22`, and `3.75e-12`.
  The heterogeneous 56-scope candidate workload sums its per-scope medians to
  2,027.270 seconds and 348,126 iterations for MAP; 729.196 seconds, 132,142
  iterations, 81 LSMR-engine retries, and seven visible checkpoint
  fallbacks for LSMR; and 105.196 seconds, 57,668 iterations, 214 CMG retries,
  and seven visible checkpoint fallbacks for CMG. These are diagnostic workload
  totals, not qualification or auto-routing evidence.

- These historical figures are recomputed from the per-scope solver summaries
  in the 15 shard `analysis.json` files under
  `/projectnb/welfgr/fereg/dev/20260903T170000Z-public-ppml-solvers-2ff872b`;
  all three task-8 scopes are excluded because task 8 produced no receipt.
  Across the remaining 53 scopes, the geometric-mean fereg/reference
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
  current N=50,000 primary uses 0.581 seconds of ordinary solve time and 850
  iterations out of 0.985 seconds, with no continuation. The mixed 3FE graph
  needs 2,578 ordinary iterations and 1.955 seconds of solve time. Simple
  aggregate bins did not produce a sufficiently useful coarse correction.
- Explicit MAP, and therefore `solver(auto)`, remains weak on poorly connected
  graphs. The difficult-chain MAP smoke needs 20,839 total inner iterations
  and about 13 seconds even after its per-pass kernels were accelerated.
- Prepared CMG still performs 11 certificate-driven RHS retries on the hard
  chain. After nuisance batching, width 64/K=0 numeric setup still costs
  0.881 seconds of a 1.049-second fit, against 0.093 seconds graph solve.
  With K=50, setup and solve cost 0.790/0.905 seconds of a 2.236-second fit.
  Exact nesting now removes the width-8 ordinary projection failure, but does
  not reduce the non-nested width-32/64 Schur solves. This is not evidence
  that executor construction is expensive; the earlier 0.01--0.40% result
  remains the relevant bound for a shared-executor API proposal.
  On the K=0 scope LSMR takes 0.102 seconds and pinned `ppmlhdfe` 0.197, so
  this remains a substantial CMG-specific weakness inside its valid envelope.
- Easy one-FE K=50 receives little benefit from better initialization. Its
  canonical weighted-linear/design passes, not FE iteration or inference,
  are the remaining target. Wide sparse checkpoints also remain material at
  one thread: 5.865 seconds of solve time in the 8.129-second K=50 fit.

## Development plan

1. Investigate a topology/harmonic-extension or Schur-aware three-FE coarse
   correction spanning the third effect. Do not restore retained all-pairs
   Schwarz or the unsuccessful aggregate-bin experiment. Judge total operator
   work and elapsed time, with the exact independent certificate unchanged.
2. Reduce CMG's actual changing-weight nuisance/Schur solve work at widths
   32--64; batching has removed the unnecessary K=0 serialization but not
   those solves. The exactly nested width-8 failure is fixed; general
   rank-deficient or near-nested Schur blocks and hard-chain RHS continuation
   remain separate targets. Keep the 64-indicator envelope
   and fresh certificates; do not confuse numeric setup with executor cost.
3. Improve explicit MAP on poorly connected graphs with a topology-aware
   coarse correction or gauge-reduced preconditioner while preserving its
   canonical projection certificates and `solver(auto)` behavior.
4. Profile canonical weighted-linear/design passes for easy one-FE/wide-K and
   wide single-thread checkpoints before changing their implementation. Rank,
   bread, strict convergence, and exact final-Hessian facts remain mandatory.
5. Reconsider an upstream CMG shared-executor API only if a future profile puts
   executor construction above the five-percent promotion threshold. A minimal
   candidate would accept an `Arc<ParallelExecutor>` while still rebuilding
   changing-weight numeric graphs and hierarchies.
6. Keep `solver(auto)` MAP-only. Never rewrite or promote the failed table-2.4
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
