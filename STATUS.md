# fereg 0.5.0 beta status

**Last updated:** 2026-09-01
**Operating mode:** prototype/development; qualification, detailed matrices,
and release evidence run only when Johannes explicitly requests them
**Development branch:** `main`; public OLS and the additive PPML testing surface are active, sharing the certified-linear session while retaining independent PPML wires and the unchanged OLS command path
**Verified candidate plugin:** `9cbe00b6c5a0d5c47af3d5d1fd32166a200d27202d335a602d584a3d5b48a4a2`
**Exploratory combined LSMR/CMG plugin:** `5ccc0cdf8841681d580043f821fa04d1d298c00bdb67d2d12148c96d5a4b8c18`
**Verified optimized-recovery source:** `4e78d015c49f659503aa9f761ca8a73473847387`
**Active milestone:** iterate on the public OLS/PPML prototype with focused changed-boundary checks; Linux/SCC development smoke is opt-in by boundary, and formal cross-platform qualification is deferred
**Release state:** public OLS and PPML beta testing surfaces are active; activation is not a tag or source-qualified release, Windows testing is deferred, and the formal activated aggregate remains open

`STATUS.md` is the mutable project-progress record. The next product target is
a candidly labeled 0.5.0 beta. ADR-0009 supersedes ADR-0008's capped routing
and RSS rule while preserving its scientific and safety contracts. Historical receipts remain
immutable even when they describe earlier hidden or fail-closed checkpoints.

## Solver-routing recalibration development state

- ADR-0021 implements the newly approved development OLS route for categorical
  intercept-only AKM designs. Auto selects CMG when observations are at least
  three times the largest FE level count, the two largest FE counts have a
  ratio of at least 10:1, and additional effects contribute at most nine
  non-base nuisance columns. Other categorical designs go directly to the
  certified LSMR route; slope-bearing/generic designs retain the ADR-0009
  pilot policy.
- ADR-0022 adds a private post-hierarchy safety guard to that route. Direct
  terminals remain CMG; iterative terminals remain CMG only through
  repeat-weighted work 4, otherwise their graph/preconditioner is released and
  auto uses direct Schwarz-LSMR. Rejected setup time is accounted, but the
  planned route has no warning, fallback flag, or MAP pilot. Explicit CMG is
  unchanged.
- Post-pilot LSMR now certifies against the original RHS scale. An uncertified
  finite candidate continues into CG without discarding its state, and all
  attempted iterations, time, preconditioner, warning, and retained
  coefficient work are accounted within the shared budget.
- The development plugin SHA-256 is
  `04cde2f5854e513567dd9551286090d42b01268e5748711c6dc7ab8a351e6be5`.
  The 312-run post-change matrix had 78/78 certified auto runs, zero fallbacks,
  and zero MAP pilots. The 10k--30k sparse three-FE series rose smoothly from
  0.016 to 0.037 seconds. Rust library tests passed 637/637 and the focused
  licensed Stata auto-route fixture passed. This is development evidence only;
  frozen/cross-platform/release qualification remains open. The local `quick`
  profile stopped on the pre-existing one-FE memory telemetry assertion
  `e(fereg_runtime)[6,1] == 1`, before the changed route; substantive memory
  selections and estimates passed an isolated diagnostic rerun. The exact
  final plugin also passed the focused licensed route fixture and 12/12
  recorded auto smoke runs across CMG and direct-Schwarz-LSMR cases.
- The subsequent balanced-AKM diagnostic recorded 4,968 runs in 1,656 cells,
  exactly three repetitions per cell. CMG certified without fallback in all
  408 cases, but 25k--100k matched sensitivity cells exposed large CMG
  graph-size/component cliffs at 2:1 and 5:1 ratios. Ten auto cells had
  deterministic LSMR-to-CG fallback with no MAP pilot; focused traces show CG
  certified an observation-space-identical retained candidate after a median
  0.0025 seconds of added solve time, localizing the event to the
  normal-equation certificate rather than a discarded solve. Forced
  low-memory LSMR setup time rose 4.5x at the median while solve time was
  unchanged, identifying reusable cross-block setup as a separate opportunity.
  No further production routing change was made. The run-level data, corrected
  matched thread/RHS subset, traces, candidate-rule counterfactuals, and report
  are under
  `benchmarks/reports/development/solver-routing-balanced-diagnostic-v1/`.
