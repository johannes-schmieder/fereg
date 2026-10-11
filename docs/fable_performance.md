# Performance plan: closing the fixest gaps without weakening science

Johannes requested this plan on 2026-10-11 after the October campaigns and
the native phase investigation of source `44a3746` (runtime identical to the
campaign plugin `57cc9789`). It is the working brief for the next
implementation rounds. It builds on the retained SCC evidence in
[SCC results against fixest](fixest_benchmarks.md) and on the native phase
measurements summarised in section 1. Scientific rules come from
[SPEC](SPEC.md), [AGENTS.md](../AGENTS.md) and
[development](development_workflow.md); nothing here relaxes them.

The order is deliberate: first the PPML strict checkpoint (Johannes's stated
priority), then changes that shrink the serial path and the number of
full-matrix passes, which generalise to more cores, larger data and other
networks. Every workstream names its code, its scientific tier, its evidence
and its stopping rule.

## 1. Facts this plan rests on

Campaign (`docs/fixest_benchmarks.md`, one million rows, 16 controls, AKM):

| Workers | OLS fereg s | OLS fixest s | PPML fereg s | PPML fixest s |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 2.232 | 3.762 | 12.807 | 19.444 |
| 4 | 1.292 | 1.861 | 6.417 | 7.905 |
| 8 | 1.043 | 1.511 | 5.324 | 5.879 |
| 16 | 0.955 | 1.325 | 4.966 | 4.698 |

An Amdahl fit through the one- and sixteen-worker points gives about 0.87 s
serial work in OLS and 4.4 s in PPML. Predicted four- and eight-worker times
(1.21/1.04 s OLS, 6.54/5.49 s PPML) match the observations, so the small
8→16 gains are the serial fraction, not a parallel inefficiency. fixest wins
the dense two-FE PPML cells by 36–50% and dense three-FE OLS by 27–47%; the
retained table is in `fixest_benchmarks.md`.

Native phase anatomy (replay harness, campaign-shaped cases, one worker,
this is an observation on a 4-vCPU Linux VM, not retained evidence; the
one-worker totals match the campaign within 5%):

| Phase | OLS 1M AKM | PPML 1M AKM |
| --- | ---: | ---: |
| Input load (per-element SPI emulation) | 0.26 s | ≈0.3 s |
| Preparation (factorize, singletons, topology, DoF) | 0.12 s | 0.9–1.2 s, grows with workers |
| FE projection, of which PCG operator | 1.60 s / 0.50 s | 5.2 s (4 frames) |
| Finite solve | 0.21 s | 1.16 s |
| Strict final solve | — | 2.85 s (24%) |
| Objective, line search, score certificate | — | 0.55 s |
| Total native | 2.33 s | 11.7 s |

What the strict phase is on this case: the solver trace shows one
`canonical_reduced_solve` with 26 iterations at inner tolerance 1e-12 for the
strict candidate (`CertifiedLinearSession::strict_candidate`), then
`profile_final_products|accepted=1`, so the final Hessian products were
reused and no third solve ran. The strict phase is therefore one fresh
canonical solve plus assessment, costing about two warm frames. The
confirmation iteration that precedes it (iteration 4: deviance change 0,
score 1.6e-14) was solved at the same tolerance under the same weights.

Instruction profiles (cachegrind, symbolised `profiling` build): in OLS the
Schur operator costs about 12 instructions per element update and the Gram
update about 10 per fused multiply-add, with bounds checks visible; the two
independent certificates and the norm passes together exceed the operator.
In PPML 27% of instructions are in `canonical_products` and 6% are
out-of-line library `fma` calls: the plugin binary has 374 `fma@GLIBC` call
sites and only five FMA-compiled kernels in
`crates/fereg-plugin/src/fused_kernels.rs`.

Structural causes that are not defects and stay: per-element SPI transfer on
Stata's main thread, independent certification at 1e-8 with 100× tighter
internal solves, the fresh strict PPML solve (SPEC §13), reghdfe-compatible
singleton and `dof(pairwise)` work, and one row-major N×K matrix shared by
all right-hand sides.

## 2. Measurement protocol for every workstream

Native phase timing without Stata:

```sh
cargo build --locked --release -p fereg-plugin --target <triple>
scripts/build_macos.sh            # or build_linux.sh; produces dist/<plugin>
bench/build_timing_harness.sh target/tmp/<task>/timing-harness
cargo run -p fereg-oracle --example replay_large --profile ci -- \
  --output target/tmp/<task>/ppml.case --rows 1000000 --levels 200000,10000 \
  --regressors 16 --weights unit --vce robust --graph random-mobility \
  --solver auto --seed 739391 --ppml
FEREG_PHASE_TIMING=1 FEREG_SOLVER_TRACE=1 \
  target/tmp/<task>/timing-harness dist/<plugin> --replay CASE OUT.json
```

