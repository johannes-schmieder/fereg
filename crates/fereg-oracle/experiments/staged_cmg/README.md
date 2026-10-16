# Staged CMG discovery prototype

This standalone numerical experiment is not linked into the fereg runtime.
It tests weighted two-FE Schur solves using the existing pinned CMG library,
exact degree-one/two elimination, a compensated heavy-edge forest, and a
one-level Galerkin prefix that can retain its arrays while upgrading its tail.
The outer operator always uses canonical worker-firm incidence. No model,
sample, rank, omission, separation, or public dispatch behavior changes.
The scope of numerical differences is the prototype's iterates and answers,
which must pass the same explicit relative residual certificate (1e-8).

Inputs are synthetic public mathematical constructions. Setup is charged to
each complete batch: clique construction is deferred and diagonal PCG never
pays for it. Input generation and shared incidence/component preparation are
outside timing. These are prepared linear-system timings, not Stata estimation
timings. All methods reuse their setup across right-hand sides, and an upgrade
persists for subsequent right-hand sides. Everything is serial; an isolated
whole node prevents contention, it does not imply 28 computational threads.

Independent witnesses cover dense grounded solves, worker-centered energy,
incidence/clique equivalence, components including isolates, linearity,
symmetry, positive energy, compensated-forest inverse bounds, and exact
elimination reconstruction. Fixed inputs and methods must repeat bitwise on
the same machine. Runtime/ABI evidence is unaffected by this standalone crate.

Discovery protocol, frozen before measurements: five graph families (expander,
fringe, weak clusters, weighted grid, high-degree workers), 4096 firms, a single
cut RHS, a single random RHS, and a mixed four-RHS batch, eight arms, one full
warmup per cell/arm, six blocks.
Each block uses a seeded (20261016) randomized permutation followed by its reverse;
workload order is also seeded and shuffled. Every observation is retained.
The batch iteration limit is 16000 per RHS and the external per-batch timeout
is 30 seconds. Scientific failures and timeouts are failures, not exclusions.
No replacement blocks or continuation are allowed. An allocation with other
scheduled jobs or verified competing compute is invalid as a whole; preserve
its evidence. OS/observer activity is measurement noise. Summaries use paired
block geometric-mean ratios and report the six block scores and their range;
these are bounded discovery observations, not release qualification or a
Mac performance claim. A separate untouched confirmation is required before
promoting any finalist.

Progressive arms start with diagonal, forest, or one-level CMG. At powers of
two from iteration 16, they use a true-residual window to forecast remaining
work, including known future RHSs. Upgrade at most once if projected work is
at least four times a fixed heuristic setup-work score and
work already spent is at least half that score. The assumed benefit is a
heuristic, not an online competitive guarantee. Checkpoint and certificate
matvecs count as work. An upgrade recomputes the true residual and restarts
PCG at the current iterate, preserving the original target and iteration cap.
A failed apparent-convergence certificate triggers at most two charged
residual-replacement restarts; otherwise the solve fails explicitly.

Build and check with `cargo test --manifest-path crates/fereg-oracle/experiments/staged_cmg/Cargo.toml`.
The binary accepts `CASE FIRMS RHS_COUNT ARM [cut|random]` and emits one JSON result.
The Python SCC driver supplies fixed budgets and retains raw observations.

The setup score (32 edges + 16 vertices) is a heuristic, not a cost bound.
Every checkpoint records the forecast, score, decision, and RHS index. A mixed
batch uses one cut RHS followed by three random RHSs; the single-random cell
separates composition from the opportunity to amortize setup.

## Initial validation

On the prototype branch based on `73d11a8a`, all seven Rust numerical tests,
six Python runner tests, Rust formatting/Clippy checks, and the 40-cell local
functional smoke passed. The progressive tests exercise a recursive upstream
tail and verify its reuse on a second RHS. These are correctness checks, not
local timing evidence. The independent mathematical/code review's finite
certificate, transition-coverage, and forecast-labeling findings were addressed.

`./scripts/run_tests.sh` passed 244 Python tests (12 skips), then stopped at
`check_github_install.py`: the checked-in plugin metadata describes source
`46aa78f67b102373675b7c72196dba1fbfb7d337` and does not match this base runtime.
The metadata is unchanged from `73d11a8a`; `check_complexity.py
--compare-production 73d11a8a` passes (192 runtime modules, 58 artifact inputs).
The remaining full-suite checks were not reached. This experiment does not
repair or qualify the existing installed plugins.

The SCC Rust 1.85.1 Linux release build and test compilation passed. The binary
SHA-256 is `ccbd046935bca8f9a6ce775680e2486e4e571361bfad1841fd5bff64ae106ec1`.
No numerical execution or speed claim follows from that compilation.

## SCC execution blocker (2026-10-16 UTC)

The executable-source manifest SHA-256 (all experiment files except this
README and Python caches, lexicographically sorted `sha256  relative-path`
lines) is `c4158e2168122e13486f40b62eda4a82159be6f139ca3c867730f5bbb52b1cd2`. The remote source was hash-verified before compilation.
No production source or native distribution artifact is changed.

SCC login and storage access worked. Repeated bounded `qstat` and `qconf`
requests failed with a GDI request-response timeout. The single smoke submission
named `fereg_scmg_1016_smoke` used project `welfgr`, two `omp` slots, 3 GB/core
and ten minutes; `qsub` timed out (exit 124) without returning a job ID.
Acceptance is unconfirmed. Do not submit a duplicate until scheduler/accounting
checks resolve that exact job name. The remote campaign identifier is
`20261016-staged-cmg-prototype-73d11a8a`.

No compute-node numerical run, `qacct` acceptance, isolation validation or SCC
timing evidence exists yet. Once the smoke is accounted successful with its
application marker, discovery requests `-P welfgr -pe omp28 28 -l
num_proc=28,mem_per_core=3G,h_rt=00:45:00`. The runner independently verifies
physical cores, affinity, PE allocation, global slot quota and competing jobs,
and samples competing compute. Its internal measurement budget is 30 minutes.
Run `scc_job.sh measure CAMPAIGN_ROOT` only after the smoke succeeds; do not
infer node isolation from the slot request alone. Both modes use the same
prepared source and compiled candidate. All 1560 planned discovery observations
(including warmups and failures) must be retained if collection completes.