- A controlled disconnected-component diagnostic then recorded 513 complete
  runs, exactly three repetitions in every solver cell, while holding N, FE
  levels, unique edges, cardinality ratio, and both degree distributions
  fixed. All runs certified without fallback and produced identical exported
  coefficients/RSS across auto, CMG, and Schwarz-LSMR. Component count alone
  is not harmful: equal tiny components are a strong CMG case. The cliff occurs
  when hundreds or thousands of tiny components coexist with a material large
  component. At the current three-observations/10:1 auto-CMG boundary, the
  1,000-component mixed cell took 1.329 seconds under auto-CMG versus 0.116 for
  explicit Schwarz-LSMR with four controls, while 1,100 equal tiny components
  took 0.045 versus 0.098. The bad hierarchy carries contracted tiny-component
  vertices through deep recursive levels; a repeat-weighted hierarchy-work
  proxy rises from 6.4 finest passes at 600 mixed components to 60.0 at 800 and
  82.3 at 1,000, then falls to 2.3 when every component is tiny. No production
  route changed. The next prototype should expose internal component/hierarchy
  diagnostics, reject high-work CMG hierarchies after cheap setup, and test a
  component-aware CMG solve. A separate unrecorded 16-control warm-up anomaly
  remains to be localized with bounded per-stage timing. Sources, raw runs,
  medians, hierarchy table, and report are under
  `benchmarks/reports/development/cmg-component-diagnostic-v1/` and
  `benchmarks/reports/development/cmg-component-autoeligible-v1/`.
- A terminal-aware hierarchy-guard calibration added a private Rust trace at
  the post-hierarchy/pre-nuisance boundary and recorded 333 focused runs across
  37 new component mixtures, plus exact hierarchy profiles for 72 cases from
  the prior balanced matrix. All 330 successful focused runs certified without
  fallback and matched scientifically; explicit Schwarz-LSMR returned
  deterministic `r(459)` in the other three repetitions of one cell. Across
  109 directly comparable joined cells, every iterative-terminal CMG win had
  repeat-weighted work at or below 2.30 and the first iterative-terminal loss
  occurred at 22.44. A candidate veto that retains direct terminals, retains
  iterative terminals only through work 8, and otherwise routes to the
  existing direct auto-LSMR/CG-continuation path rejected 17 cells without
  rejecting a measured CMG win. One greater-than-twofold direct-terminal loss
  with nine nuisance columns remains a separate RHS/nuisance cost-model issue.
  Median graph-plus-hierarchy setup was 0.0037 seconds. No production route
  changed. Source, run-level data, profiles, threshold counterfactuals, and the
  report are under
  `benchmarks/reports/development/cmg-hierarchy-guard-calibration-v1/`.
- The fresh 32-dataset hierarchy holdout recorded 288 runs at new seeds and
  graph wirings, including 0/4/16 controls and 1/8 threads. All 288 succeeded
  and certified. It moved the first iterative-terminal CMG loss down to work
  5.68; combined with the earlier maximum winning work 2.30, this selected the
  conservative development cutoff 4. The post-change rerun also had 288/288
  successful certified runs: auto used CMG in 90 and LSMR in the six guarded
  runs, with zero auto fallbacks, warnings, or MAP pilots. Three fallback
  events belonged to one explicit LSMR comparator cell. An 18-run control
  retained the all-tiny iterative CMG win and rejected the high-work mixed
  hierarchy. The universal plugin
  `ff8592ad6d0b3ad9b00027e5de898ab08bceb6d87a1a4d73e8344d341afbc680`
  passed binary audit, the actual-plugin loader harness, and the focused
  licensed Stata route fixture. The Rust core passed 641 unit tests and 24
  integration tests, plus its empty doc-test target. Direct-terminal wide-RHS
  and single-thread losses remain a separate cost-model opportunity. This is
  development evidence only; qualification remains open. Evidence is under
  `benchmarks/reports/development/cmg-hierarchy-guard-holdout-v1/` and
  `benchmarks/reports/development/cmg-hierarchy-guard-postchange-v1/`.
- The direct-terminal work is now concluded in ADR-0023. Private per-RHS traces
  showed that one `1e-10` LSMR case missed fereg's certificate by only 9--11%
  in two of 17 columns, but `1e-11` reproduced a multi-minute cliff in a
  different ordinary five-RHS, one-thread cell. Tighter LSMR was therefore
  rejected as a deterministic destination. The final reliable comparison
  recorded 366 CMG/CG runs in 61 calibration datasets and 324 runs in 54 fresh
  holdout datasets, exactly three per solver cell; both solvers certified
  throughout without fallback. The locked conservative direct-CG guard
  rejected no CMG win above 5% before implementation. Its 486-run post-change
  replay matched all 54 expected routes (15 CG, 39 CMG), certified every auto
  run, used zero fallback cells, and improved aggregate command time 1.339x
  over retaining CMG everywhere. A bounded 18-run recheck resolved the one
  visibly noisy no-control topology in favor of CG. The universal plugin
  `debb95dcaf648aa11c439deb2861971aeebb34fd707ff5608c6f371b39f7c458`
  passed binary audit and the actual-plugin ABI/self-test. Auto calibration is
  now paused: future work should prioritize component-aware CMG and Schwarz
  setup reuse, reopening thresholds only for a new reproducible common
  regression. This is development evidence, not qualification. Evidence is
  under `benchmarks/reports/development/cmg-direct-terminal-conclusion-v1/`
  and `docs/future_auto_optimization.md`.