The generated case does not match the campaign call: it requests six
observation outputs, a restricted candidate marker and deterministic
execution, and its topologies differ from the campaign's AKM. Workstream G
adds generator flags for this; until then rewrite the case arguments
(`candidate_all=1`, `deterministic=0`, optional observation `output_*=0`,
`residual=none` for PPML). Keep the required sample output at position 21
(`output_sample=21`); both wire parsers reject zero for this field. Drop the
other output placeholder columns so the varlist has 21 variables,
and replace the two FE columns with the campaign topology from
`bench/campaign.py` (`make_dataset`, design `akm-baseline`). `RRUNTIME` rows
18–21 hold preflight, input-load, native and total seconds for OLS; the PPML
`RRUNTIME` holds total seconds in row 1 and phase seconds in rows 28–40 in
`PPML_PHASES` order. `FEREG_PHASE|...` lines on stderr give sub-phase splits.

Instruction counts: `bench/native.py --cachegrind --threads 1` on 50k–200k
row cases, or `valgrind --tool=cachegrind` on a plugin linked from the
`profiling` Cargo profile for symbols. Linux only.

Qualification is unchanged: `./scripts/run_tests.sh`,
`scripts/run_plugin_replay.sh` with the previous plugin as exact comparator,
`bench/make_standard_cases.py` plus `bench/time_exact.py` on a quiet Mac
against the max(5%, 1 ms) allowance, `scripts/check_complexity.py`, and the
evidence tiers of `development_workflow.md`. Declare workloads, workers,
repeats, timeouts and the stopping budget before each experiment. Keep phase
tracing out of qualification timing. Record results in `docs/VALIDATION.md`
with artifact hashes; never cite `target/` paths as retained evidence.

SCC: use the frozen campaign inputs for exact A/B comparisons (seeded
regeneration on another node differs by ulps); the dense two-FE PPML and
dense three-FE OLS rows in `fixest_benchmarks.md` are the profiling panel,
with the sparse two-FE PPML case retained as the difficult-solver control.

## 3. Workstreams

### A. PPML strict checkpoint (first; needs Johannes's decision)

Today: after two consecutive accepted passes, `StrictCheckpointControl`
(`crates/fereg-core/src/ppml/prepared_irls/controller.rs`) runs
`prepare_attempt` → `CertifiedLinearSession::strict_candidate`
(`crates/fereg-core/src/ols/linear_session.rs`), a fresh canonical solve of
the frozen frame at `strict_inner_tolerance` = min(0.1×tolerance, 1e-12),
then `inspect`/`assess`, then `synchronize`, which reuses the final Hessian
products when `reuse_canonical_final_products` accepts and otherwise runs
`synchronize_final_hessian_frame` (a second strict solve). SPEC §13 requires
"fresh strict WLS at min(1e-12, 0.1×outer_tolerance)" and the exact
accepted-h=qmu Hessian/bread checks.

Observation that motivates the change: `scheduled_inner_tolerance`
(`crates/fereg-core/src/ppml/mod.rs`) already drives the ordinary inner
tolerance to the strict floor once deviance change and score meet their
bounds, so the last ordinary iteration and the strict candidate solve the
same weighted problem at the same tolerance. The ordinary frame is a warm
joint-direction solve; the strict one is fresh. Fresh is the certificate.

Options, in order of preference:

- **A1. Make the final ordinary solve the fresh strict solve.** When
  `strict_control.may_enter` would be true after the next accepted pass and
  the schedule has reached the floor, run that iteration's linear solve as
  the canonical fresh solve (the same path `strict_candidate` uses) instead
  of the warm joint direction, and let the strict transaction adopt that
  frame as its candidate when the response and weight generations match
  (`current_hessian_products_match` already encodes the generation test).
  The strict solve stays fresh and at 1e-12; one full solve disappears.
  Expected saving: about one frame per fit (10–12% of the 1M PPML time here,
  more on dense designs where the strict phase is 7.6 of 28.2 s). Tier:
  declared ulp scope at most, because the accepted eta of the last iteration
  now comes from a fresh rather than a warm solve; discrete decisions,
  certificates and tolerances are unchanged. Witnesses: `prepared_irls`
  strict fixtures, the dense PPML explicit-dummy oracle
  (`scripts/feppml_explicit_dummy_oracle.py`), `ppml_coefficient_accuracy.py`
  and `ppml_covariance_accuracy.py`, and the weak-network PPML control.
