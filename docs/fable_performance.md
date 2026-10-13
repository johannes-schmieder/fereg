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
| A2 | May the strict canonical solve warm-start its PCG? | Discovery stopped: all twelve scientific comparisons pass, but total native cost rises 1.883%; retain the prototype without integration or dependent qualification |
| A3 | Explicit/implicit geometry costing for dense PPML | Calibration stopped at the sparse work limit without a fitted rule or holdout; further calibration needs the bounded decision described below |
| F1 | Progress-based two-FE handoff | Diagnosis complete; retained robust-F comparison failures authorize no handoff threshold or routing implementation |
| F2 | Dense three-FE choice | Geometry feature priced with exact science; total-cost model remains unidentified, so the controlled calibration described below needs a new bounded decision |
| E | Accept one-time norm/certificate bit changes for thread-independent reductions | Yes, as one declared scope |
| H1c | Skip the `within` locality sort when it cannot change the scatter kernel (declared ulp scope) | Approved by Johannes on 2026-10-12; see section 6 |
| H2 | Dense three-FE MAP prototype gated by the certified contraction bound, LSMR fallback | Prototype approved by Johannes on 2026-10-12; freeze rule, fields, witnesses and budget first; see section 6 |
| H3 | Parallel neighbour-pair assembly in canonical products | Arithmetic-preserving variant needs no decision; the private-accumulator variant is a separate ulp-scope declaration |
| H8 | Consistent recurrence targets across automatic routes, certificate unchanged | Approved by Johannes on 2026-10-12: a hundredth of the request with the 1e-14 floor, acceptance only by the certificate at the request, no new rejection or re-route; see section 6 |
| H9 | Reuse a prior canonical preconditioner with an exact current-weight operator | Proposed implementation plan in section 7; first resolve SPEC's backend-invalidation wording and prove/cost the inverse action; strict checkpoint before ordinary frames |

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

#### A2 discovery outcome (2026-10-11)

The [retained prototype record](../bench/results/a2-20261011-prototype.json)
and [disposable reproducer](../bench/results/a2-20261011-prototype.patch)
close this experiment at its lower-total-cost gate. All twelve comparisons
across four frozen cases and 1/4/16 workers pass the declared numerical bounds
and exact protected decisions. Six calls accept compatible guesses, on AKM
and dense 4M. Native time nevertheless rises from 191.026 to 194.623 seconds
(1.883%); RRUNTIME total rises from 191.013 to 194.610 seconds. Sparse four-worker
native time rises from 17.458 to 21.671 seconds. These are single traced SCC
pairs, with baseline first, so they neither attribute the difference to a
particular phase nor establish a quiet-Mac regression. They fail the frozen
discovery benefit condition without a retry or a new routing fence.

Both allowed substantive runtime repairs were used: reject an explicitly
nonimproving initial residual before spending iterations, then clear retained
banks on early error boundaries. Sixteen focused runtime witnesses and thirty-four
driver checks pass. The first panel attempt rejected valid output metadata
before any native fit; its 10.680-second failure remains charged and retained.
The corrected panel completes all twenty-four fits in 598.391 application
seconds. Including the four eligibility fits and failed preflight, the discovery
book totals twenty-eight fits and 736.220 seconds; unused allowance does not
authorize continuing after this stop.

AKM's strict candidate reports 430 recurrence iterations. Its initial residual
operator products are outside that counter. The baseline maximum of 26 gives
at most 442 recurrence applications across seventeen RHSs. If all seventeen
guesses reach residual construction, the candidate performs 447 applications,
despite fewer recurrence iterations. The repeated `used` trace flag is call-wide,
so this is a conditional work diagnostic, not a measured per-RHS operator count
or timing attribution. The measured total-cost failure is sufficient to stop.

The production runtime remains qualified E1. No full prototype integration
suite, mapped strict/dummy/covariance qualification or quiet-Mac timing follows
this failed prerequisite. All tolerances, model semantics, certificates, fresh
strict solving, memory controls and SPI ownership remain unchanged in production.

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
6. Workstream H (section 6) after C2: H1 and H3 first, then H8 so H2 is
   judged against consistent tolerances, H2 under its approval, H4/H5 as
   bounded checks and tooling, H6 by removed pass, H7 check only.
7. H9 (section 7) is the proposed follow-up to the measured H3 implementation:
   inventory actual reusable geometry, prove its preconditioner action, test
   strict-checkpoint reuse, then consider ordinary frames. Certificate pass
   fusion is a separate preserving change; A2 reconsideration follows only
   demonstrated H9 savings.

Each commit states its tier, its witnesses and what it did not change.
Report costs as well as gains, keep failed attempts visible, and delete
`target/tmp/<task>/` after the hashes and summaries are in the commit or in
`docs/VALIDATION.md`. Never loosen a tolerance, change sample, rank, omission
or separation semantics, or add a benchmark-specific fence to win a cell.

## 6. Workstream H (2026-10-12): data-order cost, serial assembly and placement

Johannes reviewed the October 12 rerun and the C2 adoption on 2026-10-12 and
authorized this workstream. He approved the dense three-FE route prototype
(H2), the declared ulp-scope change in H1 step c and the tolerance
consistency rule in H8. Everything else below is arithmetic-preserving or
tooling. The order is by expected general gain per
unit of risk; H1 and H3 come first because each removes a serial cost that
grows with the data rather than with the worker count.

### Facts added on 2026-10-12

The October 12 rerun (`fixest_benchmarks.md`, "Optimized rerun") moved every
cell to a slower host, so only paired fixest/fereg ratios are comparable.
Dense two-FE PPML improved from 0.502 to 0.563 at 4M rows; dense three-FE OLS
did not move (0.530 to 0.510 at 4M, 0.745 to 0.710 at 1M) and is now the
largest relative gap. It runs the same three-FE LSMR route as the 16M-row
fixed64 holdout (fereg 20.441 s against fixest 7.427 s at four workers).
In that holdout the projection phase grows 36.4x for 16x rows while the LSMR
iteration count falls from four to three; every other phase grows linearly.

Reading the pinned `within` revision `297f32d9` explains the signature.
`Design::build` (`crates/within/src/domain.rs`) picks the term with the most
coefficients as dominant (`max_by_key`, so ties go to the last term) and, when
its codes are not sorted in caller order, locality-sorts the whole frame
through the serial `ObservationFrame::gather`. `Solver::solve_rhs` then runs
`permute_obs_in` on every right-hand side and `permute_obs_out` on every
demeaned result, both serial indexed loops over N. The sort exists to enable
the `SortedCoalesced` scatter kernel, but `ScatterStrategy::pick`
(`operator/design/scatter.rs`) uses that kernel only when a term has at least
`SCATTER_LOCAL_THRESHOLD = 100,000` coefficients; smaller terms use the
thread-local `Fold` kernel whether sorted or not. For the fixed64 holdout
(three 64-level effects with irregular group sizes) the sort therefore buys
nothing and costs a near-random 128 MB permutation per column plus two
serial random passes per right-hand side per solve. With `deterministic` off,
fereg's explicit route (`fe/lsmr_batch.rs::solve_batch`) also rebuilds the
`Solver`, and so the sort, on every call; the prepared Fast route retains it.
AKM inputs are sorted by the dominant worker factor and skip the sort, and the
dense campaign design's dominant factor changes level only every 10,000 rows,
so its permutation is block-wise and cheap. That is why the dense three-FE gap
needs H2 rather than H1.

Other serial passes per right-hand side in `within::Solver::solve_rhs`:
the finite check, `weighted_rhs`, the demeaned subtraction, `vec_norm`
(twice per solve), the `true_residual` subtraction and the batch
concatenation `demeaned.extend_from_slice`. The LSMR kernels themselves
(`schwarz-precond/src/lsmr/bidiag.rs`) are chunked and fix their association,
so they are not the problem.

For dense two-FE PPML the A3 panel already found the cost: forced implicit
geometry saved 7.9 s of 18.4 s at 4M/16 workers and 1.08 s at 1M because the
explicit canonical geometry's neighbour-pair assembly
(`ols/canonical_products.rs::build`, the `for g in 0..p` loop over `touched`
pairs) is serial. No cost model is needed to remove a serial loop.

Two measurement defects also surfaced. The RRUNTIME load clock for the same
1M AKM input on one node reads 0.443/0.621/0.605/0.486 s at 1/4/8/16 workers;
a serial SPI transfer should not depend on the worker count, which points at
C1's overlapped factorization oversubscribing the affinity mask by one thread.
And `bench/campaign.py::physical_cpus` takes one CPU per physical core in
`/proc/cpuinfo` order without grouping by socket, so a four-worker fit on a
two-socket node can straddle sockets; one-to-four-worker speedups collapsed
for both tools on the October 12 host (fereg 1.19x, fixest 1.48x).

### H1. within locality sort and per-solve permutation (first)

Confirm, then fix, in this order. Steps a, b, d, e and f are
arithmetic-preserving; step c is the approved ulp-scope change.