The exhaustive V2 scientific/performance matrix is SCC-only. The clean local
macOS matrix was deliberately stopped after 77 strictly valid scopes when this
policy was adopted; those partial results are diagnostic only and must not be
merged or cited as final benchmark evidence. Targeted macOS build, plugin-load,
correctness, profiling, and packaging checks remain required where relevant.

The authoritative SCC parent matrix was stopped at 100 strictly valid scopes.
Original tasks 78, 79, 82, 99, 105--106, and 92--96 hit the runner's 7,200-second
whole-scope timeout without scientific failures and remain rejected unless a
strictly validated retry has been integrated. Retry tasks 78--79 under array
`7344768`, retry task 82 under job `7346188`, and retry tasks 93--94 under job
`7349705` passed all scheduler, application, and scientific gates and were
integrated atomically with their failed parent evidence and retry provenance
preserved. Parent tasks 97--98 and
100--104 also completed strictly. The remaining parent, collector, and retry
jobs were canceled on 2026-08-28; no additional scope may be merged into this
frozen partial population. The separate run
`/projectnb/welfgr/fereg/runs/20260828T194241Z-command-v2-retry-task99-d452dbe`
preserves task 99's parent evidence. The separate run
`/projectnb/welfgr/fereg/runs/20260828T220312Z-command-v2-retry-105-106-d452dbe`
preserves tasks 105--106. The accepted inventory, comparisons, scaling,
overheads, and coverage limits are captured under
`benchmarks/reports/captured/linux-x86_64/stata19/command-comparison-v2-partial-100/`.

## PPML activated testing state

- Public `feppml` is an eclass, sort-preserving one-call wrapper over
  `_fereg_ppml`. The wrapper does not parse twice, call the plugin directly,
  or post outside the guarded transaction.
- The source contains private `_fereg_ppml` and `_fereg_estimate_ppml`
  adapters and one live-SPI `estimate-ppml` action. The action uses independent
  PPML request/result 1.1 wires and a receipt-last transaction. The bounded
  result adds inference/PSD, drop-count, separation-sequence, and digest
  evidence without repurposing the OLS action or result wire.
- The private computation path is organized around prepared data, the shared
  certified weighted-linear session and sandwich kernels, PPML-owned
  separation/ledger rebasing, changing-weight IRLS, inference, certificates,
  result construction, and fit-lifetime memory planning. Legacy dense PPML
  fitting and VCE code is not a second production route.
- `ppml-scientific` exercises the seven shared public fixtures,
  `ppml-hidden` exercises the transaction directly, and `fereg-regression`
  remains PPML-free. The frozen `ppmlhdfe` closure and
  independent explicit-dummy/recession-cone tools are qualification oracles,
  not runtime dependencies.
- These are implemented qualification surfaces, not accepted evidence. The
  final pre-activation source still requires complete local, SCC/Linux,
  supported-build/ABI, memory/performance, transaction/Break, scientific,
  comparator/oracle, and direct-F0 receipts. The candidate F0 identity remains
  unsealed until its required platform evidence closes.
- The activation is a small separately reviewable ado/help/package/profile
  change with no numerical, solver, memory-policy, OLS-wire, or `fereg.ado`
  edit. Earlier evidence is not promoted across that commit. Windows licensed-
  Stata testing and the machine-enforced cross-platform activated aggregate
  are explicitly deferred rather than recorded as passed.

## Current accepted state

- Public `fereg` implements the documented OLS surface. The frozen 40-case
  semantic inventory is executable: multiple slope-only terms, string and
  interacted FEs, lag/lead/difference materialization, iweights, continuous
  DoF, saved outputs, prediction, `test`, and `estat summarize` are no longer
  blockers.
- The README, user guide, compatibility matrix, and installed help now describe
  the implemented public surface rather than the broader 1.0 target. The help
  contains twelve marked, self-contained examples. Examples 1--11 render and
  execute under local licensed Stata/MP 18. The fereg/reghdfe portion of the
  250,000-row benchmark passes sample, coefficient, and RSS assertions and its
  three repeated timing runs. The Stata 19 `areg, absorb(worker firm)
  dfabsorb` arm follows the official Stata 19 syntax but was not executed on
  this Stata 18 host; it is documentation validation, not qualification
  evidence.