- **A2. Warm-start the strict canonical solve from the accepted frame.**
  Keep the strict solve fresh in its inputs and certificate but seed the
  coefficient-space PCG with the previous frame's FE coefficients. SPEC says
  warm starts are guesses, never certificates; the certificate is computed
  on the result, so this is compatible, but it is a ulp-scope change and
  reduces iterations (26 here) rather than removing a solve.
- **A3. Dense geometry choice.** For dense two-FE designs the canonical
  builder's serial neighbour-pair assembly dominates (`fixest_benchmarks.md`,
  "Measured causes"). Choose explicit versus implicit geometry on setup plus
  predicted solve work with independent costing, and consider ordered
  matrix-row ownership for assembly. This is the largest dense-PPML lever
  and is independent of A1/A2.

Decision needed from Johannes before implementation: whether A1's merged
fresh solve satisfies SPEC §13's "fresh strict WLS" (recommended: yes, with
the SPEC sentence clarified to "a fresh canonical solve at the strict
tolerance, which may be the final accepted iteration"), and whether A2's
warm start is acceptable for the strict solve. A3 needs the general costing
rule declared before any route change.

Evidence: declared scope naming the affected fields (final eta, coefficients,
covariance at roundoff), exact replay for every PPML case outside the scope,
independent oracles above, iteration-count and phase-time tables on the 1M
AKM, 1M and 4M dense two-FE, and 1M sparse two-FE cases at 1/4/16 workers.
Acceptance: strict-phase time falls by at least the cost of one frame on
the AKM case, no case loses a certificate, no outer iteration count rises.
Stopping rule: ten substantive repairs or one working day on one blocker,
then record the reproducer and ask Johannes.

### B. Compile the remaining `mul_add` loops to hardware FMA (preserving)

374 call sites call the library `fma` per element; the hottest are
`JointFiniteWorkspace::iterate` (`fe/slope_coefficients/joint.rs`),
`prepare_reusable_group_basis` (`fe/map.rs`), `solve_joint_slopes`,
`absorb_two_fe_pcg_reusing` (`fe/borrowed_map.rs`), `certify_with_bases`
(`fe/certificate.rs`) and the `canonical_products::build` closure. Extend the
runtime-detected table in `fused_kernels.rs` to these loop bodies, or add
function-level multiversioning for them, keeping per-element order. Both
forms are exactly rounded, so this is arithmetic-preserving: zero exact
replay differences and unchanged solver signatures are the acceptance test,
plus instruction counts (expect the `__fma_fma3` share to vanish) and no
wall-time regression on the frozen workloads. Doctor's `cpu_dispatch_path`
label must still describe the active path. Start here if a quick, safe win
is wanted while A waits for a decision.

### C. Shrink the serial path (preserving)

1. Overlap SPI loading with preparation
   (`crates/fereg-plugin/src/estimate.rs` `load_ols_input_with_team`,
   `numeric_pipeline.rs`): read the FE, weight and cluster columns first,
   then run factorization, singleton pruning, topology, DoF and the
   completeness scan on the worker team while the main thread reads the
   regressor columns. SPI stays on the main thread; workers touch only
   Rust-owned memory. Saves min(preparation, regressor load) per call at any
   worker count. Apply the same to `ppml_estimate.rs`.
2. Parallelize preparation (`fe/topology_finalize.rs`, `fe/singleton.rs`,
   `ols/topology_dof.rs`, `ols/prepared.rs`): factorize sources
   independently, build membership offsets with a parallel counting sort,
   keep level order canonical so results are exact. The PPML preparation
   phase (0.9–1.5 s here, 8.9 s on 16M rows) is the first target; measure
   why it grows with the worker count before changing it.
3. Remove serial and column-limited loops: the within-residual loop in
   `ols/prepared_fit/finite.rs` (`finish`), the scale pass in
   `prepared_score_certificate`, and the column-partitioned lanes in
   `fe/norms.rs`, `is_full_retained_unweighted_sample` and the score
   certificate, which idle workers whenever K is below the worker count.
   Row partitions must keep each column's observation order to stay
   preserving; otherwise they belong in E.
4. Lane-owned, lazily zeroed workspaces: large banks are reserved and
   zeroed serially on the calling thread (`reserved_zeros` pattern), so their
   page faults are serial. Let lanes first-touch their own slices inside the
   parallel region where the fallible-allocation contract allows.

Evidence: exact replay, unchanged signatures, and the frozen 8/16-worker
K=20 workloads, which are the regime that benefits.

