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

## SCC discovery results (2026-10-16 UTC)

The completed run and all six block ratios per cell are retained in
[discovery.json](discovery.json). Its implementation revision is `a748b81f`
and its launcher revision is `6ced4eba`. The numerical binary remains
`ccbd046935bca8f9a6ce775680e2486e4e571361bfad1841fd5bff64ae106ec1`.
The final executable-source manifest SHA-256 is
`d5fb80b68839ee7b16199a6df6b5750bc28ea81948aeac408148c821ff1bb8d4`.
The preceding manifest was
`c4158e2168122e13486f40b62eda4a82159be6f139ca3c867730f5bbb52b1cd2`;
only the batch launcher changed between them.

The campaign is `20261016-staged-cmg-prototype-73d11a8a`. SCC scheduler recovery
was verified at 09:42 UTC after the overnight GDI failure. Fresh accounting,
the live queue and output checks found no matching job or smoke output
before a new submission was made; historical acceptance remains unconfirmed. The first accepted smoke job,
`8137459` on `scc-gr4`, passed seven Rust tests but failed before any Python
smoke observation because the node's default Python 3.6 lacks `capture_output`.
Its accounting is `failed=0`, `exit_status=1`; its logs and original launcher
were preserved. The narrow repair selects Python 3.12.11 explicitly from
`miniconda/25.3.1` and caps Cargo/libtest workers at the reserved slot count.
No solver, graph, tolerance, workload or measurement-protocol change was made.

The repaired two-slot smoke job `8137471` on `scc-gr4` passed seven numerical
tests and all 40 unique smoke cases, emitted `STAGED_CMG_SMOKE_PASS`, and was
accounted `failed=0`, `exit_status=0` (5 seconds, 270.273M maxvmem).
Shell syntax checks and all six Python runner tests passed locally.
The independent Astra extra-high review accepted the launcher repair.

Discovery job `8137490` ran on `scc-wo4` from 09:50:45 to 09:54:36 UTC using
project `welfgr`, PE `omp28`, 28 reserved slots, 3 GB/core and a 45-minute wall
limit. It was accounted `failed=0`, `exit_status=0`: 231 wall seconds,
241.169 CPU seconds and 247.504M maxvmem. Actual solves were serial and pinned
to one core. All 1560 observations completed: 120 warmups and 1440 measurements,
with zero failures or timeouts. Every workload/arm combination had its
expected 13 observations.
All true relative residual certificates passed (maximum
`9.981490464158646e-9`). Digests, iteration counts, upgrades, checkpoint
decisions, tail levels and certificate counts repeated exactly within each cell.

The initial and final whole-node configurations agreed. The 22 isolation
samples span 225.157 seconds with a maximum gap of 10.889 seconds; every sample
contained only this job on its node and no recognized competing compute.
The observer detects configured executable families and CPU changes between
samples; it cannot rule out every short-lived or unusually named process.
The full raw solver/isolation evidence remains on SCC under this campaign;
its SHA-256 identities are retained in `discovery.json`. This is discovery on
one SCC node with synthetic prepared systems, not Stata end-to-end timing,
scaling evidence or Mac qualification. No confirmation panel has run.

In the following table, diagonal milliseconds are the arithmetic mean of the
12 measured observations. Every other column is its method's median of six
paired-block geometric-mean time ratios to diagonal; below one means faster.
All setup, clique construction, upgrade, application and certification work
is charged. Six block scores, including their spread, are in `discovery.json`.
These descriptive ranges are not confidence intervals.