- `paper/fereg.tex` and `paper/fereg.pdf` provide a 26-page generic
  working-paper draft for eventual Stata Journal submission. The draft covers
  syntax, examples, architecture, inference, feature comparisons against
  reghdfe 6.14.1, Stata 19 areg/xtreg HDFE, and the StataNow documentation
  accessed 2026-08-30, plus technical and reproducibility appendices. Its
  existing benchmark tables remain generated from the frozen ADR-0013 and
  ADR-0014 evidence. Two new registered three-panel vector figures report
  elapsed time, peak RSS, and peak-to-baseline RSS for a low-mobility two-way
  AKM design with ten controls. Production SCC job `7370995` passed complete
  scheduler, application, provenance, and scientific gates; its accepted
  evidence bundle contains 115 measurements, 23 summary cells, and 11 passing
  preflights. At eight million observations and four processors, median times
  were 15.9 seconds for fereg, 263.6 for reghdfe, and 217.6 for areg, with peak
  RSS of 3.10, 3.75, and 5.40 GiB. Greg remains confined to a beta diagnostic
  footnote, and incomplete broad comparison evidence remains excluded.
- The Stata public path retains one substantive plugin call. Rust owns sample
  construction and computation; SPI access stays on the Stata thread; the
  completion receipt is stored last; and failure/Break tests require rollback
  of `e()`, prior estimates, and requested output variables.
- `memory(fast|low|auto)`, scratch-backed low-memory execution, allocation
  preflight, bounded cleanup, allocation/RSS reporting, `timeit`, thread
  reporting, and deterministic per-action Rayon pools are implemented.
- Public solver controls, action protocol 1.5, and OLS result wire 1.4 are
  implemented. Explicit
  MAP, genuine batched weighted CG over the symmetric-Kaczmarz operator,
  IT/SQUAREM diagnostics, and LSMR paths expose diagnostics and require an
  independent final certificate. Categorical CG borrows the prepared topology;
  slope-bearing designs use the shared owned operator. Finite nonpositive CG
  curvature is restarted safely before publication and nonfinite curvature
  fails closed. CG now requires both reghdfe's Hestenes energy stop and the
  independent FE certificate; neither recurrence nor certificate substitutes
  for the other on the comparator-equivalent path.
- Explicit `solver(cmg)` reuses one graph hierarchy per fit, supports up to 64
  nuisance non-base absorbed-intercept columns, and retains certified CG
  polishing/fallback. ADR-0021 selects CMG before the MAP pilot for the
  development AKM topology described above, ADR-0022 rejects costly iterative
  hierarchies to direct LSMR, and ADR-0023 rejects a conservative calibrated
  subset of direct terminals to direct CG. Neither planned route is labeled a
  fallback. Fast and low modes share the route;
  setup/memory failure returns visibly to direct LSMR, and solve failure uses
  certified CG thereafter. Graph PCG runs at one-hundredth of the public FE
  tolerance to satisfy the OLS score certificate without a separate post-QR
  correction. The release decision is GPL-3.0-only; the repository license,
  dependency notices, and staged package metadata must be synchronized before
  distribution.
- One FE uses direct one-pass demeaning. Categorical multiway designs use the
  ADR-0021/0022/0023 CMG/direct-LSMR/direct-CG topology, hierarchy, and cost
  rules with no MAP pilot.
  Slope-bearing and
  generic multiway designs retain at most four MAP pilot sweeps, the 0.008
  contraction threshold for direct tight Schwarz-LSMR, and CG for remaining
  cases. A finite uncertified auto-LSMR result is polished from its current
  candidate by certified CG rather than restarted. The execution and
  route-specific solver workspaces must fit jointly. Pilot iterations,
  contraction, selected solver, iteration count, fallback, native time, and
  effective thread count are reported.
- Fast-mode saved fixed effects no longer rerun observation-space MAP. LSMR and
  CMG retain their coefficient solutions, CG propagates compact coefficient
  states through its recurrence, Irons--Tuck mirrors accepted extrapolations in
  coefficient space, auto combines its four-sweep MAP pilot with the selected
  continuation, and intercept-only MAP now records coefficients during its
  original batched `y`/`X` sweeps. MAP therefore has no recovery solve in the
  normal fast saved-output path. Routes without a retained product use bounded
  reconstruction-aware CG, then a four-sweep forward-Kaczmarz/steepest-descent
  pilot plus LSMR on the remainder; slopes use strict LSMR. Every route still
  applies connected-component normalization plus independent reconstruction
  and FE-moment certificates at the unchanged publication tolerance.