### D. Fewer passes over the N×K matrix per absorption

1. Gate the initial certificate (`certify_initial` in `fe/borrowed_map.rs`
   `absorb`): the Schur init gather already computes per-level sums of the
   same matrix; use them as a screen and run the independent certificate
   only when the screen passes. The final certificate stays authoritative.
   Preserving: the screen only decides whether a shortcut is attempted.
   Saves one compensated N×K gather per absorption, in OLS and in every PPML
   frame.
2. Compute reference norms once: `build_raw_moment_cache`
   (`ols/prepared_fit.rs`) and the absorber's `weighted_column_norms` both
   pass over the raw matrix; the finite solve recomputes within norms when
   absorption norms are not reused. One kernel for both consumers is
   preserving; sharing bits from different kernels is a ulp-scope change.
3. Tighten `apply_schur` and `upper_rank_one`: fixed-width RHS tiles,
   unchecked split slices instead of per-row re-slicing, explicit vector
   width for the Gram update, with per-element order unchanged.

### E. Declared ulp-scope changes (one workstream, one declaration)

1. Fixed-leaf row-partitioned reductions for norms and certificates, as the
   compensated dot already does (`PARALLEL_DOT_CHUNK_ROWS`), so memory
   traffic no longer multiplies when columns are fewer than workers. Bits
   change once and stay independent of the worker count.
2. Warm-start the two-FE PCG in PPML ordinary frames from the previous
   frame's FE coefficients (`absorb_two_fe_pcg_reusing` starts from zero).
3. Seed the final residual projection from retained FE coefficients so it
   is cheap on slow-mixing graphs (the chain graph repeated the full solve).

Evidence: `--affected-scope` replay with every other field exact, independent
norm and certificate witnesses, instruction counts within noise, prior timing
evidence reused per the ulp-scope tier.

### F. Route robustness (decisions; comparative evidence required)

1. Progress-based handoff from the direct two-FE PCG to CMG or Schwarz LSMR
   in OLS: the checkpoint path has `GRAPH_PROGRESS_CHECK_ITERATIONS = 8` and
   `GRAPH_MAX_PROGRESS_CONTRACTION`; the direct path has no check and ran
   4,246 iterations on an adjacent-firm chain graph. Evidence on chain,
   barbell and weak-mobility designs; no dispatch on names or sizes.
2. Dense three-FE OLS: a general choice based on geometry, observed
   contraction and total work (MAP took 22–24% less time than auto in the
   sampled panel). Sparse controls must not regress.

### G. Tooling (no runtime change)

1. Capture `e(fereg_runtime)` rows 18–21 (OLS) and the PPML phase rows in
   `bench/campaign_stata.do` metadata, so campaigns separate Stata-side,
   load and native time per cell.
2. Add a strict OLS fixest arm (`fixef.tol = 1e-11`, `ssc(K.exact = TRUE)`
   where the DoF convention requires it) so OLS ratios compare equal
   accuracy; keep default and strict profiles separate as today.
3. Add `replay_large` options for campaign-shaped cases: no observation
   outputs, unrestricted candidate, non-deterministic, and an `akm-campaign`
   topology (five consecutive periods per worker, persistence 0.8, random
   destination), so native A/B runs need no hand rewriting.

## 4. Decisions Johannes must make

| Item | Question | Recommended default |
| --- | --- | --- |
| A1 | May the final accepted iteration's fresh canonical solve serve as the strict candidate? | Yes, with the SPEC §13 wording clarified |
| A2 | May the strict canonical solve warm-start its PCG? | Defer until A1 is measured |
| A3 | Declare the explicit/implicit geometry costing rule for dense PPML | Needed before any route change |
| F | Route handoff and dense three-FE choice | Scoped decision after comparative evidence |
| E | Accept one-time norm/certificate bit changes for thread-independent reductions | Yes, as one declared scope |

## 5. Order of work and reporting

1. G1 and G2 first; they cost nothing scientifically and make every later
   claim measurable on SCC.
2. B (FMA coverage) as a contained preserving change with its own exact
   replay and timing evidence.
3. A1 once decided, measured on the four PPML panel cases at 1/4/16 workers.
4. C and D as one preserving workstream with a single wall-time
   nonregression at its end.
5. E as one declared scope; F as separate route decisions.

Each commit states its tier, its witnesses and what it did not change.
Report costs as well as gains, keep failed attempts visible, and delete
`target/tmp/<task>/` after the hashes and summaries are in the commit or in
`docs/VALIDATION.md`. Never loosen a tolerance, change sample, rank, omission
or separation semantics, or add a benchmark-specific fence to win a cell.