| Graph | RHS | Diagonal ms | Eliminate | Forest | Shallow | Full | Diag→full | Forest→full | Shallow→full |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| expander | cut | 4.068 | 1.724 | 1.503 | 1.510 | 10.721 | 1.034 | 1.520 | 1.521 |
| expander | random | 4.188 | 1.747 | 1.486 | 1.507 | 10.658 | 1.038 | 1.537 | 1.496 |
| expander | mixed | 15.199 | 1.218 | 1.183 | 1.194 | 4.259 | 4.093 | 4.145 | 1.203 |
| fringe | cut | 327.553 | 0.016 | 0.012 | 0.626 | 0.241 | 0.248 | 0.012 | 0.252 |
| fringe | random | 337.849 | 0.014 | 0.011 | 0.628 | 0.238 | 0.243 | 0.011 | 0.247 |
| fringe | mixed | 1296.625 | 0.011 | 0.009 | 0.642 | 0.089 | 0.091 | 0.081 | 0.094 |
| clusters | cut | 150.562 | 1.076 | 1.079 | 1.058 | 0.053 | 0.072 | 0.080 | 0.087 |
| clusters | random | 145.294 | 1.065 | 1.077 | 1.055 | 0.045 | 0.065 | 0.071 | 0.081 |
| clusters | mixed | 580.259 | 1.056 | 1.072 | 1.023 | 0.034 | 0.039 | 0.041 | 0.043 |
| grid | cut | 409.044 | 1.067 | 0.304 | 0.332 | 0.040 | 0.044 | 0.046 | 0.047 |
| grid | random | 362.518 | 1.059 | 0.315 | 0.339 | 0.041 | 0.047 | 0.049 | 0.050 |
| grid | mixed | 1442.206 | 1.072 | 0.309 | 0.336 | 0.020 | 0.022 | 0.023 | 0.024 |
| hubs | cut | 3.749 | 22.387 | 5.927 | 12.711 | 34.757 | 1.000 | 5.913 | 12.681 |
| hubs | random | 3.755 | 22.385 | 5.887 | 12.495 | 34.557 | 1.004 | 5.847 | 12.406 |
| hubs | mixed | 14.762 | 6.362 | 2.285 | 6.768 | 19.666 | 0.995 | 2.305 | 6.809 |

The basic motivation is supported, while the current upgrade rule needs work:

- Easy expander and hub systems converge with diagonal PCG in 15–20 iterations.
  Starting with diagonal avoids a substantial CMG setup/application cost.
  On a single cut RHS, hub diagonal→full takes 3.757 ms and never upgrades,
  versus 128.864 ms for full CMG. Its paired ratio to full is about 0.029.
- The compensated forest solves fringe systems cheaply without building full
  CMG. On a single cut RHS, forest takes 3.933 ms versus diagonal's 327.553 ms
  and full CMG's 78.881 ms. Forest→full stays cheap for a single RHS
  (4.022 ms); its six paired ratios to full range from 0.049 to 0.052.
  Exact degree-one/two elimination is also effective here (5.124 ms).
- Hard clusters and grids benefit strongly from full CMG. Diagonal→full
  upgrades on the first RHS at iteration 16 and preserves that hierarchy.
  For cluster cut, it takes 11.058 ms versus diagonal's 150.562 ms;
  full CMG from the start takes 7.979 ms. For grid mixed, diagonal→full
  takes 31.835 ms versus diagonal's 1442.206 ms and full CMG's 29.624 ms.
  Starting cheaply incurs a measurable discovery/restart cost when full CMG
  is needed immediately.
- Forecasting future RHS work produces clear false upgrades. On expander
  mixed batches, diagonal→full takes 61.819 ms versus diagonal's 15.199 ms
  (paired ratio 4.093). On fringe mixed batches, forest→full takes
  105.494 ms versus forest's 11.803 ms. Both upgrade on RHS 0 at iteration 16
  despite forecasting only about 3.3 or 4.4 remaining current-RHS iterations.
  These are counterexamples to treating the current work score as a reliable
  setup-benefit estimate.
- One-level CMG alone reduces iterations on some hard graphs, but its extra
  application cost and remaining slow modes prevent a consistent time win.
  The current data do not establish it as the best cheap starting method.

All observed upgrades occurred on RHS 0 at iteration 16. This panel therefore
exercises the first-checkpoint gate, rather than demonstrating useful later
window adaptation or a diagonal→shallow→full chain. The immediate-full arm's
`tail_levels=0` is unpopulated initialization telemetry, not a hierarchy-depth
measurement; do not interpret it as an absent hierarchy.

The independent Astra extra-high results audit reproduced completeness,
seeded execution order, certificates, exact repeatability and paired summaries.
It agreed with the benefits and batching counterexamples above. The retained
solver signatures include checkpoint decisions without repeating timed rows.

The next design question is how to price measured setup/application cost and
marginal iteration savings before upgrading, especially across future RHSs.
Selection or heuristic changes need a new frozen candidate and untouched
confirmation. This experiment has not changed public dispatch or selected
a replacement for the production solver.