- ADR-0011 makes retained-coefficient publication part of the bounded native
  thread team. Saved-only calls omit both the persistent `xb` vector and the
  combined absorbed vector; intercept-only coefficient products are scattered
  observation-major; and their reconstruction and FE moments use fixed,
  deterministic parallel chunks. The one-FE path evaluates the certificate
  directly from the target and level coefficients without allocating an
  observation-length recovery residual. Rust tests require bit-identical
  scientific results at 1/2/4/8/16 threads. The V2 Stata prototype passed both
  K=0/one-FE and K=30/worker-firm-year corners at requested 16 threads while
  recording the local Stata license cap of eight separately. No CMG upstream
  change was needed; ADR-0010 continues to govern any routing expansion.
- ADR-0012 replaces repeated full-row finite-moment scans with a bounded,
  deterministic row-major RHS/Gram kernel for up to 64 controls and adds a
  strongly guarded full-rank diagonal-pivoted Cholesky route. Weak,
  rank-deficient, nonfinite, or bread-certificate-failing cases retain the
  existing scaled column-pivoted QR. A second production round uses direct row
  slices for contiguous candidates, computes raw means and stable norms in two
  deterministic row-major passes for up to 64 raw columns, and bypasses the
  cancelling weight/unweight transform for rank-zero fits; noncontiguous and
  wider cases retain their original paths. On the targeted one-million-row,
  one-thread macOS lane, one-FE/K=30 fell from 3.154 to 2.809 seconds overall
  and from 1.903 to 1.462 seconds of native work. Worker-firm/K=30 fell from
  7.127 to 6.816 seconds overall and from 5.825 to 5.444 seconds native. All
  targeted Stata 19 science gates passed, with coefficient discrepancies below
  `6.2e-15` and saved-FE reconstruction errors below `4.5e-15`. The K=0 and
  16-thread totals did not measurably improve, confirming that their remaining
  time is dominated by the Stata/plugin boundary and already-parallel work.
  The 100 accepted SCC scopes remain the authoritative diagnosis; these local
  results are focused prototype evidence only.
  The exact audited universal plugin identified above then repeated the
  one-FE/K=30 science gate and recorded a three-run confirmation at 2.608
  seconds total and 1.338 seconds native; the longer seven-run medians remain
  the conservative performance checkpoint. A broader targeted licensed pass
  then covered iweights, fast/low/auto memory modes, rank-deficient absorbed
  slopes, saved outputs, CMG auto routing, hidden numeric transport,
  final-sample rebasing, and multiple absorbed slopes: all eight fixtures
  passed on Stata 19. The fused reduction changed two hidden-adapter clustered
  covariance receipts by only a few last-place binary units; two independent
  repeats were bitwise identical and the unchanged reference tolerances passed.
  The current worktree passes repository and portable static checks, 291
  Python tests, strict formatting and Clippy, 331 core tests, 53 FFI plugin
  tests, all Rust integration oracles, the universal macOS audit, and the
  actual-plugin ABI loader.
- Exact source `6be8f067f5c54e46e71e470b55beda7eb30a0ea1` passed the focused
  ADR-0012 Linux/SCC qualification. Setup, all twelve benchmark scopes, and
  the final collector have complete `failed=0`/`exit_status=0` accounting.
  The twelve strict shards contribute 432 timing rows and 12 passing
  scientific preflights, cover one through three fixed-effect dimensions,
  0/10/30 controls, standard and low-mobility graphs, fit and saved-FE modes,
  and native thread requests 1 and 16. The exact Linux plugin SHA-256 is
  `60d4b6525aedbb3bfbcfeb9f14d6b1b994545825f03f4fb2461cbfc352ee45a7`.
  In the matched K=30 target population, the geometric mean of the
  fereg/comparator ratio improved by 32.8 percent against reghdfe and 22.3
  percent against Greg. At sixteen requested threads the focused fit/save
  ratios were 0.916/0.839 against reghdfe; hard multiway fit cells were
  0.260--0.650. The scheduler used heterogeneous 16/20/32-processor nodes, so
  absolute cross-run times remain secondary and no noisy cell was filtered.
  The evidence and retry provenance are captured under
  `benchmarks/reports/captured/linux-x86_64/stata19/adr12-focused-12/`.
- ADR-0013 now removes per-column SPI traversal and redundant complete-input
  copies, fills prepared solve storage in cache order, fuses one-thread norms,
  and retains independent residual, score, reconstruction, and bread
  certificates. The first exact-source Linux run (`93292b9`) passed all twelve
  scientific/operational shards but exposed a portable-x86 build defect:
  `f64::mul_add` in the dominant Gram loop compiled to hundreds of millions of
  software `fma()` calls. Exact repaired source `947f793` replaced only that
  loop arithmetic with fixed-order portable multiply/add and passed five
  strict SCC repair shards. One-FE/K=30/one-thread fit fell from 104.278 to
  3.210 seconds and standard worker-firm/K=30/one-thread fit from 109.292 to
  5.080 seconds. In the repaired same-shard comparisons, fereg beat reghdfe in
  all ten fit/save cells and Greg in all four worker-firm cells; worker-firm
  fit was 0.901/0.368 times Greg at one/sixteen threads. One-FE/K=30 fit is now
  2.554 times Greg at one thread and 1.255 times Greg at sixteen, concentrating
  the next optimization target in single-FE serial boundary/publication work.
  Setup and every repair task have `failed=0`/`exit_status=0`; the exact Linux
  plugin SHA-256 is
  `94017d23d97c1a0890cd9a1a180dad4ce3482b5b29cbe6efdbece5016b3e7099`.
  Full and repair evidence is captured under
  `benchmarks/reports/captured/linux-x86_64/stata19/adr13-qualified/`.