1. Confirm (half a day, at most twelve fits). On the frozen 16M fixed64 input
   at 4 and 28 workers, run as-is and pre-sorted by the dominant factor
   (report which factor `within` selects and whether it is sorted). Sorting
   rows permutes the sample: coefficients must agree within replay tolerance
   and discrete decisions must be identical. Capture `time_setup`/`time_solve`
   and add trace-only clocks around `permute_obs_in`, `permute_obs_out` and
   `ObservationFrame::gather` in a diagnostic build. Count `Solver`
   constructions per fit (`BorrowedLsmrPrepared` already counts `builds` and
   `batches`). Controls: 1M fixed64 as-is and sorted, and 4M three-dense.
   A gap under 20% at 16M refutes the hypothesis: record it and proceed to H3
   (Johannes's revised handover order).
2. a. Parallelize `permute_obs_in`, `permute_obs_out` and the frame gather
   with chunked parallel iterators. Pure data movement, no reduction.
3. b. Parallelize the other serial per-RHS passes listed above that carry no
   reduction (finite check, `weighted_rhs`, subtractions, concatenation).
   `vec_norm` is a reduction: leave it serial or move it under the H1c
   declaration with fixed leaves.
4. c. Skip the locality sort when it cannot change the scatter kernel: sort
   only when the dominant term has at least `SCATTER_LOCAL_THRESHOLD`
   coefficients. The reproducible (`StableAdjoint`) path builds its own
   per-level row index and saves that index for sorted terms, so weigh the
   permutation cost against that saving with the same threshold rather than a
   separate rule. The row order seen by the `Fold` kernel changes, so
   per-level summation order changes:
   declared ulp scope on the projected columns, FE coefficients and
   everything downstream, with the existing independent certificates and
   replay witnesses, all discrete decisions exact. Express the rule in
   coefficients, never in row counts. Johannes approved this change on
   2026-10-12.
5. d. Build the `Solver` once per fit on fereg's non-deterministic narrow
   path (`fe/lsmr_batch.rs::solve_batch` and the explicit `absorb_lsmr`
   route), as the prepared path already does; charge the retained bytes to
   the existing admission.
6. e. PPML three-FE (`ppml/categorical_lsmr.rs`) rebuilds `Solver::new` from
   effects whenever weights change, which re-sorts and re-permutes the frame.
   Add a `within` entry point that takes a prebuilt `Design` (or borrows one)
   so only the weight-dependent preconditioner is rebuilt.
7. f. The explicit route passes `Some(weights)` even for unit weights, so
   `within` multiplies by `sqrt(1.0)` on every operator application and runs
   `weighted_rhs` per RHS. Pass `None` when every weight is exactly 1.0, as
   the prepared route's `BorrowedMapWeights::Unit` already does. Exact.

`within` is a separate repository; follow its tests and conventions, then
integrate through a pinned revision bump in `crates/fereg-core/Cargo.toml`.
Measure on the 16M input at 4 and 28 workers, the 1M fixed64 and AKM controls,
and the twenty frozen standard workloads at the workstream boundary.

H1 confirmation declaration (2026-10-12, source `c8851b5c`): ten diagnostic
fits, without fit warm-ups or repetitions: as-is/sorted 1M and 16M fixed64
at 4/28 workers, followed by the frozen 4M three-dense control at 4/28 workers.
The 16M input hash is
`0cfe985f04173707bd0c04f906dc77296703cfff797072d7903a82004595750d`;
the fixed64 seed is 98317561. Stable sorting uses `g3` then original `obs_id`,
outside the command clock; the third term wins the 64-coefficient tie.
Sorting permutes the sample's order: coefficient/VCE agreement must meet
SPEC section 20, and sample, rank, omissions, DoF, singleton accounting and
discrete solver decisions must remain identical. Both arms use the same
affinity mask. Trace-only clocks and construction counters live in an isolated
diagnostic copy of pinned within `297f32d9`; no production change is made.
Each process is capped at 120 seconds, the panel at 2,400 seconds and the
confirmation at half a day/twelve fits. The two unallocated fits do not
authorize repeated sampling. Compare `(as-is projection - sorted projection)
/ as-is projection` separately at both worker counts; mixed results are
reported before implementation. Build/loader checks are separate from these
ten frozen-input fits. Raw evidence stays on SCC; the compact result will be
`bench/results/h-20261013-h1.json`.

H1 decision (2026-10-12): stop after confirmation and proceed to H3. All ten
fits complete and their scientific comparisons pass, but sorting increases
16M projection time by 5.51%/1.47% at 4/28 workers; both reductions are below
the required 20%. The 1M reductions are 9.18%/12.27%. Two Solver constructions
and two batches occur per fit, with no retained prepared-session trace.
No phase-2 step, within change or pin bump is implemented, and the two unused
fits are not spent. See [results](VALIDATION.md#h1-locality-sort-confirmation-hypothesis-refuted-2026-10-12)
and [compact evidence](../bench/results/h-20261013-h1.json). The original
cost explanation above is the tested hypothesis, not an established cause.
Source review also finds that `Solver::new` already accepts an owned prebuilt
`Design`, explicit categorical OLS already uses a retained session, and PPML
refreshes Schwarz only at its existing refresh points rather than every
weight change. Those facts would constrain any later H1 proposal; they do
not authorize one after this stopping rule.

### H2. Dense three-FE route prototype (approved 2026-10-12)

Johannes approved a bounded prototype: an alternating-projection (MAP) attempt
for designs whose centred pair-incidence Frobenius contraction bound (the F2
geometry diagnostic, 21 ms, bounds 1.9e-16 and 1.8e-17 on the frozen dense
cases, vacuous on sparse) certifies fast convergence, with LSMR fallback under
the unchanged shared work budget and the existing final-residual certificate.
Before code: freeze the rule (bound threshold expressed as a predicted sweep
count, not a case list), the affected numerical fields (projected columns, FE
coefficients, coefficients, covariance, fit statistics; sample, rank,
omission and separation exact), the independent witnesses (explicit
projection oracle on synthetic designs, fereg-oracle dense comparisons, the
retained F panel inputs) and the budget (dense 1M/4M and sparse 1M at 1/4/16
workers, two pairs each, 120 s per fit, 2,400 s total, no retry). Sparse must
stay on LSMR and must not pay more than the geometry's 21 ms. Expected gain
from the F panel: dense 4M at one worker 16.803 to 12.919 s, at sixteen
4.814 to 3.519 s. That alone does not reach fixest's 4.224 s at four
workers; report what remains after the prototype so the next decision is
informed.

H2 implementation declaration (2026-10-12, baseline `a05ae5d7`, before code):
use the retained F2 centred pair-incidence bound for ordinary automatic OLS
with three categorical intercept effects, unit weights and symmetric sweeps.
Preserve original effect order. For each pair, compute the checked-integer
sum of `(N*n_ab - n_a*n_b)^2`, divide by `N^2*min(n_a)*min(n_b)`, and take
an outward-rounded square root capped at one. All three pairs remove only the
same global constant; additional common directions stay in the bound. There
is no row-count/case fence, empirical cost selector or weighted extension.
Checked arithmetic overflow or unavailable optional scratch declines the new
attempt and leaves the existing solver path intact.

With pair bounds `a=mu12`, `b=mu13`, `c=mu23`, define `u=a+bc+ac^2`,
`v=b+ac`, and `B=[[au,av],[c^3+bu,c^2+bv]]`. Starting at `(1,1)`, propagate
four powers with outward rounding. Predict the first complete symmetric
`0,1,2,1,0` sweep whose maximum block-moment bound is at most
`direct_lsmr_tolerance(request)`. Admit only predictions of one through four
sweeps and attempt at most the existing first four-sweep checkpoint. Four is
the existing checkpoint cadence, not a fitted campaign threshold. The bound
is an exact-arithmetic eligibility witness, not a floating-point convergence
guarantee. Cache the immutable bounds or decline once per OLS session,
including low-memory batches; never scan sparse geometry again for final
residuals. Explicit requests, PPML and the final-residual projection remain
on their existing paths.

The new MAP attempt uses the H8 target to predict its work, but its numerical
certificate stays at the **requested** tolerance and is the sole acceptance
gate, following Johannes's direct handover. A passing candidate is accepted;
do not impose a second tightened certificate. On a certificate miss, release
the candidate and its coefficient/workspace state and let existing LSMR use
the untouched input. Debit actual pilot sweeps from its existing first-half
allowance, preserve the final MAP reserve, and report all consumed work.
Abstain before the attempt if four sweeps cannot leave an LSMR iteration.
Charge histogram capacity, candidate copy, MAP workspace, original-reference
norms, coefficient products and all live overlaps to the existing admission.
Optional pre-attempt allocation refusal leaves the original route; Break and
non-numerical fatal errors remain fatal. No failed geometry prediction or
missed recurrence target rejects a request-certified estimate.
The implementation records retained coefficients directly in a preallocated
canonical batch, so no conversion allocation follows a passing certificate;
its actual capacity is checked before sweeps. H2 does not hand solver norms
to callers: their established norm reduction remains intact. A missed pilot
with consumed sweeps is reported as fallback when LSMR certifies.

Affected numerical fields are projected columns, FE coefficients, coefficients,
covariance, fit statistics and downstream numerical outputs/diagnostics. The
successful ordinary three-FE route and associated work/preconditioner diagnostics
may intentionally change from LSMR to MAP; final acceptance remains certified.
Sample, rank, omission, separation, requested tolerances and all unaffected
results stay exact. The existing 73-case Mac replay remains an exact control:
its only unit-weight three-FE Auto case (`ols-12`) has a four-sweep bound near
9.74e-4 and declines at its 1e-12 recurrence target. Keep its input unchanged.

Independent witnesses: retained F2 exact contingency and rational projection
cases, supplemented by nonuniform marginals, extra pair intersections,
disconnected geometry, rounding boundaries and memory refusal; explicit
projection/dense OLS oracles on synthetic eligible and declined designs;
fereg-oracle dense/weak comparisons; actual-plugin replay; and frozen F-panel
inputs. Retain original science bounds and all failures. The campaign is
36 measured fits: frozen dense 1M/4M and sparse 1M, 1/4/16 workers, two
alternating pairs each, 120 seconds per process and 2,400 seconds total,
no retry. The campaign entrypoint's separate small initialization is outside
the estimate clock but inside each process limit. Sparse must remain on LSMR
and pay no more than the retained 21 ms geometry cost; this is a measured
overhead gate, never a clock-based runtime route. Compare the final 4M/four-
worker command cost with fixest's retained 4.224 seconds, identifying the
separate host/source limitations. The twenty quiet Mac workloads remain at
the H boundary. The ten-repair/one-working-day blocker rule remains in force.

H2 outcome (2026-10-12): the fixed 36-fit panel completed without retry,
but the prototype is **not adopted**. All six sparse geometry clocks are
29.785–30.855 ms, above the 21 ms requirement; sparse otherwise stays on LSMR
with exact captured science and unchanged solver signatures. Do not reinterpret
that cost as a pass or spend another fit under this declaration. The checked
[source patch](../bench/results/h-20261013-h2.patch) against `a05ae5d7` retains
the implementation and witnesses; production remains the parent artifact.
All eighteen paired science/discrete comparisons and fixed-worker repeats pass.
Dense 4M native means improve 22.806 → 16.315, 7.407 → 5.981 and 4.372 → 3.712 s
at 1/4/16 workers. Dense 1M changes are -1.70%/-3.52%/+3.05%; geometry itself
costs 37.7–40.1 ms at 1M and 169.2–182.1 ms at 4M. Command time at 4M/four
workers is 7.244 s, still 3.020 s above the retained fixest 4.224 s on another
host/source. All costs and the failed gate remain in
[validation](VALIDATION.md#h2-dense-three-fe-prototype-sparse-cost-gate-failed-2026-10-12)
and the [compact record](../bench/results/h-20261013-h2.json). The 73-case exact
replay, 292 worker checks, full suite, independent projection witnesses,
retained F2 bounds and fourteen licensed fixtures pass on the prototype.
A new candidate or additional measurement budget requires a new declaration;
H4 continues on the accepted production runtime.

### H2 follow-up: reduce geometry cost (authorized 2026-10-12)

Johannes reopened H2 after reviewing its dense gains and sparse-cost failure.
The original 21 ms outcome remains failed. This new workstream starts from
`be357256`, ports the retained prototype onto completed H6 ownership, and
keeps the same mathematical eligibility, requested-tolerance certificate,
untouched-input fallback, work budget, original FE order and memory admission.
Its numerical scope and independent witnesses are those declared for H2 above;
explicit/weighted requests, PPML, final residuals, sample, rank, omission and
separation remain protected. Geometry-only changes must preserve the old
computed eligibility exactly at each requested target.

Before production edits, the bounded candidates are: (1) decline once the
same outward-rounded four-sweep calculation with unknown pair bounds set to
zero already fails all four checkpoints; (2) accumulate exact integer pair
moments once per occupied contingency cell during the existing histogram
clearing pass instead of once per row. Positive arithmetic monotonicity proves
(1) preserves the old eligibility; exact integer contingency identities prove
(2) preserves completed bounds and overflow behavior. Retain the original
prototype as a diagnostic comparator, independent rational/dense projection
witnesses, rounding/overflow/budget/Break cases and exact declined-route replay.
No new row-count, case, timing or empirical route fence is authorized.
An additional target-independent alternative to (1) is an exact marginal
lower bound: with Q_i=sum n_i^2 and M_i=max n_i, S=sum n_ij^2 >= N and
W=sum n_ij*n_i*n_j <= min(M_i*Q_j,M_j*Q_i). If checked arithmetic proves
N^3+Q_i*Q_j-2*N*min(M_i*Q_j,M_j*Q_i) >= N^2*min_i*min_j, the old capped
pair bound is exactly one. First prove 2*N^4 fits u128 so no original
intermediate could overflow; otherwise skip the optional shortcut. This avoids
a target-dependent cached decline, preserving later looser session targets.

The first candidate chooses the target-independent marginal shortcut and cell
aggregation. No partial-target cache is introduced. Independent high-effort
review found no production blocker; add explicit cancellation, consumed-pilot
norm/coefficient recovery and a 21,504-row parallel dummy-QR witness before
qualification. A task-only diagnostic compared both geometry implementations
on hash-verified frozen FE columns, five alternating pairs after an untimed
exact-bounds check. It used the optimized CI test profile with debug assertions;
those kernel observations are not release timings. The diagnostic source is
removed before the production freeze. The large quiet Mac panel uses the same
frozen Stata datasets and maintained campaign entrypoint, with native and command
clocks, platform plugin-name adjustment and tracing disabled. Its paired native
analysis uses the maintained bootstrap; independent science and exact repeats
are checked from exports outside the timed command.

Stop after at most three geometry candidates and ten substantive failed repairs
or one working day on a blocker. Targeted kernel profiling precedes the frozen
full-fit candidate. One SCC panel uses the frozen dense 1M/4M and sparse 1M
inputs at 1/4/16 workers, three alternating pairs per cell (54 fits), 120 seconds
per fit and 2,400 seconds total, without retries. Compare against current
production on the same allocated host and placement; preserve every outcome.
Report geometry separately and retain the historical 21 ms comparison, but do
not treat a clock from another host as proof of incremental end-to-end cost.

Adoption requires independent science and unchanged protected decisions, a
material dense gain and sparse/small-input nonregression under the existing
max(5%, 1 ms) allowance. Finish production and focused checks before quiet Mac
qualification: one fixed fifteen-pair panel on all twenty standard workloads,
and one fixed fifteen-pair panel on dense 1M/4M and sparse 1M at 1/4 workers,
after warmup, using the maintained paired-median 95% bootstrap and 3,600 seconds
per panel. No continuation or sampling until a pass. All cross-plugin numerical
differences require the declared scope and independent witnesses; fixed-worker
repeats remain exact. Qualification is incremental to `be357256`; the previous
H-wide inconclusive cell remains historical and is not retroactively passed.
A failed/inconclusive qualification preserves the candidate and evidence without
claiming adoption or a release speedup. Record results in VALIDATION and the
benchmark generalization section, compact records below 300 KB, raw captures
on SCC, and delete `target/tmp/h2-followup/` at closure.

### H2 follow-up candidate 2: certify at the predicted sweep

The first candidate's fixed panels are closed. All science/repeat checks passed.
The standard Mac panel has nineteen passes and one inconclusive cell (129-column
OLS at four workers, ratio 1.02043, interval [0.97225, 1.05460]). The large panel
finds dense 4M native gains of 25.97%/23.17% at 1/4 workers, but dense 1M at four
workers regresses 6.85% (interval [1.05489, 1.07918]); one-worker 1M costs 1.48%.
Sparse 1M costs 0.01%/0.39%, within the allowance. This candidate is not adopted.
All observations and the unchanged first candidate remain retained separately.
SCC's 54-fit panel passes science; sparse geometry is 15.52–16.10 ms, below the
old 21 ms number on this different host. That does not rewrite the old failure.

Before the second candidate's edits: retain complete geometry and its eligibility
bits, but pass its predicted sweep for the current recurrence target to the
automatic pilot. Add a requested-tolerance certificate at that sweep, alongside
the initial, ordinary cadence and fourth-sweep checks, deduplicating coincidences.
A finite certificate miss at the prediction checkpoint continues normally;
existing InvalidData fallback and other error propagation remain. The numerical scope includes
successful automatic three-FE results and iteration diagnostics from earlier
acceptance. Explicit MAP behavior, requested certificate, four-sweep maximum,
actual-work debit, untouched-input fallback, final projection, memory admission
and every protected discrete decision remain unchanged. SPEC and the public
Stata help expose no exclusive checkpoint cadence promise. Independent Astra
analysis endorses this concrete saved-sweep hypothesis, conditional on witnesses;
it is not a new acceptance rule or a resampling of candidate 1.

Witnesses before timing: independent dummy-QR and projection tests, retained FE
reconstruction, fixed-worker repeats and stable acceptance/iteration decisions at
1/4/16 workers, failed predicted checkpoint followed by fourth-sweep acceptance,
and four-sweep failure followed by fresh LSMR norms/coefficients and correct work
accounting. Run the full suite, universal native checks, 73-case exact protected
replay/292 worker checks and the 26 mapped licensed fixtures again. Bound targeted
profiling to dense 1M/4M at four workers, three first/second-candidate pairs after
warmup (16 fits, 300 seconds); finish source, focused checks and this diagnostic
before qualification. Retain an unsuccessful extra-certificate cost witness too.

Qualification is frozen before measurements: a new SCC 36-fit panel (two pairs
on the same three inputs at 1/4/16 workers, 120 seconds/fit, 1,800 seconds total)
and a new Mac large panel (same six cells, fifteen pairs after warmup, 3,600-second
budget). For the twenty standard Mac workloads, use two fixed fifteen-pair blocks
with warmup in each, regardless of intermediate outcomes: thirty pairs per cell,
1,800 seconds/block, 3,600 seconds total. Combine all pairs once using the same
paired-median 95% bootstrap and max(5%, 1 ms) allowance; neither individual block
is an adoption decision. This larger fixed sample addresses the observed wide
cell uncertainty on the new artifact. No continuation, favorable-cell selection,
or further sampling is authorized. Candidate 1's observations remain its own
failed/inconclusive outcome. All other adoption requirements above remain.

Candidate 2 result (2026-10-12): adopted after the declared Mac panels.
All twenty combined thirty-pair standard cells and all six fifteen-pair large
cells pass. Dense 1M native gains are 13.05%/6.46% at 1/4 workers; dense 4M gains
are 37.38%/32.31%. Full-command gains are 12.21%/5.72% and 35.80%/29.34%.
Sparse costs 0.02%/0.48%; standard wide64/four workers costs 1.82%, all within
the unchanged allowance. The candidate, source and inputs stayed frozen.
No continuation, fit retry or dropped observation occurred.

The full driver (1,746 Rust executions), 73-case exact replay/292 worker checks,
universal Mac build/ABI, 26 licensed fixtures, independent witnesses and
high-effort review pass. Fourteen Rust executions are ignored and thirteen
optional Python checks skipped. SCC build/ABI/focused tests and all 36 fits, eighteen paired
science checks and fixed-worker repeat checks pass. SCC dense gains support the mechanism, but its
sixteen-worker sparse timing costs 15.08% in two pairs and remains unresolved;
no Linux performance claim follows. The saved-sweep profile and artificial
failed-checkpoint cost are preserved. Candidate 1 and original H2 failures
remain distinct, as does the prior H-wide inconclusive outcome. See the
[full validation](VALIDATION.md#h2-follow-up-certified-dense-three-fe-gains-2026-10-12),
[first candidate](../bench/results/h2-followup-20261012-first.json) and
[adopted candidate](../bench/results/h2-followup-20261012-second.json).

### H3. Parallel neighbour-pair assembly for canonical products

In `ols/canonical_products.rs::build`, the explicit geometry accumulates each
unordered right-level pair `(h, j)` of every left level `g` into four cells of
a symmetric component matrix, serially, in `g` order. Parallelize while
preserving each cell's summation order:

1. Partition target rows `h` across lanes. Each lane runs the same `g` loop
   and the same `touched`/`totals` scan (O(N) integer work per lane) but only
   emits the pairs whose owning cell lies in its range. Off-diagonal cells
   `(h, j)` with `h < j` belong to `h`'s lane; diagonal cells `(h, h)` are
   accumulated by `h`'s lane by visiting pairs `(i, h)` for `i < h` and then
   `(h, j)` for `j > h`, in the serial loop's order. Each pair is evaluated at
   most three times; per-cell order and bits are unchanged. Mirror the lower
   triangle after assembly as the serial code does.
2. If the redundant evaluation costs more than it saves at low worker counts,
   the alternative is per-lane private accumulators reduced in fixed leaf
   order. That reassociates and is a separate declared ulp-scope item.

The union-find and weight/mass loops in the same function are serial O(N)
and secondary. This applies to every canonical call in a PPML fit, ordinary
frames and strict, and to OLS canonical products. Expected from the A3 panel:
dense two-FE PPML 4M at sixteen workers from 18.4 s toward the forced-implicit
10.9 s, and about 1 s at 1M. The stopped A3 cost-model calibration stays
stopped; this item needs no selector.

H3 implementation declaration (2026-10-12, baseline `ac8aa81f`): preserve
every scientific bit and solver signature. Own contiguous component-major
matrix rows per lane; visit eliminated groups and accumulated neighbour lists
in the original order. An owned diagonal receives earlier endpoints in order,
then later endpoints in order; its upper off-diagonal cells receive the same
ordered subtractions. Mirror upper to lower only after assembly. Each pair is
evaluated twice. The current serial code actually updates both triangles
directly; the mirror is justified by those identical ordered contributions.
Extra per-lane totals, touched lists and descriptors use only spare bytes in
the existing admitted phase peak. Insufficient scratch or a failed optional
allocation retains serial assembly, with unchanged dense/implicit selection,
center retention, output mode and estimator acceptance. One worker keeps the
original serial loop. Duplicate scans include floating additions and sorting,
so their cost is measured rather than described as integer-only work.

Before timing, require bitwise matrix equality against the retained serial
assembly on the retained A3 kernel geometries and frozen dense 1M/4M inputs,
plus the existing weighted-contrast, disconnected, cancellation and full-fit
oracles. Exact plugin replay and `./scripts/run_tests.sh` remain required.
The diagnostic timing allocation is two alternating pairs for each dense
two-FE PPML 1M/4M cell at 1/4/16 workers: 24 fits, 120 seconds per fit,
2,400 seconds total, no measurement retry. Compare with the retained A3
current/implicit observations without pooling hosts. Stop within one working
day or ten substantive unsuccessful repairs. No A3 calibration, private
accumulator reduction, tolerance or model-semantic change is authorized by
this declaration. Final Mac wall-time qualification follows the stable
Workstream H candidate, not this diagnostic panel.

H3 decision (2026-10-12): retain the preserving row-owner implementation.
Raw matrix equality passes on the retained A3 geometries and all four matrices
from the frozen dense 1M/4M inputs before the 24-fit panel. Each pair is
evaluated twice; neighbour totals and sorting repeat per lane. The optional
scratch stays within spare capacity of the already admitted phase peak;
serial assembly remains the one-worker and allocation-limited fallback.
Every captured scientific value and timing-stripped solver signature is exact
in all twelve A/B pairs. Native paired reductions at four/sixteen workers are
7.62%/22.95% at 1M and 21.18%/40.45% at 4M. One-worker changes are −0.10%/0.82%,
below the allowance. The two arms' sampled process-tree RSS maxima differ by 3.22 MiB. No private accumulators, arithmetic change,
new geometry route, tolerance change or acceptance gate was introduced.
The full Mac runtime suite, exact and threaded plugin replay, 17 licensed
PPML fixtures, Linux native checks and independent review pass. The initial
test lint and clock-classification mistake remain recorded; no fit was retried.
See [validation](VALIDATION.md#h3-ordered-parallel-assembly-2026-10-12) and
[compact results](../bench/results/h-20261013-h3.json). A3 calibration stays
stopped. Quiet Mac nonregression remains due once at the complete H boundary.
Proceed to H8 with the approved certificate-only acceptance rule.

### H4. C1 overlap oversubscription (Stata-side A/B)

On one SCC node with the frozen 1M AKM OLS input, alternate overlap on and off
at 1, 4 and 16 workers through `bench/campaign_stata.do`, five pairs per
cell, same affinity mask for both arms; report RRUNTIME rows 18 to 21. If
load inflates with overlap at low worker counts, run the overlapped
factorization on `workers - 1` lanes so Stata's main thread keeps a core, or
gate the overlap on spare cores in the resolved affinity mask. General
resource rule, arithmetic-preserving. If load does not inflate, record that
the clock pattern was host noise and stop. One day cap.

H4 execution declaration (2026-10-12, parent `42c8248c`, before fits):
use one 28-core SCC allocation and the frozen campaign `ols-akm-baseline`
1M input. Five alternating on/off pairs at each of 1/4/16 workers give thirty
measured Stata fits; each process is capped at 120 seconds, the complete panel
at 900 seconds, with no timing retries. Both arms get the identical physical
core mask in each pair. Preserve all RRUNTIME rows 18–21, command time,
solver signatures, coefficients, covariance, sample and model decisions.
The diagnostic off artifact changes only `cancellable::while_loading` to
finish loading before starting the same cancellable factor preparation on the
same action team. It still builds/consumes the same hint; no default routing
flag or new numerical implementation is added. Both plugins pass native load
and boundary guards; the existing prepared-input numerical test runs on both.
Tracing is disabled for the thirty fits.

The four-worker load clock is the primary oversubscription check; one worker
is a negative control because overlap is already inapplicable, and sixteen
workers is the scaling control. Treat inflation as demonstrated for this bounded
check only if all five four-worker on-minus-off load differences are positive
and the paired-median increase exceeds max(5%, 1 ms). This is an experimental
decision rule, not an estimator routing fence or a release-performance claim.
If it holds, use the approved general spare-core/worker-budget policy and verify
its exact replay; otherwise retain the observations and stop this item without
a runtime change. Mixed evidence is reported as inconclusive/host variability,
without extra pairs. The one-day and substantive-repair caps remain in force.

H4 result (2026-10-12): all thirty fits completed without retries and all
fifteen on/off comparisons plus six fixed-worker repeat groups retain exact
captured science and solver signatures. Four-worker overlap-on load is lower
in all five pairs: means 299.695 versus 311.857 ms, paired-median difference
−15.219 ms (ratio 0.95107). The inflation rule fails; sixteen-worker load
also favors overlap, while the one-worker negative control is unchanged
within noise. Native on/off means at 1/4/16 workers are 2.257310/2.257538,
0.906805/0.900010 and 0.464803/0.463340 seconds; these small costs remain
visible. The earlier load-clock pattern is not reproduced and is attributed
to host variability for this bounded check. Keep production unchanged and
proceed to H5. Both diagnostic plugins pass native guards and the existing
prepared-input numerical/fallback test. Jobs 8035773/8035774 pass accounting;
the panel took 112.251 of 900 seconds. See
[validation](VALIDATION.md#h4-overlap-check-no-load-inflation-2026-10-12) and
[all observations](../bench/results/h-20261013-h4.json). Raw artifacts remain
on SCC under `/projectnb/welfgr/fereg/runs/h-20261013-h4`.

### H5. Campaign placement (tooling)

Group `physical_cpus()` by `physical id` so a fit's cores come from one
socket whenever `threads` fits in a socket, bind memory to that node
(`numactl --membind` or equivalent), apply the same mask to both arms, and
record `lscpu`, the mask and the THP setting in the observations. Rerun only
the AKM OLS and PPML scaling cells at 1/4/8/16 workers on one node and report
the new speedups for both tools. No runtime change.

H5 execution declaration (2026-10-12, parent `67cb34ce`, before fits): group
allowed physical cores by socket, largest allowed socket first (socket-ID tie
break), so every prefix that fits in any socket uses one socket. Exclude SMT
duplicates, retain the existing whole-node requirement, and use `numactl` for
both physical affinity and memory binding to the selected NUMA node(s). Both
tools get the same mask/node policy. Missing topology or failed binding is a
tooling error with no unbound fallback. Observations include mask, sockets,
memory nodes, `lscpu` and THP settings; no scientific inputs or policies change.

Use the frozen October 12 AKM 1M OLS/iid and PPML/robust datasets, the accepted
H8 runtime (production identity through H4) and pinned fixest 0.14.2. On one
28-physical-core SCC node, run three alternating pairs at 1/4/8/16 workers:
48 fits, 120 seconds per process and 1,800 seconds total, no timing retries.
Before timing, run placement/tooling checks and one 1,200-row single-FE
native/reference pair for each model with the existing explicit-design oracle;
smoke failures block the panel. Reuse the validated unchanged Linux plugin.
Retain every outcome, scientific eligibility, fixed-worker repeatability,
cross-worker agreement and both tools' speedups relative to their one-worker
medians. These are descriptive observations, not a new Mac release claim.

H5 result (2026-10-12): integrated the socket-grouped physical-core selection
and explicit CPU/NUMA binding. All forty-eight fits complete without retries;
all twenty-four paired comparisons are eligible, all sixteen three-repeat
groups are exact, and both tools agree scientifically across workers. On
`scc-tj3`, fourteen cores per socket, 1/4/8 workers use socket/node 0 and
sixteen uses fourteen cores there plus two on socket/node 1. Relative to one
worker, OLS speedups at 4/8/16 are 2.083/2.851/3.055 for fereg and
1.937/2.427/2.762 for fixest; PPML gives 2.054/2.554/2.441 and
2.381/3.263/3.923. Native PPML is 4.64% slower at sixteen than eight workers;
that residual scaling cost remains visible. This is a same-node descriptive
scaling panel, not a controlled old/new placement comparison. The four
independent small smoke fits, actual child affinity/memory-policy check,
42 SCC harness tests, four placement tests and full Mac repository suite pass.
Production tokens and artifact inputs equal `67cb34ce`; no estimator behavior
changes. See [validation](VALIDATION.md#h5-socket-and-numa-placement-2026-10-12)
and [compact results](../bench/results/h-20261013-h5.json). Proceed to H6.

### H6. Data preparation and Stata-side passes

An inventory of every O(N) pass on the default OLS Fast path (three
intercept effects, four controls, no outputs) was taken on 2026-10-12 by
reading `stata/`, `plugin/stata_bridge.c`, `crates/fereg-plugin/src/estimate.rs`,
`numeric_pipeline.rs` and the `fereg-core` preparation and fit modules. The
Stata side runs one `marksample` pass and one `e(sample)` record compaction;
the ado is not where the Stata-side seconds go. The serial cost at load is
the per-element transfer: `fereg_spi_vdata_rows` issues one `_stata_->vdata`
call per element on Stata's main thread (8N calls here), with one worker
transposing tiles and the preparation team running factorization alongside
(C1). That transfer cannot be parallelized, which is why H4 matters.

Redundant or serial passes after loading, each a pure predicate or copy and
therefore arithmetic-preserving to remove or chunk:

1. `PreparedOlsLoadHint::into_matching_sources` (`ols/prepared.rs`)
   re-validates the codes the hint itself produced in the same call: a serial
   identity scan of the row map plus one serial pass per FE comparing codes
   with canonical values. Skip when the hint is consumed by its producer.
2. `validate_numeric_sample` (`prepared_categorical_data.rs`) runs about
   five serial passes (outcome finiteness, regressor finiteness, drop-reason
   count, row-map monotonicity and bounds) that duplicate the parallel
   `is_full_retained_unweighted_sample` scan on an identity sample.
   `numeric_sources_are_finite` (`prepared_rhs.rs`) is serial below eight
   regressors; partition by rows instead of by column.
3. The same immutable codes and membership are validated three times by
   `ValidatedBorrowedInterceptSource::new`: at topology transfer, at
   `ValidatedFinalizedTopology::new`, and again in `topology_dof.rs` for the
   absorbed DoF. Validate once and carry a receipt.
4. `project_intercepts` (`fe/borrowed_map.rs`) rescans the N x K bank for
   finiteness serially at solver entry, after the parallel preparation scan
   and the bitwise RHS comparison already covered it.
5. `take_full_retained_unweighted_sample` materializes an identity row map
   (8N bytes) and a zeroed drop-reason vector serially although an identity
   axis type exists; `invalid_shadow_observations` then re-checks the
   identity in parallel at publication.
6. Serial zero-fills that the next parallel pass overwrites: the membership
   `observations.resize(N, 0)` in `parallel_level_index`, and the candidate
   marker fill whose only consumer is one `.all()`.
7. The three-FE LSMR result carries no residual norms (`attach_ols_norms` in
   `fe/session.rs` serves only general and categorical bindings), so
   `dense_finite_inputs` recomputes six within norms, one serial N pass per
   column. Attach norms from the solve or certificate pass with the same
   kernel so the bits are unchanged.

Serial compensated reductions remain in the post-solve path: the normalized
intercept mean, the intercept score certificate, TSS and RSS (about six serial
N passes), and three serial union-finds for the pairwise mobility DoF. Chunking
the reductions reassociates and belongs to a ulp-scope declaration; the
union-finds are integer work and exact under any order but need a parallel
structure, so they are deferred unless the 16M profile shows them.

Memory: `y` and the controls are held twice (column bank and row-major RHS),
and the FE f64 columns stay allocated after factorization alongside their u32
codes and usize membership. Freeing the FE f64 columns once validated saves
24N bytes at no arithmetic cost. At 16M rows a serial sequential pass costs
roughly 20 to 40 ms, so items 1 to 7 together are on the order of half a
second to a second, general and cheap rather than decisive. Take them after
H1 and H3, each as a small preserving commit with exact replay.

H6 execution declaration (2026-10-12, parent `96a400c5`): take each independent
pass as its own arithmetic-preserving commit. Begin with item 2's complete
sample predicates and row-partitioned numeric-source finiteness, using the
existing installed-team `StagePlan` policy. A complete unweighted identity
sample may pass one chunked check of values, row identities and retained flags;
on a failed shortcut, retain the established validator and diagnostic order.
No numerical reduction, solver signature, certificate, sample, rank, omission
or separation policy changes. Witnesses are malformed/boundary sample tests,
full runtime checks, actual-plugin exact replay and the mapped preparation /
transaction fixtures; wall-time nonregression stays at the complete H boundary.
Subsequent items remain separate commits. Item 1's public hint currently borrows
mutable caller inputs: independent design review requires an owned OLS input
capsule across loading/classification before removing its code/value checks.
Independently supplied public hints and PPML's borrowed reuse retain validation.

Item 2 result: the installed-team row predicate replaces the complete sample's
serial validator scans and partitions source finiteness by rows at any model
width. Declined shortcuts still use the original ordered validator. Three
focused boundary tests, `run_tests.sh` (1,680 Rust executions), universal Mac
ABI/load checks, all 73 exact replay cases, 292 worker checks and ten licensed
preparation/transaction fixtures pass. No scientific bits or solver signatures
change. The shortcut adds a probe before the original validator on malformed
inputs; no wall-time improvement is claimed before the H boundary. Core runtime
grows by 32 lines and loses one dispatch condition. See
[validation](VALIDATION.md#h6-sample-scans-2026-10-12) and the
[compact evidence](../bench/results/h-20261013-h6-sample-scans.json).

Item 3 declaration (parent `eff56a47`): reuse `ValidatedFinalizedTopology`
through absorbed DoF, checking owner identity before bypassing raw validation.
The consuming single-source transfer constructor seals only the exact buffers
already checked during transfer. Public raw topology adapters, metadata,
cluster and slope checks retain their validation. This removes repeated
membership predicates without changing arithmetic, solver signatures or any
scientific decision. Witnesses include checked/owned DoF comparisons for
multiple effects, interactions, nesting and slopes; foreign-owner rejection;
transfer ordering, alias and allocation failures; full runtime checks, exact
plugin replay and mapped licensed FE/DoF/transaction fixtures. Wall timing stays
at the complete H boundary.

Item 3 result: transferred single-source membership is checked once and sealed;
ordinary and categorical DoF reuse that exact owner with a foreign-owner guard.
Rebuilt topology retains validation at the original finish point after source
retention and rebase. Independent review rejected an earlier implementation
that moved interaction scratch ahead of candidate-buffer release; the repaired
ownership state lives in the existing preparation-topology module. All 87
focused topology tests, `run_tests.sh` (1,680 Rust executions), universal Mac
checks, 73 exact replay cases, 292 worker checks and seventeen licensed fixtures
pass. Core production grows 24 lines; dispatch and size-violation counts remain
unchanged. No arithmetic, signature, certificate or model decision changes.
The initial size-ratchet and stale-import failures are retained. See
[validation](VALIDATION.md#h6-membership-validation-reuse-2026-10-12) and
[compact evidence](../bench/results/h-20261013-h6-membership.json).

Item 4 declaration (parent `bebc850d`): carry the existing checked
`PreparedOlsRhs` owner through ordinary dense initial OLS projection. Only the
structural automatic and reusable categorical entry checks may consume that
proof to skip a repeated finite scan; generic matrices, categorical generic
builders, final residuals, PPML and other backends keep their checks. Preserve
configuration/dimension/value/weight error order and all arithmetic, solver
signatures, requested tolerances, certificates and discrete model decisions.
No mutable RHS access, detached receipt, new allocation or sample-size fence is
introduced. Witnesses compare checked and raw projection for zero through three
effects, weights and loaded-row reuse, malformed axes/values, unchanged final
and PPML projections, full runtime checks, exact replay and mapped licensed
fixtures. Wall timing remains at the complete H boundary.

Item 4 result: the immutable RHS owner now survives initial ordinary OLS
projection, avoiding the repeated finite predicate in structural automatic and
reusable categorical entrypoints. Raw, generic, final-residual and PPML paths
retain their checks. Twelve focused RHS tests include 32 checked/raw projection
comparisons with exact residuals, norms, coefficients and clock-normalized
reports. The full suite passes 1,682 Rust executions; universal Mac checks,
73 exact replay cases, 292 worker checks and seventeen licensed fixtures pass.
Independent review approves the ownership and unchanged error ordering. Core
production grows 52 lines, with unchanged dispatch and size-violation counts;
there is no new allocation or numerical change. Two compile corrections and
a Python-version inventory failure remain recorded. No timing gain is claimed
before the H boundary. See
[validation](VALIDATION.md#h6-checked-initial-rhs-2026-10-12) and
[compact evidence](../bench/results/h-20261013-h6-finite-inputs.json).

Item 1 declaration (parent `1b0922f1`): keep the early OLS categorical columns
inside an owned capsule through optional factorization and authoritative
preparation. Core alone restores their vacated slots and consumes the capsule;
reuse without code/value revalidation requires the actual complete unweighted
sample-transfer branch and matching requested categorical source identities.
Classified inputs, independently supplied borrowed hints and PPML retain their
existing checks. Preserve metadata, loaded-input, weight, availability and
categorical-domain error ordering, SPI/Break authority and all numerical/model
decisions. Move the raw buffers without copying; admit descriptor storage
before the first SPI read and account for its lifetime beside tiles, factors
and moment preparation. Optional refusal before reading keeps the existing
loader; after reading, raw ownership must survive optional factorization failure.
Witnesses compare capsule/plain preparation including missing values, weights,
clusters and singleton pruning; changed requests and candidate masks; pointer
and capacity ownership; allocation refusal, completeness errors, late SPI
failure and Break; unchanged borrowed-hint/PPML tests; full runtime checks,
exact replay and licensed transactions. Timing remains at the H boundary.

Item 1 result: early OLS columns now stay owned alongside their optional
encodings until core restoration and classification. Only the actual complete
unweighted transfer skips self-revalidation; weighted/classified cases retain
the old checked reuse, avoiding an unnecessary refactorization in the first
draft. Three focused core tests cover 104 capsule/plain comparisons, signed
zero, changed requests, malformed slots and raw/encoding pointer identity.
Plugin witnesses cover early/late completeness contradictions, optional
refusal, late SPI error/panic, exact and one-byte-short budgets at 2/4/16
workers, and unchanged moment-cache decisions. The full suite passes 1,687
Rust executions; universal Mac checks, 73 exact replay cases, 292 worker
checks and seventeen licensed fixtures pass. Independent review approves.
The cost is a 56-byte descriptor per early source on this host, freed before
classification, plus 176 core and 99 plugin production lines. Raw observation
buffers are moved, never copied. The new-function size and test-only Clippy
failures were repaired and retained. No wall-time improvement is asserted.
See [validation](VALIDATION.md#h6-owned-load-hints-2026-10-12) and
[compact evidence](../bench/results/h-20261013-h6-load-hint.json).

Item 6a declaration (parent `d34359b5`): initialize the already reserved
membership observation bank on the installed team using the existing safe
indexed-collection helper, generalized to copied scalar values. Keep the same
fallible reservation, actual-capacity accounting, scatter destinations and row
order. This removes the serial initialization bottleneck, not the overwritten
writes: safe mutable scatter slices still require initialized elements, so the
bank receives the same 8N zero bytes and 8N scatter bytes on a 64-bit host.
No extra observation bank, unsafe conversion, arithmetic or solver decision is
introduced. Witnesses cover pointer/capacity preservation for f64 and usize,
exact serial/parallel membership and every optional reservation failure, full
runtime checks, exact plugin replay and licensed preparation transactions.
The candidate marker and identity/drop-reason representations remain separate
changes; timing stays at the complete H boundary.

Item 6a result: membership first-touch now uses installed-team indexed
collection into its existing fallibly reserved bank. Ten focused storage and
membership tests, the full suite (1,687 Rust executions), universal Mac checks,
73 exact replay cases, 292 worker checks and seventeen licensed fixtures pass.
Allocation identity/capacity, scatter order, scientific bits and solver
signatures are unchanged. Four core production lines are added; dispatch and
size-violation counts are unchanged. The 8N-byte zero writes remain before the
8N-byte scatter writes; no extra observation bank is introduced. No failed
repair or timing gain is claimed. See
[validation](VALIDATION.md#h6-membership-initialization-2026-10-12) and
[compact evidence](../bench/results/h-20261013-h6-initialization.json).

Item 5a declaration (parent `a3d37699`): reuse the existing `FinalizedAxis`
representation for the authoritative sample row map. Complete unweighted
preparation constructs an identity axis after its existing complete-sample
checks; classification and singleton compaction retain explicit selected rows.
Migrate consumers to logical by-value access without materializing identity
merely for validation, equality, hashing or publication. Consumers that truly
require an owned vector, including PPML's existing ownership boundary, convert
fallibly and consume selected storage without copying. Count actual capacities;
the implicit row axis owns no 8N-byte bank. Raw samples still undergo counts,
dimensions, bounds/order, values, weights and row/drop consistency validation.
Publication borrows the sealed prepared OLS owner rather than independent row
and drop slices, retaining dimensions, finite residuals, receipt/output checks
and atomic rollback while removing repeated membership validation. Arithmetic,
solver signatures and sample/rank/omission/separation decisions stay exact.
Witnesses compare implicit/explicit identities and malformed raw axes, excluded
rows, weights and singleton cascades; PPML separation and original-row errors;
publication failures and transactions; full runtime checks and exact plugin
replay. Eliminated allocations remove their own allocation failures; remaining
materialization stays fallible and admitted. Drop reasons and candidate markers
remain explicit in this slice. Timing remains at the complete H boundary.

Item 5a result: complete OLS preparation uses an implicit row identity,
removing its 8N-byte heap map on this host. Selected axes transfer without
copying; PPML explicitly retains its required row vector. Publication borrows
the sealed prepared owner and checks the independently produced residuals,
shapes and receipts. All 54 focused/mapped executions, Clippy, the full suite
(1,692 Rust executions), universal Mac checks, 73 exact replay cases,
292 worker checks and seventeen licensed fixtures pass. Independent review
approves the proof boundaries. Core grows 36 production lines and plugin
shrinks 27; size, dispatch and chunk counts are unchanged. Two migration
compile errors, two invalid PPML test fixtures, a test lint, an initial Python
version error and a stale inventory position are retained. No scientific
policy or signature changes; drop/candidate masks remain separate. See
[validation](VALIDATION.md#h6-identity-row-axis-2026-10-12) and
[compact evidence](../bench/results/h-20261013-h6-axis.json).

Item 5b declaration (parent `b2cf4d50`): represent a complete sample's uniform
Retained drop reasons without an N-byte heap bank. Introduce a logical repeated
or explicit value owner with reference access and exact sequence equality;
classified builders retain their existing reserved mutable vectors and wrap
them only after classification. Public repeated values are data, not proof:
raw shape, counts, numeric/weight checks and row/drop consistency remain
authoritative. No-drop compaction keeps the implicit owner; actual deletion
materializes it fallibly before mutation, with its full live capacity covered
by compaction admission. Preserve selected buffer ownership and existing
worst-case preparation/compaction forecasts. Keep PPML's mutable result/owner
ledgers explicit through consuming fallible conversion where needed; read-only
generation samples and sealed OLS publication use logical access. Do not add
allocating indexing or implicit slice conversions. All arithmetic, solver
signatures, requested tolerances, certificates and sample/rank/omission/
separation decisions remain exact. Witnesses cover repeated/explicit equality,
malformed raw masks, allocation/capacity and no-drop/deleting compaction,
missing/weight/singleton precedence, PPML rebase/rollback and publication,
full runtime checks and exact plugin replay. Candidate markers remain unchanged
in this commit; timing stays at the complete H boundary.

Item 5b result: complete OLS samples now keep Retained reasons as a repeated
scalar, removing the N-byte mask. Classified masks retain their vectors;
no-drop compaction remains implicit and actual deletion materializes under the
unchanged full compaction allowance. Production PPML initial/final ledgers
remain explicit. All 39 focused executions, Clippy, the full suite (1,698 Rust
executions), universal Mac checks, 73 exact plugin cases, 292 worker checks and
seventeen licensed fixtures pass. Independent review approves the ownership
and admission boundaries. Core adds 82 production lines and one logical-equality
condition; plugin, chunk and size-violation counts are unchanged. The lead's
inventory helper initially missed a Python import path; fixed before the
ratchet check. The collector also needed to handle interleaved nested-test
stdout; all successful invocations were recovered without rerunning tests.
No scientific or solver-signature
change and no timing claim. See
[validation](VALIDATION.md#h6-uniform-drop-reasons-2026-10-12) and
[compact evidence](../bench/results/h-20261013-h6-drop-mask.json).

Item 6b declaration (parent `71bb1d4d`): use the same repeated/explicit
owner for candidate markers. An all-candidate input keeps a repeated true
value with no N-byte allocation or fill. Add a concrete borrowed logical view
so classifiers, chunked SPI reads and PPML adjustments consume the same
representation without materialization or a second classification path.
Explicit markers retain their reservation point and SPI read/write order;
all-candidate chunked loading keeps its existing no-mask path. Repeated true
or false is raw data, never a validation receipt: dimensions, metadata,
source-use, weight-domain, factor and exclusion precedence remain unchanged.
Logical reads do not allocate. PPML stages its existing explicit original-axis
mask, swaps the complete owner and restores its variant, pointer and capacity
before propagating any returned error, alongside probability weights. Count
actual retained capacities but preserve loading, compaction and PPML future
mask forecasts. No numerical kernel, solver signature, requested tolerance,
certificate or sample/rank/omission/separation rule changes. Witnesses compare
repeated and explicit true/false markers, malformed and excluded inputs,
numeric/string scalar/batched SPI paths and errors, memory boundaries and
PPML generation restoration; full runtime checks, exact plugin replay and
licensed transactions remain required. Timing stays at the complete H boundary.

Item 6b result: all-candidate inputs keep a repeated true value, removing
the N-byte marker allocation/fill and allowing constant-time logical all/count
checks. Explicit masks retain SPI ordering and capacity; PPML restores the
entire original owner and weight bits before returning. Future-mask forecasts
stay unchanged. All 78 focused/mapped executions, Clippy, the full suite
(1,707 Rust executions), universal Mac checks, 73 exact plugin cases,
292 worker checks and seventeen licensed fixtures pass. Independent review
approves the borrowed-view, exclusion and restoration boundaries. Core adds
71 production lines and plugin 23; size-violation, dispatch and chunk counts
are unchanged. Two fixture-adapter compile failures and one incorrect all-false
PPML fixture expectation are retained; no production scientific repair.
No timing gain is claimed. See
[validation](VALIDATION.md#h6-uniform-candidate-markers-2026-10-12) and
[compact evidence](../bench/results/h-20261013-h6-candidate-mask.json).

Item 7 execution declaration (parent `2d5dc7dc`): fuse the LSMR residual
certificate's weighted data read with the caller's Fast OLS within-norm
recurrence. Keep two distinct states: the existing 512-row FE leaves merged
in their original order, and the uninterrupted row-order dense OLS scaled
sum. Neither consumer changes arithmetic. Use bounded stack column blocks
and the installed-team scheduling policy, without row-count or case fences.
Eligibility comes from the initial prepared OLS frame and matching normalized
weight scale; Low, PPML, raw frames, final residuals and continuations keep
their existing paths. Existing MAP norm handoff is unchanged.

Within the fresh LSMR certificate workspace, temporarily place dense norms in
`rhs_scales`. At the original scale check, retain the original RHS order and
`max(reference, residual)` expression, move each dense norm to the now-dead
`reference_norms` slot, and run the unchanged group certificate using the
original scales. Only after all original coefficient allocations and actual
capacity admissions may a passing result transfer this already-accounted bank.
No new heap bank or early mandatory admission is introduced; dense-only invalidity
disables the optional cache and preserves the caller's original error path.
Failed certificates discard it before the unchanged continuation. The requested
certificate, solver signatures, sample, rank, omission and separation stay exact.

Witnesses compare both states with independent retained recurrences at leaf
boundaries (511/512/513 rows), extreme/subnormal/zero values and nonuniform
weights, check invalid-input error precedence, both automatic and retained
LSMR with/without coefficients, certificate failure, and exact/short/overcapacity
memory admission. Full runtime, 73-case exact replay and mapped licensed checks
remain required. Before whole-H timing, one bounded diagnostic kernel panel
compares the original separate passes against the fused pass at 1M rows,
widths 1/6/17/65, workers 1/4/8 and unit/nonuniform positive weights. Matrix
cell i is `((i % 173) - 86) / 91`; nonuniform weights repeat
`[0.25, 0.5, 1, 7]` and normalize by 7. Original certificate scratch uses
`categorical_parallel_counts` at maximum FE level count 64 (the fixed64
family), including zero scratch at one worker. Use one warm-up pair and three
alternating measured pairs per cell, 300 seconds total, no retry.
Run this panel without competing work on its host and retain all cells, including
costs. This is targeted profiling, not a release performance claim. Whole-H
nonregression remains the single standard Mac panel after production edits.
The Mac host remained busy before any timing began, so this diagnostic uses
one isolated 28-core SCC node, with its eight-CPU mask and memory on one
socket for both implementations. Reserve four slots/4 GB per core/25 minutes
for build and a one-test smoke, then 28 slots/3 GB per core/10 minutes for the
single 300-second panel; no host pin or numerical retry. Record source and
binary hashes, placement, THP, logs and successful accounting for both jobs.
This host choice does not satisfy the final Mac timing requirement.

Item 7 result: initial Fast OLS automatic and retained LSMR can transfer
an already-accounted norm bank after the original coefficient/admission
checks. The fused data read retains both original recurrences; certificate
failure, dense-only invalidity, Low, PPML and raw/final frames preserve their
existing behavior. All 13 focused executions, the full suite (1,714 Rust
executions), universal Mac checks, 73 exact replay cases, 292 worker checks
and twenty licensed fixtures pass. Independent architecture review approves
the provenance, error-order and allocation boundaries. Seven fixture/migration
failures are retained, including incorrect test assumptions about automatic
MAP continuation; no production solver repair was made. Core adds 218
production lines and two dispatch sites; no new size violations.

The single declared diagnostic panel on an isolated SCC node completed 24 cells
and 72 measured pairs, checking both norm banks bitwise after every warm-up and measured pair.
Paired median fused/separate ratios range from 0.7406
(width 1, 8 workers, unit weights) to 1.1910
(width 65, 4 workers, unit weights). All cells and their three-pair ranges remain in the compact
record. Six columns at four workers cost 12.77%/12.13% more for unit/nonuniform
weights; 65 columns cost 19.10%/16.76% more at four workers and 14.98%/15.19%
more at eight. Those costs remain visible in this preserving candidate; there
is no claim that the fused kernel is faster for every shape. Three pairs
quantify observed variability only; they do not establish a release-performance
claim. Sharing a read changes the distribution of parallel work, and both arithmetic states still require their original
updates. Whole-fit nonregression remains the complete H boundary.

See [validation](VALIDATION.md#h6-dual-order-lsmr-norm-handoff-2026-10-12) and
[compact evidence](../bench/results/h-20261013-h6-norms.json).

FE numeric-column release declaration: release redundant finalized OLS FE
numeric columns only after singleton processing, reencoding, rebasing and
cluster certification. Original request roles and finalized bindings exclude
ordinary sources (including omitted terms), slopes, interactions and cluster
sources; PPML retains every numeric column. A generic sample bank separates
raw preparation from an opaque finalized bank whose slots remain either raw
vectors or privately proved encoded sources. Preserve every source index;
never use an empty numeric slice as an encoded placeholder or reconstruct the
released f64 column. Raw public validators retain their existing contract.

The private owned-OLS preparation boundary holds the original request and
checks exact canonical raw/code correspondence without allocating. Proof,
optional reservation or capacity-admission refusal retains Raw and the original
later diagnostics. After the unchanged moment decision and membership
admission, pass a finalized-owner ceiling: the smaller of the recorded
preparation peak and usable bytes minus metadata, actual loaded-RHS capacity
and the full admitted moment workspace. Checked arithmetic failure disables
release. Drop temporary cluster-source IDs before conversion; charge complete
Raw ownership plus requested, then actual, new slot capacity while all old
numeric buffers and their outer descriptors remain live. Final physical
ownership must not exceed the original owner. No new mandatory allocation,
error, estimate rejection or solver routing rule is introduced.

Retain the pre-release owner-byte scalar for the existing execution, output
and recovery admissions. Actual owned bytes remain available for observed
memory accounting. This deliberately preserves Fast/Low selection, coefficient
retention and solver budgets after releasing storage. All scientific bits,
solver signatures, arithmetic, tolerances, certificates, sample, rank, omission,
separation and publication decisions remain exact. Independent review and
focused witnesses cover raw pointer/capacity identity, exact savings, signed
zero, invalid correspondence and error order, exact/short/overcapacity
conversion admission, concurrent moments, singleton reencoding, omitted/shared
roles, clustered and saved-FE recovery, PPML retention, Fast/Low, finite-budget
routing and output admission. Full runtime, exact replay and mapped licensed
checks precede integration; whole-H timing remains the single final Mac panel.

FE numeric-column release result: owned OLS preparation can now drop
privately validated FE-exclusive f64 columns after final factorization. Source
indices remain intact, shared/omitted ordinary, slope, interaction and cluster
sources stay raw, and PPML retains every numeric column. Original owner bytes
still govern execution, output and recovery admission; observed physical bytes
reflect the release. This saves approximately 24N bytes for three exclusive FEs,
less source-descriptor expansion, at the cost of one parallel correspondence
pass per eligible column and a temporary descriptor bank. It does not create
a new route, refusal or acceptance rule.

Independent architecture review, 37 focused executions, the full suite
(1,726 Rust executions), universal Mac native checks, 73 exact replay cases,
292 worker checks and 25 licensed fixtures pass. All scientific results and
solver signatures remain exact. Seven unsuccessful migration/fixture attempts
are retained; one exposed a missed topology consumer that still required a raw
vector, now migrated to the sealed encoded-source receipt. Two early checks
did not validate the isolated candidate and are excluded from passing evidence.
Core adds 371 production lines and three dispatch sites; plugin adds 27
lines, with no new size violations. The whole-H quiet Mac panel remains pending.

See [validation](VALIDATION.md#h6-finalized-fe-numeric-column-release-2026-10-12)
and [compact evidence](../bench/results/h-20261013-h6-fe-columns.json).

### H7. Small-input fixed cost (check only)

The 100k cells lost ground in the October 12 rerun (AKM OLS paired ratio
1.954 to 1.270, AKM PPML 2.003 to 1.553) while fixest's times barely moved.
One bounded Stata-side A/B of the current plugin against the October 10
plugin on one host at 100k rows decides whether this is host noise or added
per-call cost from C1/C2/E1 at small N. Report; do not tune.

H7 execution declaration: after the final H6 source is frozen, compare its
Linux plugin with the retained October 10 `e29164e6` plugin (SHA-256
`57cc9789efd6ecc810d6dd82c55822dac3bd537bd7be32bb7a4bcd82e2a574a1`)
through `bench/campaign_stata.do`, with the same current frontend and frozen
100k AKM OLS/PPML inputs. Both use four physical cores and memory on one socket
of one isolated SCC node. Record placement, THP, environment and every plugin,
input and runtime identity. Export solver diagnostics only after stopping the
command clock; trace-only clocks remain disabled.

Use seven alternating measured pairs and one excluded warm-up pair per model:
28 measured fits and four warm-up fits. Four independent 1,200-row smoke fits
precede the panel; each fresh Stata process also performs its existing small
initialization fit, for 72 estimator invocations total. Bound each process at
120 seconds and the panel at 900 seconds, with no retry, continuation or tuning.
Reserve four slots/4 GB per core/25 minutes for setup and 28 slots on a 28-core
node/3 GB per core/20 minutes for the panel (11 slot-hours maximum). Capture
accepted job IDs and require successful scheduler accounting, native audits,
independent smoke checks, complete outputs and preserved runtime hashes.

Analyze paired median current/October-10 ratios with 10,000 fixed-seed
(739391) bootstrap resamples and a 95% interval. A lower bound above
`1 + max(5%, 1 ms / baseline median)` supports added cost; an upper bound
within that allowance supports nonregression; otherwise report inconclusive.
Retain every observation. Existing campaign tolerances govern continuous
results; sample, rank, omission, singleton, separation and DoF decisions stay
exact, as do each plugin's fixed-worker repeats. Report clock-stripped solver
signatures and H8 iteration changes. This two-arm comparison cannot isolate
all intervening changes or by itself prove the historical host-noise cause.

H7 result: the single declared paired comparison is complete. Four smoke
witnesses, all sixteen old/current science comparisons (including warm-ups)
and exact repeats for both plugins pass. The current/October-10 timings are:

| Cell | October 10 seconds | Current seconds | Paired ratio [95% interval] | Verdict |
| --- | ---: | ---: | --- | --- |
| OLS command | 0.169399 | 0.167542 | 0.9937 [0.8375, 1.0917] | inconclusive |
| OLS runtime total | 0.151418 | 0.149957 | 0.9990 [0.8194, 1.1022] | inconclusive |
| PPML command | 0.594051 | 0.501664 | 0.8453 [0.8321, 0.8506] | nonregression |
| PPML runtime total | 0.557795 | 0.466115 | 0.8357 [0.8195, 0.8439] | nonregression |

The PPML paired command ratio is 0.8453 (about 15.5% lower). OLS is
inconclusive: its 0.9937 point ratio and wide interval establish neither
added per-call cost nor historical host noise. H8 raises OLS recurrence
iterations from 13 to 16; PPML solver signatures are unchanged.
The record includes OLS preflight/load/native clocks, PPML phase clocks and
clock-stripped solver diagnostics. This comparison includes all intervening
runtime changes, including H8, so it cannot identify C1/C2/E1 independently or
prove the historical host-noise explanation. Costs and inconclusive cells are
retained. No tuning, retry or continuation was performed. See
[validation](VALIDATION.md#h7-small-input-fixed-cost-check-2026-10-12) and
[compact evidence](../bench/results/h-20261013-h7.json).

### H8. Consistent recurrence tolerances across automatic routes (approved 2026-10-12)

Facts. Explicit `solver(lsmr)` iterates to a hundredth of the requested
tolerance (floor 1e-15, never looser than the request); automatic LSMR uses
`direct_lsmr_tolerance` (a hundredth, floor 1e-14); CMG uses
`PCG_TOLERANCE_MULTIPLIER = 0.01`; PPML's inner schedule tightens to
`min(0.1 x tolerance, 1e-12)`. All of them certify the FE moments at the
requested tolerance. The two-FE automatic OLS route
(`fe/borrowed_map.rs::absorb_two_fe_pcg_reusing`) instead stops each
right-hand side when `two_fe_pcg_rhs_ready` sees its Schur group-moment proxy
below half the requested tolerance, then reconstructs and runs the same
certificate. The F1 record shows the consequence on a weakly connected
control: projected regressors accurate to 2.6e-6 under that route and to
3.3e-13 under LSMR, with the robust F statistic moving at the 1e-5 level
while coefficients still passed. Tightening the two-FE stop to 1e-10 did not
make those F witnesses pass; CMG and LSMR did. So the stop rule is an
inconsistency to remove, and the weak-graph accuracy is a separate question.

Johannes's decision on 2026-10-12: tolerances must be consistent across the
automatic routes, ulp-level and small result changes in the direction of
accuracy are accepted, and no change may reject or re-route estimates that
pass the certificate at the requested tolerance.

Rule to implement:

1. The two-FE automatic route aims its recurrence at a hundredth of the
   requested tolerance with the same 1e-14 floor as automatic LSMR, in its
   existing proxy metric, never looser than the request. Derive it from the
   same helper the other routes use so there is one definition.
2. The independent certificate at the requested tolerance remains the only
   acceptance gate. A right-hand side whose recurrence budget is exhausted
   while the certificate passes at the request is accepted and reported with
   a stop reason, not failed and not re-routed. MAP polishing and LSMR
   fallback trigger only when the certificate at the request fails, exactly
   as today. No new failure mode, no tighter certificate, no new fence.
3. PPML's scheduled inner tolerance is already at or below this target and
   is unchanged. The 1e-14 final residual projection for saved outputs is
   unchanged. Explicit user requests keep their current contracts.
4. The stronger question, whether the certificate should bound the projected
   regressors rather than the FE moments on badly conditioned graphs, is a
   design decision for Johannes, not part of this item. Keep the F1 barbell
   and weak controls as retained witnesses and say in `SPEC.md` and the user
   guide that automatic routes can differ at roughly the 1e-5 level in
   derived statistics on weakly connected graphs.

Evidence: result-changing on cases where the two-FE recurrence used to stop
early. Declared fields: projected columns, FE coefficients, coefficients,
covariance, fit statistics; sample, rank, omission, separation and all stop
decisions exact. Witnesses: the F1 independent QR and 80-digit algebra on the
retained barbell and weak controls, the existing replay with
`--affected-scope`, and the fereg-oracle two-FE comparisons. Measure the
campaign two-FE OLS cells (AKM 1M at 1/4/16 workers, dense 1M/4M, sparse 1M)
before and after; report the iteration and time cost. If the two-FE OLS
native time rises by more than ten percent on any campaign cell, report to
Johannes before integrating rather than weakening the rule. Bounded to two
working days.

H8 implementation declaration (2026-10-12, baseline `e725a515`): compute the
ordinary automatic OLS proxy target through `auto::direct_lsmr_tolerance`.
Pass the actual target to both PCG readiness checks; do not apply the old
half-tolerance multiplier again. Carry the OLS/final-residual context through
both the retained categorical owner and the stateless structural fallback.
Other callers, including PPML, keep their existing half-request proxy target;
the 1e-14 final projection therefore keeps its existing 5e-15 proxy. The
requested certificate, iteration budget, MAP reserve, initial certificate,
checkpoint continuation and LSMR fallback rules stay unchanged. At requests
below the helper's floor, its existing cap at the request remains authoritative
for ordinary OLS, as approved; the final-residual path remains separate.

A certificate-passing result continues to publish `Certified`. Changing that
code to `MaximumIterations` would make `SolveReport::certified()` reject the
result and is prohibited. Trace-only per-RHS diagnostics will identify proxy
completion, budget exhaustion or breakdown separately from final acceptance.
Inner iteration counts and numerical diagnostics are affected work/results;
final acceptance, solver/fallback identities, warnings and all model/discrete
decisions remain protected. The previously named numerical scope includes
projected columns, FE coefficients, coefficients, covariance, fit statistics
and their downstream saved numerical outputs. No certificate is tightened or
weakened, and a missed proxy target does not become an acceptance gate.

Before editing production, bind the Mac replay scope to the three ordinary
two-FE Auto inputs (`ols-09`, `ols-24`, `ols-27`), listing permitted numerical
leaves and inner-work counters; all other replay fields/cases remain exact.
Use the existing independent replay and two-FE projection oracles, plus the
retained F1 barbell/weak cases (seed 739391, 32,768 rows, 128/128 levels,
16 controls, analytic weights, robust VCE). Regenerated F1 request hashes must
equal `4024d3bd...`/`1c0cdf17...`. Recompute the full-dummy QR witness and
80-digit Wald/mixed-covariance algebra on exact binary64 inputs. The historical
strong-F failures remain visible; this is a tolerance-consistency change, not
an asserted repair of the weak-graph certificate. F1 is capped at six native
captures, 120 seconds per capture and 600 seconds total, without retries.

The cost panel uses frozen campaign two-FE OLS inputs: AKM 1M, dense 1M/4M
and sparse 1M, each at 1/4/16 workers, two alternating pairs per cell (48 fits),
120 seconds per process and 2,400 seconds total, without retries. Record native
time and iteration counts, preserving both pairs and any failures. Report any
cell exceeding a ten-percent native-time increase to Johannes before
integration; do not weaken the approved rule. Quiet standard Mac timing stays
at the H boundary. The two-working-day item cap and ten-repair/one-day blocker
stop rules remain in force.

H8 decision (2026-10-12): implement the approved consistency rule without
changing the requested certificate or any rejection/fallback gate. Mac full
suite, 73 exact parent replay cases, 292 worker checks, fourteen licensed OLS
fixtures and Linux build/focused checks pass. The 48-fit campaign completes
without retries. Sparse 1M native costs rise 13.31%/12.17%/10.95% at 1/4/16
workers, reported to Johannes before integration; main/final iterations change
92/11 → 120/0. AKM changes 14/0 → 18/0; dense remains 1/0. Keep these costs;
do not weaken the target or repeat the panel. All campaign paired science,
protected decisions and within-arm fixed-worker repeats pass. F1 QR ordinary
bounds pass, but the unchanged strong-F witness remains failed (barbell 2.480x,
weak 1.076x its bound). This is not a stronger weak-graph certificate.
See [results](VALIDATION.md#h8-recurrence-consistency-with-measured-sparse-cost-2026-10-12)
and [compact evidence](../bench/results/h-20261013-h8.json). Quiet standard Mac
timing stays at the whole H boundary. Proceed to the approved H2 prototype.

Final H boundary declaration: after all production edits and focused
qualification, compare the final H6 universal Mac plugin with the retained
pre-H3 baseline (SHA-256
`9ced317e7cb995510995490e76dd492b491ee7a830a7147884ac5b76c73e38c8`).
Regenerate and hash-check the twenty frozen standard workloads. First run
the 73-case plugin replay at 2/4/8/16 workers under the existing H8 affected
scope; H1 produced no runtime change and H2 was not integrated. Then run one
quiet Mac panel with seven alternating measured pairs after one excluded
warm-up pair per workload, 320 native calls total, 90 seconds per process and
1,800 seconds total. No continuation or retry is declared. Retain every
observation and the existing paired-median 95% bootstrap upper-bound rule
against max(5%, 1 ms). Each plugin must repeat its scientific results and
solver signature exactly; cross-plugin agreement uses the scoped replay.
Freeze plugins, harness and input hashes before timing and check them again
afterward. Pause measured-host agent/build/test work and coordinate unrelated
applications before the quiet window. Report failed or inconclusive cells
without tuning or replacing their observations.

Final H boundary prerequisites, retained before timing: the final H6 Mac
plugin passed the pre-H3 comparison on all 73 replay cases and 292 checks at
2/4/8/16 workers under the pinned H8 scope, with zero exact, intentional,
scientific, repeat or discrete differences. Twenty frozen timing inputs and
the reviewed launch/collection tools were preserved on SCC while host
coordination was pending. That historical prerequisite record remains intact;
the completed panel is reported in the final boundary result. See
[validation](VALIDATION.md#h-final-mac-boundary-prerequisites-2026-10-12) and
[compact evidence](../bench/results/h-20261013-boundary-prerequisites.json).

Final H boundary result: the single declared quiet Mac panel completed
all twenty workloads and 320 native calls in 643.298 seconds.
Nineteen workloads pass nonregression; one remains inconclusive. No cell
meets the declared criterion for a material regression. The whole-H timing
gate does not pass; no overall release-performance claim follows. All
observations are retained, with no retry, continuation, tuning or selected
resampling. The inconclusive cell is 129-column OLS at four workers: paired ratio
1.038577, 95% interval [0.981125, 1.059293] against allowance 1.05. It proves
neither material regression nor host noise. Activity Monitor was temporarily
paused with an automatic resume guard and restored after timing; indexing
settled before launch. Both plugins repeat scientific results and
clock-stripped solver signatures exactly on every timing workload.
The separate 73-case H8-scoped replay passes all 292 worker checks; all
unaffected fields and discrete decisions remain exact. See
[validation](VALIDATION.md#h-complete-workstream-mac-boundary-2026-10-12) and
[compact evidence](../bench/results/h-20261013-boundary.json).

### Evidence and stop rules for H

`./scripts/run_tests.sh` and `scripts/run_plugin_replay.sh` on every runtime
change; zero exact replay differences and unchanged solver signatures for
preserving steps; declared fields with independent witnesses for H1c and H2;
`scripts/check_complexity.py --compare-production REV`. Wall-time
nonregression on the twenty frozen Mac workloads once at the workstream
boundary. Results to `bench/results/h-20261013-*.json` under 300 KB each,
prose to `VALIDATION.md` and `fixest_benchmarks.md`, decisions here. Task
outputs under `target/tmp/h-<item>/`, deleted before reporting. Phase caps
are hard; after ten substantive unsuccessful repairs or one working day on
one blocker, record the reproducer and ask Johannes.

## 7. H9 proposal: reuse preconditioners at exact current weights

Johannes requested this action plan on 2026-10-12. This section proposes
implementation and experiments; it records no runtime approval or performance
result. Start from integrated source `b4b4f6d3` (or identify its successor before
work), which includes H3, H8, H6 and the accepted H2 follow-up. H3's measurement
prerequisite is complete. Keep the earlier H boundary's inconclusive cell and
the subsequent H2 incremental qualification distinct; neither qualifies H9.

The objective is to avoid repeated neighbour-pair assembly while solving the
actual current weighted problem. First target the strict checkpoint, then
ordinary canonical frames if the first stage saves total cost. The strict
solve remains fresh, with SPEC's `min(1e-12, 0.1*outer_tolerance)` target,
the existing tighter reduced recurrence target and original-RHS stopping
reference. Final Hessian/bread, inference and both bias modes still use the
exact accepted `h=q*mu`. No tolerance or model-semantic change is proposed.

### Correct the premise before estimating savings

- H3 already reduced dense 4M PPML native time from 19.540 to 11.636 seconds
  at sixteen workers in its SCC panel. The earlier A3 forced-implicit result
  was 10.896 seconds on a different source/host/harness. These are separate
  observations, not a remaining 7.9-second saving or a new paired comparison.
  Use the [H3 evidence](VALIDATION.md#h3-ordered-parallel-assembly-2026-10-12)
  and [A3 discovery](VALIDATION.md#a2a3f2-first-diagnostic-panels-2026-10-11)
  to choose measurements, not to promise that H9 flips the dense cell.
- `ols/canonical_products.rs` currently drops weighted geometry before
  returning `Products`. Its reduced recurrence is diagonally scaled CG;
  there is no retained PCG preconditioner interface. Direct calls have a
  Cholesky factor; explicit iterative calls have a normalized matrix.
  Reusing the latter requires a specified approximate inverse action.
- Ordinary frames do not necessarily build canonical geometry. The latest
  successful producer can precede the last ordinary frame by several weight
  updates. The reported near-final drift of about `1e-6` must be verified
  against that producer's weights. `fe/cmg.rs` has a weight-ratio diagnostic
  for seed admission, not a canonical preconditioner cache.
- The canonical certificate already reconstructs residuals from original
  inputs and accumulates both FE score families and the weighted Gram in
  one traversal. H6 also shares selected LSMR/OLS norm reads. Inventory the
  remaining certificate passes before proposing another fusion.

### H9a. Freeze the contract, operator and preconditioner

**SPEC prerequisite.** Section 13 says to refresh/invalidate numerical backend
state whenever weights change. Obtain Johannes's decision on a narrow
clarification before runtime implementation: all current-weight operators,
products and proofs are refreshed/invalidated; separately typed historical
preconditioner state may survive solely as solve acceleration. It carries no
certificate, current-weight identity, canonical-rank or Hessian authority.
Do not silently reinterpret that sentence or edit the strict tolerance.

Use two categorical FE partitions and the existing canonical eligibility.
OLS, explicit LSMR/CMG, three-FE routes and unsupported canonical shapes keep
their existing paths. For the first prototype, saved-FE requests use the cold
path; enabling them later requires a named reconstruction witness. Keep
accepted-mean final-product reuse and refresh decisions unchanged.
Strict-only discovery enables the cache consumer in `strict_candidate` only;
any separately required accepted-mean solve keeps its cold path at this stage.

For eliminated-side masses `M_a`, retained-side masses `M_b` and aggregated
weighted incidence `C`, the grounded Schur operator is
`S_t = M_b,t - C_t' M_a,t^-1 C_t`. Rebuild current normalized weights, masses,
incidence values, RHS, roots and anchor decisions on every call. Adapt the
existing `apply_incidence_schur` implementation; skip only the pair assembly
and the setup explicitly replaced by the cache. An implicit operator still
costs setup and work per application. It is exact in its weight/design inputs,
not promised bitwise identical to explicit matrix multiplication.

Freeze one inverse-action design per existing geometry representation:

| Producer | Cached payload | Inverse action for the first prototype |
| --- | --- | --- |
| Explicit direct | Existing admitted Cholesky factor of normalized `A_0` | Forward/backward triangular solves |
| Explicit iterative | Existing normalized matrix `A_0=D+L+L'` | One symmetric Gauss-Seidel action: `(D+L')^-1 D (D+L)^-1 r` |
| Implicit or declined canonical producer | None | Existing cold route |

The second action is a fixed linear SPD preconditioner when its diagonal is
positive and finite; it is not `A_0^-1`. Its quality must be measured. Charge
both triangular sweeps, diagonal storage and setup. Do not add an unpriced
Cholesky factorization, residual-dependent inner CG, or a second solver family
to rescue a failed prototype. Ordinary PCG requires an appropriate symmetric
positive-definite preconditioner; nonlinear inner solves require a different
method ([PETSc CG](https://petsc.org/release/manualpages/KSP/KSPCG/),
[flexible CG](https://petsc.org/release/manualpages/KSP/KSPFCG/)).

Current and cached coordinates must match. With roots `R_t`,
`A_t=R_t^-1 S_t R_t^-1`; transport cached inverse action `B_0` by
`B_t=T B_0 T`, `T=R_t R_0^-1`. Check finite, positive scale conversions.
Require matching sample/generation, original-row axis, FE source/coding,
eliminated side, components, anchors, free-coordinate order and generated
column/basis identities. Current anchors use largest weighted mass and can
change: a mismatch discards the cache. Do not freeze yesterday's anchors.

Measure drift against normalized **cache-build** weights, including the
solver divisor: `rho=max_i(max(w_t[i]/w_0[i], w_0[i]/w_t[i]))`. An invalid,
underflowed or overflowing ratio declines reuse. Propose the fixed initial
threshold `rho <= 2`, without fitting it to campaign cases. Positive-weight
quadratic forms obey `rho^-1 S_0 <= S_t <= rho S_0` on identical grounded
coordinates: eliminate the left coordinates by minimizing the same weighted
quadratic form to derive the bound. Thus transported preconditioning bounds
the condition number by `rho^2` times its build-state condition number. It
does not prove unchanged iteration counts, especially for Gauss-Seidel.
No adjacent-frame reset of the drift baseline is permitted without a rebuild.

### H9b. Own, admit and retire the cache safely

Implement an optional preconditioner owner in `CertifiedLinearSession` and
separate operator/preconditioner application in `canonical_products.rs`.
Install a producer only after its certificate and enclosing frame acceptance;
its build identity stays immutable. Strict trials borrow accepted cache state.
On rejection, error, Break or rollback, discard speculative replacements and
clear optional cache state without allocating. Clear it on sample/topology/
basis incompatibility and inference handoff. Restore accepted fit authority
through the existing transaction regardless of cache availability.

Budget actual capacities before retention. The payload includes approximately
`8*N + 8*sum(d_c^2)` bytes for build weights and factor/matrix, plus roots,
diagonal, identities and component/index storage. Charge new PCG scratch,
fresh implicit incidence and weights, reconstruction storage and any old/new
overlap as well. Today's weight bank becomes the response buffer, so retaining
it can require another N-vector; moving a factor does not make its longer
lifetime free. Prefer releasing an obsolete cache before replacement over
keeping two matrices. Allocation/admission misses retain the already selected
Fast/Low route and public memory allowance, without making a formerly feasible
cold solve fail. Include actual peak reporting and optional-allocation faults.

Retain existing cold admission on misses. `Plan::new` currently rejects some
calls on pair-assembly work before representation selection; specify a reuse
plan that charges the operations it executes, rather than bypassing that guard
globally or restarting A3's stopped selector calibration.

For each component/RHS, preserve zero-RHS behavior and the original target;
let `M=min(max_iterations, component_dimension)`. Initially allow speculative
PCG at most `floor(M/2)` iterations (decline when zero), then at most one cold
rebuild using the remaining budget. Count all failed iterations and operator/
preconditioner applications, including explicit residual checks. A late
certificate failure consumes the same ledger; no retry resets the cap.
Keep the existing unpreconditioned Euclidean residual norm and original-RHS
reference for stopping. PCG's `r' B_t r` supplies recurrence coefficients,
not a replacement stopping norm. Require finite positive preconditioner energy
for a nonzero residual and retain current-operator curvature guards; breakdown
uses the same bounded fallback. The present CG helper uses one square norm
for both jobs, so separate these quantities explicitly and test them.
Fatal errors propagate through the existing transaction. This bounds work
but cannot guarantee identical behavior at every finite budget: changed
success/fallback or protected decisions in the validation panel stop the
prototype, rather than justify a special-case fence or tolerance change.

### H9c. Prove correctness before measuring fits

Declare H9 as result-changing work. Name affected projected response/controls,
coefficients, eta/mu, subsequent working responses and weights, objectives,
continuous score/certificate diagnostics, Hessian/bread, covariance and derived
statistics, plus reduced iteration/preconditioner work. Bind exact replay fields
to the frozen baseline before candidate calls. Sample, rank, omission,
separation, missingness, success/error, certificate pass/fail, outer counts,
strict accept/resume decisions and existing fallback decisions stay exact;
new internal preconditioner-use diagnostics are explicitly scoped. Unaffected
routes remain exact. Require fixed-worker repeatability and cross-worker
scientific agreement with stable discrete decisions.

Independent witnesses, before full-fit discovery:

1. In `fereg-oracle`/`fereg-testkit`, explicitly assemble the grounded dummy
   design and current Schur system; compare operator products, solutions and
   orthogonality. Check both inverse actions for linearity, symmetry, positive
   energy and coordinate transport against an independently formed matrix.
2. Exercise uniform/nonuniform drift, cumulative small changes crossing the
   threshold, normalization changes, anchor swaps/ties, disconnected and weak
   graphs, extreme weights, zero/tiny RHSs and multiple RHSs. Preserve the
   existing tolerance and accuracy bounds; no new relaxed witness threshold.
3. Run strict-movement, accepted-Hessian, explicit-dummy PPML, coefficient/
   covariance and strong-statistic witnesses, saved-output cold fallback and
   both mean-bias modes. Recompute certificates from original inputs/current
   weights, never the PCG recurrence or retained matrix.
4. Test memory boundaries and actual capacities, optional allocation failure,
   exhausted work, late certificate failure, Break, rejected strict trials,
   separation generation changes and atomic rollback. Review the architecture
   independently with bounded high-effort Astra assistance before integration.

### H9d. Execute bounded stages and stop on failed prerequisites

Freeze source/plugin/input identities, rules, tracing and all limits before
each stage. Proposed budgets are maxima, not obligations to exhaust them.
Every attempted fit and failed attempt counts; no measurement retry or
outcome-driven threshold change is included.

| Stage | Work and cap | Gate to the next stage |
| --- | --- | --- |
| Inventory | Four frozen PPML inputs (AKM 1M, dense 1M/4M, sparse 1M), 1/4/16 workers: 12 baseline fits; 120 s/fit, 900 s total | Identify actual producer/consumer calls, representation, cache age/drift, compatible anchors and admitted bytes; demonstrate a plausible total-cost saving on post-H3 code |
| Operator/cache prototype | One design above, at most two substantive repairs and one working day; focused witnesses before campaign fits | SPEC clarification recorded; inverse actions, current operator, scientific scope, memory and rollback checks pass |
| Strict-only discovery | Same four inputs/workers; two alternating baseline/candidate pairs per cell, 48 fits; 120 s/fit, 2,400 s total | Reuse occurs; declared science passes; dense 4M total native time improves by at least 5% in both pairs at 4 and 16 workers; each cell's median paired cost increase is within max(5%, 1 ms) |
| Ordinary-frame extension | Only after strict success: one pair per original cell plus one pair for four frozen held-out graph shapes at 1/4/16 workers, 48 fits total; 120 s/fit, 2,400 s total | Same scientific/protected decisions; geometric mean of paired total-native-time ratios below one against strict-only; no cell exceeds the allowance; all holdouts judged without retuning |

Freeze the holdouts before ordinary-frame edits: uneven degrees, weak bridges,
disconnected components and wider RHS banks. Bind exact generator parameters,
seeds and hashes then; these are evaluation inputs, never runtime selectors.
The proposed drift rule is unchanged. Refresh on incompatibility or excessive
drift, and otherwise reuse only admitted state. If additional cost gating is
necessary, stop and declare a general costing proposal before implementing it.
Do not identify eligibility by dataset name, size bands or a dense-case flag.

Trace each canonical call's phase (ordinary, strict candidate or accepted-mean
refresh), assembly/factorization and current-operator setup, inverse-action
cost, per-RHS applications, residual reconstruction/certification, cache
hits/misses/rebuilds, ratio and total native/command time. Include the cost of
creating and retaining the cache in the earlier frame. Report peak memory and
discarded work. A shorter strict phase alone does not pass discovery.
These small panels screen hypotheses; they establish no release speed claim.

On a stable passing candidate, complete `./scripts/run_tests.sh`, scoped
`scripts/run_plugin_replay.sh` including 2/4/8/16-worker repeats, mapped licensed
PPML scientific/transaction checks, universal Mac build/load/ABI checks and
the complexity ratchet. Then qualify once on a quiet Mac: twenty frozen
standard workloads, seven alternating measured pairs after one excluded
warm-up pair per cell (320 calls, 90 s/call, 2,400 s total), under the existing
paired-median 95% bootstrap upper-bound allowance of max(5%, 1 ms).
Also measure the frozen dense 4M target at 4/16 workers with the same rule
(32 calls, 120 s/call, 1,800 s total) if absent from the standard panel.
Require a 95% upper bound below 0.95 for the claimed material target gain.
No continuation is included. Finish edits, targeted profiles, reviews and
science first; run no competing work on the measured host. Failed or
inconclusive qualification remains a failed prerequisite to a performance claim.

Preserve compact hashes, all observations and decisions in the existing
`bench/results/`, `VALIDATION.md` and this brief; remove task outputs from
`target/tmp/h9/` at closure. No GitHub testing or new platform claim follows
from this proposal. Keep each implementation boundary coherent and reviewable.

### Follow-ups: certificate passes, then a separate A2 decision

Certificate fusion is an independent arithmetic-preserving slice. First map
the remaining passes in `fe/certificate.rs`, `fe/borrowed_map.rs` and their
callers, excluding the already-fused canonical path and H6 norm handoff.
For an eligible path, fuse effect accumulations only if every cell keeps its
observation order, compensation/FMA recurrence, scale and final reduction
order. Preserve original-RHS reference norms and the residual norm; they are
different quantities. Some scale-dependent recurrences may need a prior norm
pass. One residual read is an optimization target, not a blanket exactness
proof or a universal lower bound on this implementation's pass count.

Keep independent evaluation from declared design/current weights and returned
residuals. D1 remains reject-only; no recurrence-derived certificate, row
sampling or sketch replaces the full check. Admit all-effect accumulators
within the existing allowance, retain the existing path on a memory miss,
and preserve parallelism where a serial scatter would lose more than it saves.
Require bitwise certificate/norm/output equality and unchanged signatures,
including failure paths, plus a measured removed pass/instruction reduction.
Bound this slice to one design, two repairs and one working day; qualify its
own stable boundary without mixing its gains into H9 discovery.

Only after H9's measured setup savings, propose a separately budgeted A2
retest against H9 without guesses. Retain A2's failed result: its 1.883%
total-cost increase was not a proven setup attribution. Count guess conversion,
initial exact-operator application, all PCG/preconditioner work and retention
memory. No automatic reopening of A1/A3, warm-start integration or relaxation
of the strict `1e-12` cap follows from this plan.
