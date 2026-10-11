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
  --regressors 16 --weights unit --vce robust --graph akm-campaign \
  --solver auto --seed 739391 --ppml --candidate-all --non-deterministic \
  --no-observation-outputs
FEREG_PHASE_TIMING=1 FEREG_SOLVER_TRACE=1 \
  target/tmp/<task>/timing-harness dist/<plugin> --replay CASE OUT.json
```

The G3 flags above request unrestricted candidates, nondeterministic execution
and no optional observation outputs (`residual=none` for PPML). They retain
the required sample output at position 21 (`output_sample=21`) in this
two-FE, sixteen-regressor, unweighted example; both wire parsers reject zero
for this field. No unused output placeholders are written. The `akm-campaign`
topology has five consecutive periods per worker, persistence 0.8 and random
destination firms; its first level count must equal ceil(rows/5). It uses the
oracle's specified RNG, so it is a campaign-shaped diagnostic, not a byte
reproduction of NumPy's campaign inputs. SCC comparisons must still extract
the frozen campaign data rather than regenerate it. Existing generator
defaults and oracle graph meanings are unchanged. `RRUNTIME` rows
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

### A. PPML strict checkpoint (A1 conditionally approved; B qualification required)

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
bounds. This does not establish that the last ordinary iteration and the
strict candidate solve the same weighted problem: accepting the IRLS step
can change eta and therefore the Hessian weights and working response. The
ordinary frame is a warm joint-direction solve; the strict one is fresh.
Freshness, strict accuracy and independent certification must all be proved.

Options, in order of preference:

- **A1. Make the final ordinary solve the fresh strict solve.** When
  `strict_control.may_enter` would be true after the next accepted pass and
  the schedule has reached the floor, run that iteration's linear solve as
  the canonical fresh solve (the same path `strict_candidate` uses) instead
  of the warm joint direction, and let the strict transaction adopt that
  frame only with explicit evidence of fresh canonical provenance and the
  current strict tolerance, and matching accepted-state working response,
  Hessian weights, retained sample and canonical basis.
  `current_hessian_products_match` checks generation labels, exact weights
  and scale, and certification; it does not itself prove fresh canonical
  provenance or strict accuracy. Any unproved eligibility or mismatch uses
  the existing strict path. A saved frame and a 10–12% saving on 1M PPML are
  hypotheses, as are roundoff-only result differences. Tier: declared ulp
  scope, contingent on the witnesses; discrete decisions and tolerances stay
  unchanged, and independent certificates remain required. Witnesses: `prepared_irls`
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

Johannes approved A1 conditionally on 2026-10-11, with the eligibility proof
above and all current safeguards retained: both accepted convergence passes,
independent certificates, meaningful movement/publication-envelope checks,
exact accepted-h=qmu Hessian/bread validation, basis/rank checks, objective
replay, eta reconstruction and atomic rollback. An adopted frame must not
make movement assessment vacuous by being compared only with itself. Clarify
SPEC §13 narrowly with the implementation; the approval authorizes no change
to tolerance, sample, rank, omission or separation semantics. It does not
waive B qualification or other prerequisites. A2 stays deferred until A1 is
measured; A3 requires a declared general costing rule before a route change.

The bounded A1 panel is complete: no frame was adopted and no solve was saved.
Johannes's follow-up on 2026-10-11 directs deferral of this unsuccessful variant,
with its [failed-attempt evidence](VALIDATION.md#performance-plan-a1-failed-frame-adoption-experiment-2026-10-11)
and compact reproducer retained, while C/D continues. Do not broaden A1's
eligibility or weaken its checks. A2/A3 initially remained deferred pending the
concrete proposals below. Johannes's later authorization in section 4 now
permits their bounded discovery and research without reopening A1.

Evidence for A1: declared scope naming the affected fields (final eta, coefficients,
covariance at roundoff), exact replay for every PPML case outside the scope,
independent oracles above, iteration-count and phase-time tables on the 1M
AKM, 1M and 4M dense two-FE, and 1M sparse two-FE cases at 1/4/16 workers.
Acceptance for A1: strict-phase time falls by at least the cost of one frame on
the AKM case, no case loses a certificate, no outer iteration count rises.
Measure actual solve counts and total fit time alongside phase times. Moving
a solve into the ordinary iteration alone is not saved work; demonstrate
fewer actual solves and lower total fit time. Declare affected fields and
verify every unaffected field and discrete decision exactly.
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

#### E declaration before implementation (2026-10-11)

The comparator is qualified C/D, source `aeb6ee64`. Implement E1 before E2
and E3, under this single numerical scope. Fixed row leaves and their merge
order must be independent of the worker budget, including the serial path.
Partials require checked admission and fallible reservation before iteration;
the independent certificate passes remain independent. The scaled norm must
remain at least the largest computed weighted element, so D1's finite-error
proof continues to apply. Any replacement recurrence needs its own proof and
independent accuracy witness before D1 can use it.

Affected numerical outputs may be coefficients, covariance/inference, fitted
values, eta, means, residuals, identified saved-FE combinations, continuous fit
statistics and certificate magnitudes. Input-bound replay scope files must name
the exact numerical leaves before candidate replay. Presence flags, certificate
pass flags, status/omission slots, rank, sample/drop reasons, separation sequence
and trace decisions, solver families/routes, outer iterations and halving counts
remain exact. PPML RCERT indices 0–14 are numerical; index 15 remains exact.
No whole-table or whole-runtime-signature exemption is allowed. Inner iteration
or work counters may differ only at individually named schema locations for an
implemented warm start, with original stopping targets and explicit trace
evidence. Bias numerical leaves require the existing independent exact and
randomized bias witnesses; their controls and support counts remain exact.
Anything outside these enumerated numerical fields remains exact. This is an
allowed envelope, not permission to accept an unexplained changed field.

E2 is limited to ordinary-frame two-FE projection guesses with matching RHS,
sample, topology and coordinates. Refresh the weight-dependent operator and
form `b_new - A_new*x_guess`; retain the original RHS norm as the stopping
reference. Clear incompatible or failed guesses. Strict candidates, final
Hessian synchronization and canonical products remain fresh and cold: A2 is
still deferred. E3 may use an existing compatible coefficient bank, but must
not subtract FE fitted values twice from an already-within Fast residual.
Additional storage cannot change an already selected memory route. Probe actual
eligible call sites before building either optimization; absent eligible work
is a reason to defer, not to expand routing.

Independent witnesses cover weighted scaled norms and score moments, explicit
dummy OLS/PPML coefficients and covariance, the sealed weak-graph reference,
disconnected/chain/barbell/weak-mobility designs, dynamic weights, tiny RHSs,
near-collinearity, both bias modes and saved-FE identified combinations. Exercise
widths 1/17/65/129 and 1/4/16 workers, fixed-worker repeats, allocation failure,
Break and rollback. The small replay corpus alone cannot certify changed
large-row kernels; targeted large-row numerical witnesses are required.

Discovery is bounded to one route-coverage probe and, for each addition, the
four frozen PPML campaign cases plus three difficult OLS graph controls at
1/4/16 workers, two alternating pairs, at most 84 fits and 45 minutes per
panel (120 seconds per fit). At most two substantive repair panels per addition
precede a findings/decision stop. The first certificate, unexplained discrete,
memory-admission or out-of-scope difference blocks that variant. No weaker
threshold or benchmark-specific eligibility fence may repair it. Record costs
and unsuccessful attempts. Full repository and mapped scientific/transaction
checks remain required at integration. Reuse C/D timing only if the declared
ulp tier's instruction-count condition is established; otherwise qualify a
stable candidate with fresh quiet-Mac timing before making a performance claim.

The three discovery OLS controls are frozen generator inputs with 32,768 rows,
two 128-level effects, sixteen controls, analytic weights, robust VCE and seed
739391: chain, barbell and weak mobility. Their independent witness uses a full
dummy weighted QR, one grounded second-effect coordinate, the finite-control
rows of its pseudoinverse and HC1 with the full model rank. It checks the
existing SPEC coefficient/covariance/fitted/residual bounds; exact replay checks
sample and model decisions separately. The 73-case replay corpus has at most
512 rows per input, so E1 must retain its exact legacy bits and cannot use that
corpus alone as evidence for the new multileaf arithmetic.

If fresh timing is needed, use the same twenty frozen Mac standard cells and
two-look rule qualified for C/D: fifteen initial pairs (1,800 seconds), at most
fifteen extra pairs for eligible inconclusive cells (900 seconds), 90 seconds
per fit, and no continuation after an initial point-estimate failure. Retain
all pairs and use the larger of the 98.75th-percentile paired-median bootstrap
and the binomial order-statistic bound with per-look tail at most 0.0125.
The allowance stays max(5%, 1 ms). Result-changing timing requires exact repeats
within each artifact and unchanged fingerprints across blocks; cross-artifact
science comes from the separately scoped comparisons. Tracing is disabled.
Instruction diagnostics retain the four frozen 50k SCC shapes, one worker,
two repetitions per artifact, 600 seconds per instrumented fit and 2,700 seconds
total. No changed artifact inherits an earlier candidate's observations.

The first E1 large-case comparison failed its narrower replay manifest:
OLS Gaussian null log likelihood (`RFIT` zero-based value index 23) changed
by 16 representable steps on the chain control. That attempt stays failed.
The manifest had incorrectly classified this statistic as input-only;
`build_statistics` derives it from within-TSS (already scoped at index 5),
which uses the initial absorbed-response norm. Independent review confirmed
this dependency fits the continuous-fit-statistics declaration above. Before
the fresh bounded panel, name index 23 in each baseline-bound OLS scope and
add FE-only weighted-QR witnesses for within-TSS and Gaussian null likelihood,
using analytic-weight normalization and the existing replay statistic bounds
of rtol=1e-12/atol=1e-14. Preserve the original manifest and failure hashes.
OLS overall TSS and PPML null mean/null likelihood remain exact. This repairs
the numerical manifest, with no new tolerance, counter, route or discrete
exception and no change to the native candidate.

Before timing, compare one baseline/candidate pair on each of the twenty
hash-verified standard workloads (40 fits, 120 seconds per fit, 900 seconds
total). Freeze each input-bound numerical scope from the baseline before its
candidate call; enforce exact protected fields and numerical envelopes,
including binary outputs, and record per-artifact scientific fingerprints.
Bind those fingerprints to the subsequent timing repeats. This preflight is
scientific coverage of the timing panel, not an extra timing look or a
performance claim; the independent reduction and large-QR witnesses above
remain required.

#### E2/E3 coverage decision (2026-10-11)

The completed 84-fit discovery panel preserves protected decisions and fixed-
worker repeats at 1/4/16 workers. Every ordinary candidate PPML frame on the
four frozen campaign inputs uses the admitted joint basis: three frames on
each AKM/dense fit and eight on each sparse fit. These calls bypass the direct
two-FE projection that E2 would warm-start. Defer E2 because this panel contains
no eligible work; do not invent a route to create coverage. The optional Frozen
fallback can reach direct projection, but its Fast RHS is an already-within
correction stream, so a guess for an original response/control is not compatible.

E3 is also deferred. The Fast final residual already subtracts the FE fit, so
subtracting a retained response-minus-controls FE combination again is wrong.
The raw-residual Low path and the public no-saved-FE Fast path have no compatible
retained bank. Adding storage or changing the selected memory route exceeds
this optimization's declaration. Neither deferral changes the fresh strict
canonical solve; A2 still requires Johannes's decision.

### F. Route robustness (decisions; comparative evidence required)

1. Progress-based handoff from the direct two-FE PCG to CMG or Schwarz LSMR
   in OLS: the checkpoint path has `GRAPH_PROGRESS_CHECK_ITERATIONS = 8` and
   `GRAPH_MAX_PROGRESS_CONTRACTION`; the direct path has no check and ran
   4,246 iterations on an adjacent-firm chain graph. Evidence on chain,
   barbell and weak-mobility designs; no dispatch on names or sizes.
2. Dense three-FE OLS: a general choice based on geometry, observed
   contraction and total work (MAP took 22–24% less time than auto in the
   sampled panel). Sparse controls must not regress.

The bounded comparison panel uses one qualified source/artifact and existing
explicit requests; it changes no default route. F1 compares auto/LSMR/CMG on
the three frozen E OLS graph controls plus frozen 1M AKM and dense two-FE OLS,
at 1/16 workers with two reversed-order repetitions: 60 fits, 120 seconds per
fit and 1,800 seconds total. F2 compares auto/MAP on frozen 1M/4M dense
three-FE and 1M sparse three-FE OLS at the same workers and repetitions:
24 fits, 120 seconds per fit and 1,200 seconds total. No budget extension or
automatic retry is planned. Missing cells remain unattempted. The two missing
transport inputs are extracted from their original frozen campaign DTAs into
a separate F manifest; no E-bound input is changed or regenerated.

Retain setup/solve/phase costs, actual preconditioners, iterations, modeled
memory, certificates, failures and exact fixed-worker repeats. Model/sample/
rank/omission/DoF decisions and output presence stay exact across requests;
solver diagnostics and their established effective tolerances can differ
because the explicit requests differ. Numerical comparisons keep the existing
SPEC and stronger replay-statistic bounds, with the three independent OLS
witnesses. Shared-host Linux observations with two repeats are diagnostic,
not quiet-Mac route qualification. This panel has no new holdout and cannot
observe the direct PCG's eighth-step contraction. It may support a bounded
proposal, but does not authorize a progress threshold or default-route change.

Before the first F2 fit, normalize its explicit MAP request to
`preconditioner=off`. F1 exposed a task-driver defect: rewriting only solver and
workers retained the frozen `preconditioner=auto`, which the native syntax
correctly rejects with CMG or MAP. Require exactly one original auto
preconditioner; only F2's MAP arm changes it to off. Every other argument and
the complete data/body bytes stay unchanged. The unstarted F2 job is replaced
with newly bound tooling; its 24-fit/1,200-second/120-second-per-fit limits and
all scientific checks remain unchanged. F1's original request failures and
stronger fit-statistic comparison failures remain failed, without retry.

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

## 4. Johannes's decisions and remaining gates

Johannes's follow-up authorization on 2026-10-11 permits bounded F1 diagnosis,
A2 discovery, and separate A3/F2 costing research. Prioritize demonstrated
total-fit savings: A3/F2 receive the main design effort, while A2 gets a quick,
decisive test of compatible guesses and net cost. Start costing from existing
evidence before spending the calibration budget. A2/A3/F2 may proceed to a
bounded prototype and qualification when their predeclared checks support it;
no additional approval is needed merely to move between those stages. Write a
finite budget for every added prototype/panel before execution and remain
within the limits below. If they are insufficient, return for direction.

Freeze each rule, affected numerical fields, independent witnesses, memory and
fallback/work-budget contracts, and validation protocol before held-out
evaluation. Include representative million-row cases and unseen geometry,
sparse/weak/disconnected controls, workers and memory constraints. Account for
iteration prediction, preconditioner changes and host-dependent costs;
Mac/Linux evidence is not interchangeable. Do not retune after a holdout or
add benchmark-specific fences. Keep current production defaults until a
candidate qualifies. Preserve all tolerances, certificates, explicit solver
contracts, memory controls, transactions and model semantics. Failed
prerequisites block dependent work only. Stop for real scientific ambiguity,
a needed semantic change or the agreed limits. Use separate numerical/costing
agents, stabilize edits and targeted profiles before full timing, and keep
competing work off the measured host. A1 and E2/E3 remain deferred; new evidence
of opportunity alone does not authorize broadening their implementation scope.

| Item | Question | Decision / recommendation |
| --- | --- | --- |
| A1 | May the final accepted iteration's fresh canonical solve serve as the strict candidate? | B qualified; the bounded A1 experiment saved no solve and is deferred by Johannes's follow-up on 2026-10-11. Preserve its evidence; do not broaden eligibility |
| A2 | May the strict canonical solve warm-start its PCG? | Bounded discovery authorized; continue through qualification only if compatible guesses reduce net total cost and preserve the declared safeguards |
| A3 | Explicit/implicit geometry costing for dense PPML | Bounded research and supported prototype/qualification authorized after freezing the rule, scope and protocol |
| F1 | Progress-based two-FE handoff | Bounded diagnosis authorized; failed comparisons alone authorize no handoff threshold or routing implementation |
| F2 | Dense three-FE choice | Bounded conditional geometry research and supported prototype/qualification authorized; sparse timeouts prohibit an unconditional MAP switch |
| E | Accept one-time norm/certificate bit changes for thread-independent reductions | Yes, as one declared scope |

### A2 authorized discovery: retain per-RHS projection guesses

The ordinary joint-fit FE coefficient vector is not a projection of each
original RHS. A compatible producer does exist: `canonical_products::Products`
holds a `right` coefficient bank for the response and each generated control.
`CertifiedLinearSession::try_canonical_products` currently consumes the projected
controls and drops that bank. The proposed experiment would move it into an
optional session-owned guess and use it only in the iterative two-FE reduced
canonical solve. No persistent bank exists at the qualified E1 baseline.

Bind a guess to the unchanged sample, topology, eliminated effect, connected
components and anchors, and exact generated-column identities/order. Retain RHS
scales and recompute all weight-dependent geometry. Convert the old projection
coefficients to current RHS scales and reduced-coordinate roots; incompatible
or nonfinite conversion declines to the existing cold path. Form the new
residual explicitly as `b_new - A_new x_guess`, keeping the original RHS norm
and unchanged strict tolerance as the stopping target. The smaller warm residual
must never become a weaker reference. Fresh strict inputs, original-input
certificates, cold fallback, movement and accepted-Hessian checks remain required.
A guess carries no certificate and is cleared on failed staging or rollback.

For width `W=K+1`, retained-side levels `q` and components `C`, a conservative
payload estimate is `G=8*q*W + 8*W + 8*K + 8*C + 256` bytes, with actual owned
capacities charged by implementation. For K=16, q=10,000 and one component this
is 1,360,528 bytes; no N-row guess copy is needed. Keeping the old bank during
construction adds G to that overlap because construction already includes the
new bank. Move the new bank; cloning would add another G. Admit this optional
storage without changing the already selected Fast/Low route; if the unchanged
budget cannot cover it, use cold solving. The admission and rollback proof is
part of the implementation gate.

Bound the experiment to one design, two substantive repair iterations and one
working day. Discovery uses the frozen 1M AKM, 1M/4M dense and 1M sparse PPML
cases at 1/4/16 workers, one baseline/candidate fit each: 24 fits, 120 seconds
per fit and 30 minutes for the panel. Allow at most one corrected panel, for
48 fits and 60 minutes total. Stop if compatible guesses never reach the solve,
certificates/discrete decisions change, outer iterations increase, or reduced
iterations fail to save total work. Acceptance is lower total cost after guess
conversion, residual construction, setup and certification; eliminating an
entire solve was A1's objective and is not required for A2. Only after discovery passes would seven
alternating pairs plus one warmup per artifact/cell run (192 fits, two-hour cap),
with setup, solving, certification, strict phase and total fit costs reported.
Repository and independent scientific gates remain prerequisites.

Before allocating a retained bank, an eligibility preflight records canonical
producer/strict-consumer identity, selected geometry and weight-dependent
component-anchor fingerprints on the four frozen PPML discovery inputs, one
worker, one fit each. A trace-only task artifact changes no arithmetic or
selection. Limit it to four fits, 120 seconds each and 600 seconds total,
charged against the existing 48-fit/3,600-second discovery maximum. A subsequent
full 24-fit discovery leaves at most twenty corrected calls, rather than an
extra full panel. Initial bank compatibility is necessary, not acceptance;
actual saved work, memory admission and lower total cost remain required.

Expected affected scope includes projected response/controls and derived eta,
coefficients and covariance. Derived means, Hessian weights, working responses,
objectives and continuous score diagnostics must be named if replay changes
them; none may be silently exempted. Sample, rank, omission, separation, solver
family, certificate decisions and outer counts stay exact. Witnesses are the
strict fixtures, explicit-dummy PPML, coefficient/covariance accuracy scripts,
disconnected components, unequal weight scales and weak networks. Saved-FE
outputs need a named witness/scope or must decline this optimization.

The four-fit eligibility preflight supports a bounded iterative prototype,
principally on AKM; it does not establish a speedup. Freeze its first design as
follows before implementation. Admit optional guess storage last, after every
existing installation choice, using the existing solver-increment admission.
Charge two complete guess payloads plus a per-component/RHS iteration ledger
(`2G+L`, actual capacities bounded); keep the canonical builder allowance and
all existing memory routes unchanged. Move only successful canonical banks,
stage their replacement transactionally, and clear guesses on errors, rollback,
incompatible identities and inference handoff. Saved-FE requests decline A2.

For each component/RHS, preserve the original cold normalization and target
and let `M=min(max_iterations, component_dimension)`. A compatible warm attempt
may use at most floor(M/2) iterations; any cold retry gets only M minus all
iterations already attempted. Zero RHS keeps the existing zero solution. A
late numerical certificate failure drops the failed banks before one fresh
cold rebuild, carrying the same consumed-work ledger; fatal errors propagate.
No iteration allowance resets. This policy cannot prove identical fallback
decisions for every finite budget: a failed warm attempt can consume work the
cold solve needed. Exact fallback, certificate and outer-decision checks must
therefore reject any observed difference, including on the sparse control.
Matching anchors cannot exclude that control, and no case-specific fence may
repair a failure.

Before the existing 24-fit discovery panel, allow at most thirty targeted
test invocations and 900 seconds for admission boundaries, unchanged installation
choices, ownership overlap, conversion and original-RHS stopping targets,
shared-cap exhaustion, certificate retry, fatal errors, Break and rollback.
There are still only two substantive repair iterations and one working day;
this test budget adds no native campaign fits. Bind the existing named numerical
replay leaves to each baseline before the corresponding candidate call, and
retain exact protected decisions. Independent strict/dummy/covariance witnesses
remain required before qualification.

### A3 authorized research: costing before route changes

For component dimensions `d_c`, total reduced dimension `d`, RHS width `W`,
neighbor-pair assembly count `P` and predicted RHS iteration counts `I_j`,
compare these setup-plus-solve models:

- Explicit direct: `a*P + b*sum(d_c^3) + c*W*sum(d_c^2) + common`.
- Explicit iterative: `a*P + e*sum_j(I_j*sum(d_c^2)) + common`.
- Implicit iterative: `f*N + g*sum_j(I_j*(N+d)) + common`.

`common` includes RHS construction, reconstruction and independent certification;
include differing storage/transfer costs explicitly. Constants depend on resolved
workers. Hard memory admission remains separate. Both explicit and implicit
alternatives must be priced before `canonical_products::Plan::new` rejects a
route on neighbor-pair work. Compatible completed-call observations may inform
iterations; cold calls need a conservative structural estimate. No campaign
names, tuned case exceptions or benchmark-specific thresholds are allowed.

Proposed calibration: N in {2^14, 2^17}, eliminated-group degree in {4,16,64},
RHS width in {2,17}, workers in {1,4,16}, two repetitions of each feasible
route: at most 216 kernel measurements. Fit nonnegative coefficients once.
Hold out four distinct graph shapes, including skewed degrees and weak bridges,
at N=2^18, widths {8,65}, workers {1,4,16}, two repetitions per feasible route:
at most 144 measurements. Cap the exercise at 45 minutes and one model revision;
held-out results judge the frozen model and do not tune exceptions. Unstable
predictions or gains requiring exceptions stop the experiment. Research begins
with retained observations. A supported rule may proceed to a bounded prototype
and qualification under the authorization above, with its numerical scope,
independent witnesses and added finite panel budget declared first.

The diagnostic budget is allocated across at most 216 calibration route calls,
144 frozen-holdout route calls and 48 million-row end-to-end calls, with a total
2,700-second limit. Calibration may use two repetitions per feasible route on
the declared N/degree/width/worker grid; component work and RHS scheduling waves
replace a naive division of all costs by worker count. Implicit product work
uses aggregated grounded incidences, while actual storage remains separately
charged. Before fitting a rule, a task-only diagnostic artifact may force an
existing explicit-direct, explicit-iterative or implicit-iterative geometry,
but all current memory checks, stopping targets and independent certificates
remain binding. Infeasible or failed calls are recorded, never relabelled.

First spend at most 36 calibration calls (N=2^14, degree 4/16/64, width 17,
workers 1/16, three feasible geometries, two repetitions) and eight whole-fit
calls (frozen dense PPML 1M/4M, workers 1/16, current/implicit, one pair), with
30/120-second per-call limits and 720 seconds combined. These are subsets of
the maxima above, not additional budgets. Record complete setup, factorization,
RHS, reconstruction, certification and total costs, per-RHS/component work,
memory, failures and outer counts. Geometry changes may affect projected
response/controls, moments, coefficients, eta/mu, objective, bread/covariance,
derived continuous statistics and solve-work diagnostics. All model decisions,
success/error behavior, requested tolerances, certificate gates and unaffected
fields stay exact; no outer-iteration increase qualifies. Existing independent
incidence/dense, unequal-weight/cancellation, disconnected, full-dummy PPML,
strict-Hessian, memory/rollback and Break witnesses remain prerequisites.
No production costing rule is inferred from a single diagnostic pair. Freeze
the predictor and its uncertainty/abstention policy before held-out evaluation;
actual held-out iterations may diagnose the model but may not retune it.

The first kernel subset fixes 128 retained-side levels and
`N/(2*degree)` eliminated-side levels. Each unique edge has two observations
with equal dyadic weights and opposite dyadic residuals, giving an independent
exact FE-orthogonality witness; weighted Gram products are checked separately.
The three task-only route artifacts share these inputs. This narrow geometry
is calibration, not an unseen holdout or a general iteration model. Trace
overhead remains in the clocks; factorization is nested in setup, and the
`4*N+q` admission proxy covers selected incidence work, not total setup.

Before fitting a selector, extend calibration by the following fifteen cells,
three existing geometries, workers 1/16 and two reversed-order repetitions:
180 calls, at most 30 seconds each and 600 seconds combined. Together with the
first 36 this uses the existing 216-call calibration maximum; the 144 holdout
and remaining forty million-row calls stay untouched. This explicitly amends
the earlier grid with width 4 and degree 256. Width 4 provides a narrow admitted
RHS bank; width 2 would refuse every route on that proposed zero-prior-work cell.
No route is changed to manufacture a feasible calibration observation.

| Cell | N | Left levels | Right levels | Degree | Repeats per edge | Width | Geometry |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 1 | 16,384 | 2,048 | 32 | 4 | 2 | 17 | Regular |
| 2 | 16,384 | 2,048 | 256 | 4 | 2 | 17 | Regular |
| 3 | 16,384 | 2,048 | 2,048 | 4 | 2 | 17 | Regular |
| 4 | 131,072 | 2,048 | 128 | 4 | 16 | 17 | Regular |
| 5 | 131,072 | 2,048 | 256 | 4 | 16 | 17 | Regular |
| 6 | 131,072 | 2,048 | 2,048 | 4 | 16 | 17 | Regular |
| 7 | 131,072 | 16,384 | 128 | 4 | 2 | 17 | Regular |
| 8 | 131,072 | 4,096 | 128 | 16 | 2 | 17 | Regular |
| 9 | 131,072 | 1,024 | 128 | 64 | 2 | 17 | Regular |
| 10 | 131,072 | 1,024 | 1,024 | 64 | 2 | 17 | Regular |
| 11 | 131,072 | 16,384 | 128 | 4 | 2 | 4 | Regular |
| 12 | 131,072 | 256 | 256 | 256 | 2 | 17 | Fully crossed |
| 13 | 131,072 | 1,024 | 128 | 64 | 2 | 17 | Two disconnected blocks |
| 14 | 131,072 | 1,024 | 1,024 | 64 | 2 | 17 | Two blocks, two bridges weighted 2^-20 |
| 15 | 131,072 | 1,024 | 1,024 | 64 | 2 | 17 | Same topology, bridge weights 1 |

Freeze row order (left group, edge, repeated observation), neighbor rule
`(group+17*edge) mod right_levels`, paired dyadic residuals and additive FE
values, and seed 739391 for RHS amplitudes before these calls. Block cases apply
the same neighbor rule locally; cells 14/15 replace the last edge of the first
left group in each block with the other block's first right level. All nonbridge
inputs match between those cells. Ordinary edge weights are
`(2+(group+3*edge) mod 7)/8`; each repeated pair shares its weight and has opposite
residuals. The allowance is 256 MiB, tolerance 1e-12 and prior work zero.
Keep the existing exact-orthogonality and direct-Gram witness bounds, explicit
refusals and fixed-worker scientific repeats. Raw centered-incidence energy,
anchor mass fraction and normalized diagonals are diagnostic features, not
floating-point certificates. Refusals retain their reasons and completed work;
budget exhaustion is incomplete, never a successful panel. No holdout is used
to revise this calibration design or to tune a rule.

The additional calibration stopped at its first timeout, as declared: cell 3,
explicit iterative geometry, one worker exceeded 30 seconds after completing
eleven RHS solves of 962–963 iterations each. Twenty additional calls completed
their independent witnesses, five refused existing admission gates, and 154
were not attempted. The first 36 kernel calls had completed successfully.
No coefficients were fitted and no holdout was used. This incomplete grid does
not support a general production cost/iteration rule; retain the current route.
The [stopped calibration evidence](VALIDATION.md#a3-calibration-stopped-at-the-sparse-work-limit-2026-10-11)
preserves costs and a disposable reproducer. Resuming or redesigning this
calibration requires a new bounded decision rather than continuing the unused
budget automatically.

### F1 authorized diagnosis: resolve the comparison before handoff

Retain the current automatic route. The bounded F1 panel did not establish a
passing alternative across its declared comparisons. Twenty CMG requests were
invalid because the comparison driver retained `preconditioner=auto`; these
syntax failures are tooling failures, not CMG performance observations. Eight
auto/LSMR comparisons failed the stronger robust-F statistic bound on barbell
and weak-mobility controls, although both routes passed the independent
coefficient, covariance, fitted/residual and within-statistic witnesses. The
independent QR covariance gives a robust F much closer to LSMR on the weak
control. Investigate this discrepancy rather than relaxing the comparison bound
or copying the baseline value. The original F1 attempt remains failed.

The present authorization is diagnostic only: trace the robust covariance
and Wald-statistic error to its source, prepare valid CMG requests, and measure
actual ordinary-PCG progress before choosing a handoff threshold. Bound this
work to one working day and two substantive repair attempts. A new native panel
would be predeclared separately, at most 60 fits/1,800 seconds with 120 seconds
per fit on the same inputs, retaining the failed original observations. Do not
repeat the panel until a diagnosed request or numerical repair justifies it.
Any runtime repair needs its own declared scope and independent witnesses.

The first authorized diagnosis is eight Mac native captures on the regenerated,
hash-matched frozen barbell and weak-mobility controls, one worker: Auto at
1e-8, diagnostic Auto at 1e-10, LSMR at 1e-8, and valid CMG at 1e-8 with
`preconditioner=off`. Use the qualified E1 plugin, tracing enabled, no warmup,
120 seconds per fit and 600 seconds total, no retry. Preserve every full beta
and covariance matrix, the original F1 statuses and the unchanged strong bound.
Compare an 80-digit Wald calculation on each observed covariance with mixed
beta/covariance calculations using independent full-dummy QR/HC1. The tighter
Auto request tests sensitivity only; it changes no production tolerance or
comparison allowance. A mismatch in either retained input hash stops the panel.
No timing or Linux replication claim follows from these Mac diagnostic calls.

If the eight captures isolate covariance influence rather than Wald rounding,
allow one follow-up capture of the accepted within-control bank and bread for
the weak control, Auto/LSMR, one worker: two calls, 60 seconds each and 120
seconds total, no retry. Use a task-only instrumented copy; retain exact public
science against each uninstrumented arm and compare projected controls,
bread and resulting influence against independent QR. This is numerical
diagnosis within the same one-day/two-repair limit, not a route prototype or
an authorization to strengthen/relax a production stopping policy silently.

An eventual Auto-only handoff should occur before reconstruction mutates the
original RHS matrix. Return typed ownership of that unchanged input, preserve
sample/topology/weights and saved-FE obligations, and keep explicit requests
unchanged. Release attempted numerical banks before alternative setup or charge
their full overlap under the public memory budget. Debit all attempted work
against one iteration budget and preserve the existing MAP safety reserve.
Progress may reject an attempt; only the full independent certificates may
accept a result. The checkpoint's iteration-eight contraction rule is not
calibrated for ordinary OLS, and the current panel does not observe that signal.
Its chain reports 127 iterations, not the plan's historical 4,246; the latter
reproducer was not available. No threshold or default-route change is proposed
for approval from these observations alone.

### F2 authorized research: geometry with sparse protection

Retain the current automatic route. MAP completed the frozen dense 1M/4M
three-FE comparisons with lower observed native costs at 1/16 workers, but
all four sparse MAP fits hit the unchanged 120-second limit. The 24-fit panel
finished with twenty passes and four timeouts, without retry. These are two cold
shared-host repetitions, not a qualified performance claim. The failed sparse
fits have no accepted timing ratio, and the dense gains cannot justify an
unconditional switch. Full outcomes and costs are in the
[retained comparison record](../bench/results/f-20261011-comparisons.json).

The first stage is costing research, bounded to one
working day and one model revision. First use the retained successful phase,
iteration and memory observations to test whether already-available geometry
can explain total setup-plus-solve cost. Candidate features are FE levels and
term slots, group-size/connectivity structure, RHS width, selected memory and
the resolved preconditioner. N alone or a campaign name is not a route rule.
The dense 1M/4M cases have 186/297 total term slots and straddle the existing
256-slot Diagonal/Schwarz boundary; treat that difference explicitly. Charge
any additional setup, progress observation and overlapping memory to the
proposed rule. Sparse timeouts are censored failures, not fitting targets with
invented completion times. If these observations cannot support a general
cost model, stop and specify the missing diagnostic before running more fits.

The first F2 diagnostic prices a possible bounded MAP attempt at its existing
first four-sweep certificate checkpoint. It does not implement a default route.
Use the three frozen dense 1M/4M and sparse 1M inputs, workers 1/16, one Auto
capture and one explicit MAP request with an iteration limit of four per cell:
twelve calls, 60 seconds each and 600 seconds total, no retry. Add trace-only
checkpoint costs without extra numerical checks or modified stopping bounds.
An exhausted MAP request remains a failure with no completed-fit ratio. Report
primary and final-residual work separately; a primary failure observes no final
residual cost. The four-sweep cadence is not a convergence-rate estimate.
Compare incremental pilot/copy/certificate cost with the unchanged allowance;
the source-qualified retained Auto observations are diagnostic references, not
quiet-host confidence bounds. Any later pilot prototype needs typed untouched-
input ownership, fully charged backup/overlap and a shared work budget: debit
pilot work from the first-half LSMR allowance while preserving the existing
MAP safety reserve. No routing code follows from these censored probes alone.

The completed checkpoint diagnostic rejects a general four-sweep pilot on
sparse cost. A separate geometry-only diagnostic may price a sufficient
convergence predictor: centered pair-incidence Frobenius bounds for three
unit-weight intercept effects, propagated through the existing symmetric MAP
order. Exact independence is not required. Extra pairwise common directions
remain in the bound; do not silently remove them as though they belonged to
the common space of all three effects. The predictor never accepts a numerical
fit or replaces any existing certificate.

Before any conditional-route prototype, independently review this derivation
and its rounding/normalization limits. A task-only probe may compute checked
integer contingency moments with reusable smaller-level count storage and
report their bounds, scratch capacity and setup cost, without changing solving.
Bound its correctness checks to ten synthetic feature cases with direct
contingency/explicit-projection witnesses, and its pricing to six original
frozen Auto fits (dense 1M/4M and sparse 1M, workers 1/16), 120 seconds per fit,
600 seconds total and no retries. Require exact retained scientific outputs.
Charge all histogram visits, clearing and allocation; the measured cost must
fit the unchanged sparse allowance before progressing. This adds no MAP pilot
or holdout calls. Weighted models, a floating convergence guarantee and a
default selector remain outside this diagnostic.

The six geometry-pricing calls completed with exact retained public science.
One cached computation fits the diagnostic sparse allowance (about 21 ms,
0.18%/0.59% of the same-host approximate Auto cost at 1/16 workers). Recomputing
geometry for both primary and residual calls costs over 8% on the dense
sixteen-worker cases, so that duplication is not a supported selector design.
The bounds distinguish the observed dense and sparse inputs, but this does not
identify a general total-cost rule. Existing dense observations change N,
levels, preconditioner and LSMR iterations together; rollback-copy, overlap and
failed-attempt cleanup costs are absent, and the MAP bound does not predict
cold LSMR work. No selector or holdout follows from these calls.

Stop at the following missing diagnostic rather than invent coefficients.
A new bounded decision would freeze a controlled same-host phase calibration
that varies RHS width, topology and the existing preconditioner regimes
independently. Price geometry, a fallible rollback copy, MAP workspace, four
sweeps and the original-input certificate against unavoidable Auto setup alone,
with explicit uncertainty and abstention before distinct holdout geometries.
If even that conservative setup-only comparison admits no useful cases, a
separately declared cold-LSMR work predictor is necessary. Repeating the same
dense fits does not resolve this identification gap. The
[retained geometry pricing](VALIDATION.md#f2-geometry-pricing-and-the-missing-total-cost-model-2026-10-11)
records both the positive feature result and the unresolved model gate.

Before any prototype, freeze its features, parameters, numerical scope,
fallback/work-budget contract and finite validation budget. The current automatic
LSMR attempt uses a tighter direct tolerance and at most half the shared
iteration budget, with MAP fallback from untouched original inputs. Preserve
these protections and explicit solver contracts. A supported bounded prototype
and qualification are authorized under section 4; no unvalidated dispatch rule
may replace the current production default.

A proposed holdout must use distinct geometries on both sides of the existing
preconditioner boundary, including sparse, weak/disconnected and memory-limited
cases. Declare its cases and bounds before execution; do not reuse the current
tuning cases or revise the rule after seeing held-out results. Independent
explicit-design witnesses, unchanged model decisions, fixed-worker repeats,
public memory/refusal and Break checks precede quiet-Mac qualification. The
completed 24-fit panel includes no such holdout and by itself supports no
default change.

## 5. Order of work and reporting

1. G1 and G2 first; they cost nothing scientifically and make every later
   claim measurable on SCC.
2. B (FMA coverage) as a contained preserving change with its own exact
   replay and timing evidence.
3. A1 after B qualification, under the conditional approval, measured on the
   four PPML panel cases at 1/4/16 workers.
4. C and D as one preserving workstream with a single wall-time
   nonregression at its end.
5. E as one declared scope; F as separate route decisions.

Each commit states its tier, its witnesses and what it did not change.
Report costs as well as gains, keep failed attempts visible, and delete
`target/tmp/<task>/` after the hashes and summaries are in the commit or in
`docs/VALIDATION.md`. Never loosen a tolerance, change sample, rank, omission
or separation semantics, or add a benchmark-specific fence to win a cell.