- ADR-0014 qualifies certified robust and clustered VCE hot reductions.  The
  broad exact-source SCC stage covered 34 IID/robust/one- through five-way
  cluster scopes, analytic/probability weights, one million/K=10 and ten
  million/K=30 rows, and one/sixteen native threads with 34 green preflights
  and zero command/science failures.  A first bread-hoisting repair remained
  scientifically clean but was rejected because transposed independent-meat
  writes were cache hostile.  Exact source
  `490e03a3e2458d8bc463c3f60389062e2f8fae42` keeps both hot reductions
  contiguous while preserving separate storage and reverse fixed-block
  certificate merging.  Its final eight-scope SCC gate has complete
  `failed=0`/`exit_status=0` accounting, 72 timing rows, eight green
  preflights, exact threads, and Linux plugin SHA-256
  `9cbe00b6c5a0d5c47af3d5d1fd32166a200d27202d335a602d584a3d5b48a4a2`.
  Fereg beat reghdfe in 8/8 final scopes and Greg in all 4/4 sixteen-thread
  scopes; its geometric-mean ratios were 0.987 against Greg and 0.258 against
  reghdfe overall, and 0.620/0.216 at sixteen threads.  Serial ten-million-row
  robust and one-way clustered VCE remain about two times Greg while staying
  2.7--2.8 times faster than reghdfe.  Full staged evidence is captured under
  `benchmarks/reports/captured/linux-x86_64/stata19/adr14-vce-qualified/`.
- Warm local Stata/MP timing checkpoints for saved output are shown below.
  Unless specifically noted after the table, they are alternating-order
  medians. The 300,000-row runs use nine pairs; the 30,000-row CG/IT
  runs use nine pairs, MAP five, and SQUAREM three. Absolute overhead is small
  for retained paths; MAP remains the candid exception even with its best
  tested fallback.

  | Solver | Rows | No saved FE | Saved FE | Delta |
  |---|---:|---:|---:|---:|
  | auto | 300,000 | 0.321 s | 0.377 s | +0.056 s |
  | LSMR | 300,000 | 0.354 s | 0.365 s | +0.011 s |
  | CMG | 300,000 | 0.287 s | 0.331 s | +0.044 s |
  | CG | 30,000 | 0.093 s | 0.100 s | +0.007 s |
  | Irons--Tuck | 30,000 | 0.295 s | 0.310 s | +0.015 s |
  | MAP | 30,000 | 0.858 s | 0.887 s | +0.029 s |
  | MAP | 300,000 | 3.626 s | 3.727 s | +0.101 s |
  | SQUAREM | 30,000 | 2.553 s | 2.950 s | +0.397 s |

  The new MAP rows are alternating-order warm medians (15 pairs at 30,000 rows
  and nine pairs at 300,000 rows); the earlier solver rows retain their
  registered medians. At the original 30,000-row reproducer scale, saved MAP
  output now adds 3.4 percent, down from the earlier 44.9 and 5.6 percent
  checkpoints. At 300,000 rows it adds 2.8 percent, down from 21.2 percent; the
  paired-difference median is 0.098 seconds (2.7 percent of the no-save median).
  These timings include coefficient accumulation, strict normalization and
  certification, and writing two observation-scale Stata variables. The MAP
  path stores coefficients in the projection kernel's native level-major
  order, fuses their update with parallel group means, transposes once after
  certification, reuses exact connectedness from the DoF result, and avoids a
  second observation-scale buffer for saved-only publication. The original
  `tests/manual/simple_AKM.do` reproducer's saved AUTO
  and CMG runs were 0.129 s and 0.066 s. The expanded
  saved-output parity fixture passes MAP, CG, IT, SQUAREM, LSMR, and CMG with
  matching coefficients/RSS and explicit FE-channel reconstruction identities.
  The complete local entry point passes repository checks, 269 Python tests,
  strict formatting/Clippy, 318 core tests, 53 FFI plugin tests, integration
  oracles, mock SPI, and portable static checks.
- `feppml` is active for public testing through the guarded PPML transaction.
  This does not make the current 0.5.0 source a tagged or qualified release.
- The reference closure remains pinned to reghdfe 6.14.1 (`29b2b203...`),
  ftools 2.50.0 (`d6e24d66...`), and require 1.3.1 (`f4760b86...`).
- The normal Stata PLUS tree has the exact local comparator closure: reghdfe
  6.14.1, ftools 2.50.0 (compiled), and require 1.3.1. gtools/gregress 1.11.8
  is pinned as a diagnostic speed target with ado and macOS plugin hashes.
- Committed candidate `786425f2a00049901e6a84877f486a0058aea471`
  passes 293 core tests, 52 plugin
  tests, all Rust integration/doc tests, 258 Python tests, strict Clippy and
  formatting, repository checks, the universal macOS build/audit/actual-plugin
  ABI loader, and the licensed 32-fixture Stata surface after rebasing only the
  deterministic candidate anchors affected by the corrected CG stop. The
  exact-source local full-profile plugin SHA-256 is
  `d7c1182e782049b541fed2f34caf720ecd82d245e4e3a1b8fb67045d260dfa85`.
  The local full receipt records `status=success`, `stata_rc=0`, and the exact
  candidate SHA. This is exact-source local evidence; the final immutable
  release receipt and remaining platform/resource qualification are pending.
- The integrated ADR-0009 implementation adds a sealed 100-digit weak-graph oracle,
  an independent observation-space projection audit, direct/forced-refinement
  comparison arms, route-specific memory planning, and borrowed Schwarz-LSMR
  topology reuse. Its final universal macOS plugin passed the fresh-seed 1m
  holdout, untouched 10m freeze, all 32 licensed Stata fixtures, 269 Python
  tests, strict Rust/FFI/all-target suites, mock SPI, binary audit/loader, and
  deterministic packaging checks. The evidence summary is
  `benchmarks/expected/direct_lsmr_qualification_v1.json`.
- The ADR-0010 100,000-row crossover and guardrail calibration rejected short,
  nuisance-heavy, and nearly balanced graphs and froze only the robust region.
  The permanent manifest/analyzer enforce a five-percent per-selected-cell
  slowdown ceiling and a faster selected aggregate. The upstream CMG pin now
  exposes conservative pre-build and exact retained-memory reports consumed by
  fereg's joint memory planner.

## 0.5.0 beta exit gate

The beta is not ready until default `solver(auto)` is consistently faster than
the pinned `reghdfe` on representative standard OLS problems in the
authoritative SCC/Linux matrix. A comparison counts only after matching
the estimation sample, singleton policy, coefficients and omissions, VCE, DoF,
requested outputs, tolerance, and effective thread count.

For the beta matrix, use warm medians with the registered repetition policy;
include dense and sparse two-way FEs, a worker-firm mobility design, absorbed
slopes, clustered inference, moderate regressor counts, and one- and
eight-thread runs where meaningful. Default `solver(auto)` must be faster than
`reghdfe` in every registered standard case, not merely faster on average. The
stronger ADR-0009 performance targets remain development goals and any case
that does not meet them must be reported candidly. The selected route and any
fallback must be visible in diagnostics and backed by the same final scientific
certificates.

The exact beta source SHA must also pass complete repository checks on SCC,
targeted licensed macOS fixtures, native Apple-Silicon clean-install and binary
audits, repeated-call/rollback checks, and deterministic beta archive staging.
macOS is not an exhaustive-matrix platform. Rosetta, hosted Linux/Windows, and
licensed Windows evidence remain important qualification work, but they do not
replace the authoritative SCC speed, correctness, and scaling gate.

## Completed, evidence-backed baseline

- Four sealed MAP cases at committed source
  `df53822b3263d7425a9a2a42b16b58f7b306dc91` cover dense and sparse two-FE
  designs at one and eight threads. Speedups over the pinned reference are
  2.41x, 2.52x, 2.14x, and 2.40x. Their exact receipts are preserved under
  `build_evidence/local/benchmarks/map-baseline-df53822/`.

- At committed source `a0cc6ecb4aecc6e33bb668c9857ce920f8a5e5f9`, the isolated
  licensed Stata/MP 18 Apple-Silicon quick lane passed 30/30. Its tested plugin
  SHA-256 was
  `10902ebea72314511c7ac03f830486c94fe13a5579da9e0cb260c3fe21547340`.
- The same implementation milestone passed repository checks, 232 Python
  tests, strict C, mock SPI, `cargo fmt`, Clippy with warnings denied, all-target
  Rust tests, and FFI tests. Full workspace counts were 278 core and 50 plugin
  plus integration tests.
- Licensed absorbed-slope checks passed IID, robust, aweight, cluster,
  `dof(none)`, and `dof(firstpair)`. The pure all-slope-only model now matches
  the pinned comparator's structural no-constant result.
- A constrained-memory local fixture passed with a 539,500,000-byte limit:
  fast mode returned `r(909)`, low mode succeeded, and auto selected low.
- The earlier accepted exact quick/full and captured hidden receipts remain
  useful historical evidence, but they do not qualify the current public
  source.

## Active checkpoint: direct Schwarz-LSMR qualified for auto

Calibration evaluated requested and 100-times-tighter direct LSMR,
off/diagonal/Schwarz preconditioners, forced restarted CG correction at 4/8/16
iterations, and the prior coarse-LSMR/ordinary-CG hybrid. The frozen survivor
is direct Schwarz-LSMR at `max(1e-14, tolerance * 1e-2)`, without CG refinement.
The selection, tolerance, preconditioner, pilot threshold, and diagnostics were
frozen before fresh-seed holdouts.

The full 100,000-row matrix produced 148 timing records and ten green
scientific preflights. Only explicit IT failed on the high-mobility case at one
and eight threads; both returned `r(459)` and published no estimate.

The final-hash one-million-row holdout and untouched ten-million-row freeze
passed every hard scientific, stability, runtime, native-allocation, and leak
gate:

| Case | Threads | 1m candidate/old CG | 10m candidate/old CG | Auto route |
|---|---:|---:|---:|---:|
| standard AKM | 1 | 0.986 | 0.973 | CG |
| standard AKM | 8 | 1.033 | 0.996 | CG |
| low-mobility AKM | 1 | 0.262 | 0.231 | direct Schwarz-LSMR |
| low-mobility AKM | 8 | 0.344 | 0.493 | direct Schwarz-LSMR |

At 10m, routed allocation-model errors were 0.4 percent and 20.3 percent at
one/eight threads. Repeated-call leak gates passed. The reghdfe residual
distance was 1.68e-6/1.73e-6 and is report-only under ADR 0009; the independent
100-times-tighter stability distance was 1.19e-7 against a 5.33e-7 envelope,
and all FE/OLS score certificates and comparator RSS/fit contracts passed.

The joint memory planner selects CG when direct LSMR does not fit and CG does,
and fails with `r(909)` before large allocations when neither route fits. The
old process-RSS ratio remains diagnostic; it is deliberately not a promotion
blocker when modeled allocation, cleanup, and science pass.

## Deferred qualification backlog

The items below limit future qualification or release claims; they do not
authorize a test campaign during ordinary prototype work.

1. The activated source needs exact-commit public `ppml-scientific`, hidden,
   Break, staged-install, and full-profile receipts; pre-activation runs are
   diagnostic rather than transferable evidence.
2. The direct `fereg` F0 manifest and final activated comparison are not sealed
   across every required platform. The `fereg` command and OLS wire remain
   frozen while this evidence is collected.
3. Windows licensed-Stata testing is explicitly deferred. Consequently the
   complete PPML scientific/comparator/oracle, memory/performance,
   transaction/Break, clean-install, and platform matrix cannot yet close on
   one activated source and artifact identity.
4. The formal activation aggregate has no waiver path and remains open. Do not
   tag, publish, or describe the source as qualified until Windows and all
   exact-source aggregate inputs pass.
5. The integrated ADR-0009/ADR-0010 route still needs a clean exact-commit
   rebuild, CI, fresh 1m/10m CMG holdouts, the broader release matrix, and
   immutable release receipts.
6. Explicit raw-start `solver(cg)` fails closed on the 1m low-mobility
   diagnostic because its refined residual fails the independent OLS score
   certificate. Auto's four-sweep pilot passes and is the beta acceptance path;
   the explicit escape hatch must remain documented as diagnostic until that
   raw-start score interaction is repaired.
7. Explicit IT remains quarantined after the two registered high-mobility
   `r(459)` failures. It may not influence auto or publish uncertified output.
8. The direct-LSMR qualification covers the registered standard and
   low-mobility 1m/10m cases, and ADR-0014 now covers the broad clustered VCE,
   weight, moderate-K, scale, and thread surface.  The final release candidate
   still needs the combined slopes/saved-output/solver/resource matrix even
   though the component Rust and Stata surfaces are green.
9. Rosetta, hosted Linux/Windows, and licensed Windows evidence is not refreshed
   for this candidate. No tag or publication is permitted yet.

## Development plan

1. Implement the next prototype change and run the smallest focused local test
   for its boundary.
2. When Linux or SCC behavior matters, use the four-slot development smoke and
   select exactly `ols`, `ppml`, or `shared`; inspect its result directly.
3. Run a focused benchmark only to answer a concrete performance or memory
   question.
4. Preserve the deferred qualification backlog and frozen evidence unchanged
   until Johannes explicitly requests a detailed or release-level campaign.
