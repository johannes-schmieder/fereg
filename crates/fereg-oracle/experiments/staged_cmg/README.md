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

## Automatic solver design after discovery

The target is low complete-estimation latency across problem classes, with no
user solver selection. The proposed next architecture combines representation
selection, a small solver portfolio, bounded exploration and reuse across
realized solves. It is a research proposal, not a change to public Auto or an
approved replacement for the current qualification campaign.

### Existing machinery and the remaining gaps

The source audit of production revision `92774888` finds much of the plumbing
already present: original-design certificates, deterministic CMG checkpoints,
setup/memory admission, direct proposals, MAP/IT prefixes, seeded LSMR and
one-worker preparation. See [the routing contract](../../../../docs/SPEC.md),
[work accounting](../../../fereg-core/src/fe/routing.rs) and
[general pair routing](../../../fereg-core/src/fe/borrowed_map/routing_lsmr.rs).
Public OLS Auto is still the incumbent policy; PPML Auto resolves to MAP and
has separate historical-preconditioner reuse. These are distinct integration
problems.

At `within` revision `efe35ee`, automatic Schwarz already includes local
approximate Cholesky with `split_merge=Some(2)`, an AC2-style configuration.
A direct two-FE approximate-Cholesky comparison is a new placement of an
existing family, not the invention of a missing library. Production CMG pins
`3422b040`, while this frozen experiment pins `eb3f2bc1`. Compare both artifacts
before attributing a production advantage to the prototype. The specialized
production two-FE path packs active RHS columns with independent
CG recurrences; this is not true block Krylov, and other CMG adapters have
different batching. The standalone experiment solves RHSs serially.

`within::SolveSession::run_action` can resume an unchanged recurrence and restart
from a recomputed residual after replacement. Its preconditioned residual
telemetry is not comparable across replacements. CMG's `BuildPrefix` parks
construction with private state; it does not publish a usable incomplete
hierarchy. This prototype's custom one-level prefix is a separate mechanism.
An actual staged-hierarchy API must expose a usable full preconditioner that
is fixed, linear, symmetric and positive definite on the compatible quotient
at every published stage before ordinary PCG uses it. Existing construction
control can first be used as a rendezvous: keep the builder parked while the incumbent runs, then park the
incumbent while a prepaid builder quantum runs. Two retained thread stacks do
not require simultaneous numerical computation.

### Setup pricing is a separate failure mode

The independent challenge review identified an important distinction in the
existing results. Rechecking the committed signatures, the expander mixed-batch
trigger implied about `4*(16+3.282) = 77.1` diagonal iterations against 80 actually
needed without upgrading. The fringe forest trigger implied about
`4*(16+4.383) = 81.5` against 71. These forecasts were reasonably close; they do
not establish that exaggerated future-RHS difficulty caused those two false
upgrades. Using each cheap arm's measured mean non-setup time per iteration,
those forecasts price remaining work at approximately 11.6 ms and 10.1 ms;
the upgraded arms' incremental setup is approximately 34.5 ms and 65.1 ms.
Replacing only the setup price with that observed cost would reject both under
the existing four-times-price rule. This ex-post conversion supports setup
underpricing as a material cause, without claiming it was the only contributor.
Across the five single-cut cells, full-CMG setup alone cost about 14 to 617
times that family's average
diagonal iteration time, estimated as solve time excluding setup divided by
iterations. This is a descriptive conversion, not a calibrated work tariff.
Repair setup pricing and metering independently from progress forecasting.
Offline re-pricing can identify hypotheses; it cannot measure a new policy's
restart, cache, construction or certification costs.

### Architecture to investigate

1. **Keep the exact problem and its certificate central.** Every path solves
   the same weighted regression. Explicit solver requests and existing sample,
   rank, omission and separation decisions remain authoritative. Approximation
   belongs in preconditioners, never in a silently changed estimation problem.
2. **Choose the representation before buying a hierarchy.** Reuse canonical
   incidence and component metadata. Estimate duplicate-cell compression,
   degrees, possible fill, RHS width and memory traffic cheaply during existing
   passes. Compare the matrix-free incidence operator, canonical-cell operator,
   grounded direct component solves and exact low-degree reductions. Do not
   expand high-degree worker cliques just to decide whether expansion was useful.
   These choices need their own charged setup and reconstruction costs. The
   prototype elimination arm still applies the original fine-space outer
   operator: solving an exactly reduced core is a distinct opportunity.
   Production already has a retained-star CMG representation; extend and compare
   it rather than recreating that mechanism. Degree-two elimination inserts an
   edge between surviving neighbors while removing two; reducing either side
   can leave the bipartite form. The reduced-core interface must support that
   general Laplacian and account for adjacency updates and reconstruction.
   Numerical reduction must retain all observation contributions and reconstruct
   eliminated coordinates; it is
   not statistical singleton deletion.
3. **Use a small set of complementary methods.** Diagonal, forest/elimination,
   direct solves on affordable components, full CMG and existing approximate
   Cholesky are the first candidates. A shallow stage is optional: it has not
   earned a universal position between diagonal and full CMG. For three or more
   FEs, preserve the general LSMR/Schwarz route and investigate global coarse
   information that pairwise solvers miss. General multi-FE systems and slopes
   are not automatically graph Laplacians.
4. **Observe actual progress and control speculative spending.** Use original-
   space progress per active RHS/component, including certificate work. A
   noncontracting residual window is an uncertain forecast, not a prediction
   that the entire iteration cap will be consumed. A nearly finished solve
   receives a short bounded continuation before expensive construction. Do not
   multiply the first RHS's difficulty by the number of unseen RHSs. Installed
   setup may be reused; hypothetical future work may not fund current spending.
5. **Reuse what has demonstrated value.** Share fixed-weight hierarchy/factors,
   canonical operators and useful coarse spaces across RHSs. Under PPML weight
   changes, distinguish reusable topology from old numeric preconditioning and
   current numeric operators. Revalidate progress against the current problem.
6. **Allocate parallel resources deliberately.** First exploit packed RHS
   operations and independent components. Test deterministic serial interleaving
   of one incumbent and one challenger before running several bandwidth-heavy
   solvers simultaneously. A small builder on another core is useful only if
   complete-estimation measurements show that its overlap repays foreground
   slowdown, cache disruption and extra memory. Worker availability alone is
   insufficient evidence.

Keep the conservative admission ledger separate from a selection model.
Selection can use a frozen hardware/resource model of streamed bytes, irregular
accesses, arithmetic, synchronization, batch shape and factor work. It should
predict remaining setup plus remaining applications, restart and certification,
with uncertainty. It must not use live elapsed-time races to determine a
numerically different path. A learned router can prioritize a candidate, but
its predictions neither waive certificates nor authorize unbounded exploration.

A bounded spectral monitor is another candidate ranking signal. Within a fixed
PCG epoch, the existing recurrence coefficients can produce Lanczos Ritz
estimates without extra operator applications; see the
[Lanczos connection](https://netlib.org/linalg/old_html_templates/section2.9.1.html).
Its scalar computation and storage still cost work. A CG energy-error bound
using a valid condition bound is not an original regression residual certificate.
Early Ritz values may miss weakly excited slow modes and repeated eigenvalues;
they are not certified
condition-number or remaining-iteration bounds. Cap the retained history, reset
it at residual replacement or preconditioner changes, and validate a separate
Golub--Kahan interpretation for LSMR: its bidiagonal singular values refer to
the effective rectangular operator, whose normal equations square its condition
number. Initially collect this telemetry without routing on it. A small observed spectral tail can motivate coarse enrichment,
but cannot prove that the full problem has only a few troublesome directions.

### What a protected portfolio can and cannot guarantee

For an unchanged incumbent trajectory, let `I` be its cumulative charged work
and let `C` include all optional work: initialization, copies, probes, failed
builds, candidate iterations, certificates and scheduling. A single cumulative
invariant `C <= a I + q` gives a work envelope. If the incumbent reaches its
original certificate after `W_inc`, total work through that point is at most
`(1+a) W_inc + q`. New setup cannot be relabeled as incumbent work to manufacture
credit. The additive allowance `q` must not reset at every checkpoint or RHS;
large indivisible factorization steps must be bounded or split.

A challenger comparison also needs a minimum service guarantee; an upper
spending ceiling alone can starve it. The comparator must follow an independently
defined trajectory. A challenger started from a later incumbent iterate is not
proved faster than the same method started from zero. Changing shares requires
accounting for prior phases, rather than applying the final share retroactively.

This is conditional on unchanged incumbent state, sufficient service and
consistent accounting. It is not a wall-time bound: memory/cache effects and
heterogeneous work remain relevant. Choosing diagonal as the incumbent only
protects comparison with diagonal, not with production Auto. Keeping only an
iterate and restarting is not preserving the incumbent recurrence.

Existing native caps remain authoritative. Challenger iterations consume the
same per-projection cap and can cause failure before an otherwise successful
incumbent finishes. The envelope therefore applies until termination or cap
exhaustion; it is not a completion/fallback guarantee under the current shared
cap. Explicit cap-exhaustion experiments are required. Exhaustion where production
Auto succeeds is a candidate failure, never an omitted timing or a successful
regret observation. No new quota semantics are proposed by this document.

There is an unavoidable exploration tradeoff even in an ideal additive-work
model. Giving a fraction `p` to a protected incumbent and `1-p` to an independent
challenger gives service factors `1/p` and `1/(1-p)`, before startup/quantum
terms. An 80/20 split protects the incumbent to 1.25 times its work but can delay
an overwhelmingly better challenger by five times its own work. A 50/50 split
has factors two and two. Structural evidence and reusable state can improve
practical choices; they do not erase this uncertainty. These examples are not
latency promises or selected production constants. Equal logical work grants
also do not by themselves establish serial/parallel bitwise identity: numerical
ordering, worker configuration, input snapshots and adaptive decisions must
meet the existing repeatability and cross-worker scientific contracts.

### Adaptive upgrades with new information

A useful middle stage may add a small coarse space that represents errors the
cheap method cannot remove. This is a more targeted research direction than
always adding another generic level. Independent relaxed test vectors, local
aggregate modes, or information from previous RHSs can supply such directions;
[bootstrap AMG](https://arxiv.org/abs/1406.1819) develops related coarsening ideas.
[Subspace recycling](https://arxiv.org/abs/2001.10347) motivates reuse between
related solves. Neither source establishes a speedup for this implementation.

On the compatible quotient where `A` and the baseline inverse `B0` are SPD,
let the independent columns of `Z` be orthogonal to the nullspace, define
`E = Z' A Z` and `Q = Z E^-1 Z'`, and use the balanced inverse
`B = Q + (I-QA) B0 (I-AQ)`. It is SPD on that quotient and satisfies `B A Z = Z`.
For any vector `v`, its quadratic form is the sum of the nonnegative coarse
energy and `((I-AQ)v)' B0 ((I-AQ)v)`; both can vanish only for zero `v` there.
Stable auxiliary-basis filtering is not a change to model rank. Cached `AZ`
avoids two extra full operator products per application, but costs additional
vector passes, reductions and roughly two `n by k` arrays: even rank four
requires about 64 MB per million retained coordinates in double precision,
before scratch and the incumbent state.

Freeze the inverse during each ordinary PCG epoch and recompute `b-Ax` on
replacement before restarting. Fixed iteration counts do not make an inner CG
solve linear. A flexible Krylov method is a separate algorithmic change, not
permission to vary an ordinary PCG preconditioner invisibly; see
[PETSc's FCG contract](https://petsc.org/main/manualpages/KSP/KSPFCG/) and
[variable-preconditioning counterexamples](https://arxiv.org/abs/math/0605767).

A basis taken solely from the current uninterrupted fixed-PCG history provides
no new subspace information. At equal operator/preconditioner work, an iterate
confined to that same Krylov space cannot beat CG's exact-arithmetic energy
minimizer. Recycling is better motivated for later RHSs, necessary restarts,
changed operators or genuinely independent directions. A few coarse vectors
may repair a few bottlenecks; a grid's many slow modes can still require a full
multilevel construction. Basis application cost must be measured, not inferred
from iteration savings. Distinguish a stronger tail solve from better
interpolation: an exact solve on the existing coarse space still cannot remove
an error component outside that space. General spectral/interpolation
enrichment can produce an SPD Galerkin matrix with positive off-diagonals;
that matrix must not be passed to a Laplacian-only CMG backend without a valid
representation. Piecewise-constant aggregation and a separate generic SPD
coarse correction have different interfaces.

For PPML, if support and coordinates are unchanged and positive weight ratios
lie in `[m,M]`, then `m A_old <= A_new <= M A_old` in Loewner order. The same
bounds survive a common Schur elimination by the minimum-energy identity,
with the same elimination coordinates and a well-defined common quotient.
An old fixed SPD inverse remains a valid preconditioner, with a condition-number
bound worsened by at most `M/m`; it need not be rebuilt merely for validity.
A current coarse correction must rebuild `Z' A_new Z`, and current elimination
must use current weights. Extreme ratios, numerical underflow, changed support
and changing normalization can invalidate or make these bounds uninformative.
Existing PPML convergence and separation semantics remain unchanged.

### Prioritized next experiments

**First, separate two bounded experiments.** The controller experiment keeps
numerical kernels and their dependency pins unchanged. For each of diagonal
and forest, compare cheap-only, the present forecast, an observed-active-RHS
rule with one bounded near-completion continuation, and incumbent-preserving
interleaving at two predeclared exploration shares. Add immediate full CMG as
a reference. These are eleven arms, with no learned router or new numerical
family in that comparison. Charge all setup, probes, copies, restoration,
synchronization and certification; no credit for unseen RHSs. The known
expander/fringe false upgrades are mandatory regression cases, supplemented by
larger problems, disconnected mixtures and opposing RHS difficulty orders.
The result identifies a controller conditional on its starter, not an automatic
rule that selects the best starter.

The representation experiment compares existing static solver families on the
same exact input representation and reduction plan: diagonal/direct core,
forest, CMG, ordinary approximate Cholesky and its verified AC2-style variant.
Keep current diagonal and full-CMG arms as references. Separate retained-star
versus expanded-clique construction from exact outer-core reduction and from
approximate factorization. This resolves whether hub costs belong to the
solver family, its representation, or both. Ordinary AC and AC2 are not assumed
to be nested upgrades: changing sampling can change earlier elimination and
fill. Reuse the production retained-star and within factor infrastructure.

Freeze the two panels independently. A controller fix can proceed in the
existing prototype while the representation comparison establishes which
challengers merit a broader automatic selector. Before production-policy
selection, bridge to explicitly frozen production dependency pins and its
packed independent-RHS semantics. The old serial-RHS experiment cannot select
a batched production policy directly. This separation resolves the reviews'
different priorities without conflating a routing improvement with new kernels.

**Second, test genuinely useful intermediate work.** Add an owned hierarchy
stage, or the bounded coarse correction above, as separate challengers. Every
published full preconditioner must be fixed, linear, symmetric and positive
definite on the compatible quotient; a symmetric terminal alone is insufficient. Test a basis learned on one RHS against both aligned
and unrelated later RHSs, charging its training to the complete realized batch.
Test new independent local directions separately for single-RHS acceleration.
Compare against uninterrupted PCG, mandatory-restart controls and immediate
full CMG. Within the recycling experiment, use three arms: repaired controller
only, one-time coarse seeding, and seeding plus the recurring balanced inverse.
A proposed initial rank cap is four. Accepted solution corrections are a cheap
first source of candidates; reuse of their already-computed operator images
needs matching bindings and numerical validation. One-time seeding may capture
most of the benefit without streaming a dense basis every iteration. Include
unrelated/orthogonal later RHSs and moving bottlenecks, not just favorable
correlations. Do not conflate a better controller with a better preconditioner.

**Third, extend the winning mechanism to repeated and general regression.**
Evaluate PPML sequences with fixed, smooth-changing and abrupt-changing weights;
three/five-FE pairwise versus global coarse corrections; slopes; wide RHS banks;
and tight memory. Compare whole estimation including loading, projection,
coefficient fitting, covariance, certification and publication. A faster prepared
Schur solve alone does not establish the fastest regression command.

The corpus should cross graph structure, scale, edge density, degree imbalance,
component size, bottleneck strength, weight spread, RHS spectrum/correlation,
RHS order/count, duplicate-cell ratio and memory allowance. Include weak bridges,
long chains, anisotropic grids, heavy tails, dense high-degree workers and
adversarial RHS mixtures. The [SDDM2023 corpus](https://rjkyng.github.io/SDDM2023/)
adds graph stressors; it does not replace regression designs. A graph edge of
conductance `g` embeds exactly as a degree-two worker with two weights `2g`,
which makes such systems available as two-FE stress tests. Partition by
graph/topology or data source before
selection, not only by random RHS seed on the same graph. Include actual eligible
regression workloads and independent dense/QR witnesses, preserving all public
model decisions. Separate Linux discovery from focused Mac confirmation.

Before each campaign, freeze artifacts, input manifests, candidate policies,
setup allowances, per-case timeouts, total allocation budget, exclusion rules
and a single selection criterion. Use randomized balanced blocks of complete
solves at the declared harness
boundary for solver discovery, and complete-estimation blocks for regression-
command selection and claims. Retain failures, timeouts and unfavorable
observations. Summarize
median, upper-tail and worst-case regret against the best scientifically valid
feasible static candidate, plus regressions versus production Auto and memory.
Do not estimate the oracle from individual timing minima. Selection, untouched
confirmation and any repeated-look allowances are separate; an inconclusive
finalist does not replace the incumbent. Existing scientific thresholds and
the repository timing allowance `max(5%, 1 ms)` remain in force; a proposed
research ranking does not relax release qualification.

A separate CMG experiment should compare terminal-factor policies on the same
representation and operator. Attribute setup to coarsening, ordering, symbolic
fill analysis and numerical factorization. A smaller terminal can reduce setup
and increase application/iteration cost; choose using the whole solve. The
challenge review's scratch terminal-threshold timings on a shared VM motivate
this test but are not accepted performance evidence. Production's newer
terminal-affordability screen must be included before proposing another screen.
After work accounting is validated, a separately frozen experiment may compare
a fixed setup ceiling with an allowance that grows from completed incumbent
work, retaining memory, native-iteration and total-run limits. Borrowing against
hypothetical future RHSs is not a substitute for earned credit.

The first upstream changes to justify are exact continuation for the specialized
two-FE recurrence and original-space per-RHS telemetry. Existing AC factors and
`within` sessions should be reused. Controlled AC construction is needed before
claiming small interruptible build quanta; its inspected builder is monolithic.
Promote reusable hierarchy-stage APIs only after the isolated prototype shows
that those intermediate stages repay their costs. LAMG/LAMG+ is a
serious external comparator and a source of enrichment ideas, not yet a porting
commitment. The [LAMG paper](https://arxiv.org/abs/1108.1310),
[approximate elimination study](https://arxiv.org/abs/2303.00709), and recent
[LAMG+ preprint](https://arxiv.org/abs/2606.24791) report complementary strengths
on their own corpora; those claims require independent reproduction on this
regression workload. Dense sketching and mixed precision remain lower-priority
challengers until profiles justify their cost and strict numerical witnesses
qualify them; [sketch-preconditioning stability](https://arxiv.org/abs/2302.07202)
is a particular reason not to equate an iteration reduction with reliability.

## Protected-incumbent prototype and frozen second discovery

Claude Opus 5.5 Max read `f9157e09` and CMG `eb3f2bc1` and supplied the
file-level implementation plan in the [consultation](https://claude.ai/chat/699fabc6-ba7f-46ba-9917-11e57c430570).
The lead implemented it, with independent Astra extra-high API, mathematical
and architectural reviews. This remains a standalone experiment. The old eight
arms retain their original solver. The dependency revision and CMG numerical
options are unchanged; `experimental-components` exposes the controlled APIs.

Four new arms are `diagonal-q14`, `diagonal-q1`, `forest-q14` and `forest-q1`.
They retain the complete cheap PCG recurrence, its target, two-repair limit and
certificate cadence. Every eight cheap iterations, a scoped constructor gets
prepaid work and runs until a deterministic frontier; the main thread waits.
There is no concurrent numerical computation. A complete CMG hierarchy and
workspace create a separate PCG challenger, initialized from a copy of the
current cheap iterate and a recomputed original-operator residual. Later RHSs
start the challenger at zero and reuse the hierarchy. Cheap certification drains
and joins an incomplete constructor, discarding its suffix; a later RHS may
retry. All spent work and the construction cap remain cumulative across RHSs.
No partial hierarchy is usable. A winner must pass the same finite `1e-8`
certificate, including a second certificate after final centering.

The combined cheap/challenger cap is 16000 iterations **per RHS**. Certification
of the last permitted iterate is allowed; exhaustion remains failure. Builder
and challenger numerical errors remain arm failures even if the cheap track
would otherwise succeed. Memory refusal disables optional work for the batch.
The 1 GiB limit models requested storage, jointly covering input/RHS banks,
all outputs and telemetry, both recurrences, forest construction transients,
raw clique tuples, graph, private CMG construction, final workspace, an 8 MiB
builder stack and fixed bookkeeping allowance. It excludes allocator/OS
metadata and is not an RSS guarantee. Baseline memory failure fails the arm;
optional admission is checked before the builder stack is allocated.

The ledger enforces `optional_issued <= incumbent_work / denominator`, with
`denominator` 4 or 1 and no additive grant. Credits come only from performed
cheap recurrence work; future RHS counts and forecasted convergence earn none.
Grants reserve work before execution and are refunded only for the unused
remainder after terminal acknowledgement and join. Snapshotting, initialization,
certificates, failed/abandoned construction and challenger iterations are charged.
CMG's primitive-work gates are augmented with 32 units of callback bookkeeping
per boundary. The wrapper's fixed tariff is in `recurrence.rs`: incidence
matvec `16*cells + 8*workers + 2*n`, projection `8*n + 4*components`, diagonal
apply `8*n`, forest apply `24*n`, plus explicit vector/reduction allowances.
Challenger initialization/iterations prepay conservative complete-operation
bounds, including CMG's queried scalar-cycle bound. One whole scalar cycle
is indivisible. An epoch costs 256 nominal control units; each constructor
costs 4096 nominal launch/cleanup units. Construction has a cumulative `10^12`
unit cap. These fixed accounting units are **not calibrated time predictions**.
The same-machine result/decision trace is deterministic. Timing never routes.

This ledger is a spending envelope, not a completion or wall-time guarantee.
Actual allocator/deallocator, page-fault, thread startup, synchronization and
cleanup latency has no tariff-derived bound; all contributes to elapsed batch
time. The original incidence preparation/input generation remains outside timing.
CLI JSON serialization is outside the batch timer for every arm, as before;
internal decision construction, solve output storage and joining are inside.
The controlled full-build arm `full-controlled` measures controlled construction
without competition from a cheap track and must reproduce ordinary full-CMG
scientific signatures. Its total timer includes scoped hierarchy destruction;
ordinary full destroys its hierarchy after recording total time. Setup-phase
comparisons avoid this total-timer difference.

The second discovery is frozen before SCC timings:

- Thirteen arms: the original eight, the four shares and `full-controlled`.
- Twenty-two cells: the original fifteen at 4096 firms; grid and clusters with
  four RHSs at 8192; hubs with one random RHS at 8192; a disconnected interleaved
  expander/grid mosaic at 8192 with cut and four mixed RHSs; and reversed mixed
  RHS order for expander and fringe at 4096. The smaller extension deliberately
  keeps this first controller discovery bounded; it does not test broad scaling.
- Seed 20261017; one complete warmup and six randomized balanced forward/reverse
  blocks per cell. Exactly **3718 observations**: 286 warmups and 3432 measured.
- Thirty-second per-batch timeout, thirty-minute internal campaign budget,
  forty-five-minute scheduler limit. No replacement sampling or rescue panel.
  Failures, cap exhaustion and timeouts remain observations. No scientific
  failure is relabeled as host noise. Missing output is incomplete evidence.
- One isolated 28-physical-core SCC node, `omp28`, all 28 slots reserved, both
  solver and parked builder pinned to one physical core. Verify configuration,
  live allocation and competing compute before/during/after the panel.
- Smoke first: thirteen Rust tests, 65 small arm/case observations, and 135 Linux
  legacy replays (120 original-arm signatures plus 15 full-controlled comparisons).
  Require `qacct failed=0`, `exit_status=0` and `STAGED_CMG_SMOKE_PASS`.
- Discovery acceptance requires accounting success, complete observations,
  `STAGED_CMG_DISCOVERY_COMPLETE`, exact repeated scientific/control signatures,
  finite certificates and valid isolation. Scientific failures may be retained
  in a completed panel but disqualify the affected candidate.
- Report six paired block ratios against cheap-only, full and old-trigger arms,
  plus setup, completion/refusal/abandonment, winners and tariff expenditure.
  The descriptive screen requires zero scientific failures/timeouts, easy false-
  upgrade cells within `1 + share + 0.05` of their cheap starter, and grid/cluster
  time below half the cheap starter. This is a discovery screen, not a release
  gate. No finalist is promoted without untouched confirmation; no Mac claim
  follows from these Linux measurements.

New witnesses check controlled/full exact equality, cheap trajectory completion
with zero service and memory refusal, independent dense agreement including
isolates, repeatability across construction grant quanta, deterministic abandon
and join, iteration exhaustion and actual CMG installation/reuse. Runtime
production tokens and artifact inputs are unchanged from `f9157e09`.

Before deployment, all thirteen Rust numerical/controller tests, eight Python
harness tests, formatting, strict Clippy and `git diff --check` passed. The
independent architectural reviewer verified the memory/cap repairs and found
no remaining blocker. `check_complexity.py --compare-production f9157e09`
confirms unchanged production identity (192 modules, 58 artifact inputs).
With installed Python 3.14, `./scripts/run_tests.sh` passed 243 Python tests
(13 skips), then stopped at the same pre-existing plugin/source identity check
described above; the remaining runtime/ABI suite was not reached. An initial
invocation with the shell's Python 3.9 failed four imports because `tomllib`
was unavailable; using the installed supported Python resolved those imports.
These are functional checks, not local performance evidence.

### Separate grid input correction, declared before its data

The original second panel is retained unchanged, including every failed row.
Its larger grid cell is a protocol error: the generator requires a square
vertex count, and 8192 fails before any solve or valid timing. This cell supplies
no solver evidence. The 8192 cluster residual-replacement failures and the
larger-hub controlled-full memory refusal are genuine retained outcomes, not
input errors. The original screen cannot pass and will not be rescued.

After independent Astra review, a **separate supplemental discovery** repairs
only the input-domain mistake. The deterministic choice is the largest square
not exceeding the originally intended 8192, namely 8100. The new
`protected-grid` manifest has one cell, grid/8100/four mixed RHSs, all thirteen
arms, seed 20261018, one warmup and six balanced blocks: 169 observations
(13 warmups, 156 measured). It reuses the exact Linux executable from `e0155f8b`.
The per-solve timeout remains 30 seconds; internal budget is 600 seconds and
scheduler limit 15 minutes. It runs once after the original allocation ends,
with the same independently verified whole-node isolation and one-core pin.
There are no retries, substitutions or pooling with original observations.

The harness now validates every manifest dimension before execution, including
mosaic's embedded half-size grid. Historical constants and summary lookup are
preserved, so regenerating the original summary cannot hide its invalid cell.
The supplemental allocation first records thirteen exact-cell functional
preflights outside the statistical panel. Panics, malformed output, wrong
identity or invalid successful certificates block measurement. Structured
numerical/resource failures and timeouts stay failures and are retained.
Supplemental success cannot erase the original failures or pass its screen.


### Protected-construction results: no candidate for promotion

The implementation is `e0155f8b`; input validation and the separately declared
supplement are `c91c68e3`. The [retained evidence](protected-discovery.json)
contains all 299 arm/cell summaries, paired block ratios, deterministic decision
records, failure counts, scheduler accounting, artifact identities and raw-output
hashes. Original and supplemental campaigns remain separate. Raw observations
remain on SCC in campaigns `20261016-protected-cmg-e0155f8b` and
`20261016-protected-cmg-grid-c91c68e3`. The executable SHA256 is
`78313fb46e35b56da71a7261620b1852fdeca3beb6751babad2cbc0de967878f` in both.
No production runtime or upstream numerical code changed.

Smoke job **8138077** passed thirteen Rust tests, 65 small solves and 135 legacy
replays, with the required marker and scheduler `failed=0`, `exit_status=0`.
Original discovery job **8138109** ran on `scc-yk4` for 1190 seconds, ending
2026-10-16 12:53:02 UTC. It recorded all **3718** observations: 286 warmups and
3432 measured solves. Both scheduler fields were zero and the completion marker
was present. The runner recorded 115 isolation snapshots. Numerical computation
used one physical core; reserving and independently checking all 28 cores
provided isolation. There were no observed competing compute jobs, replacements
or excluded observations. These are Linux prepared-Schur measurements, not
end-to-end regression-command timings or Mac qualification.

The original panel retained **351 failures**, including 27 warmups:

- 169 grid/8192 observations failed before solving because that generator
  requires a square vertex count. This is the input-domain error described
  above; these rows provide no timing evidence.
- 169 clusters/8192/four-mixed-RHS observations reached the residual-replacement
  limit, across all thirteen prototype arms. This is an unresolved numerical
  failure, not evidence specific to the new controller or to production solvers.
  The failed rows do not identify the failing RHS/recurrence.
- Thirteen full-controlled/hubs/8192/random observations were rejected by the
  conservative joint memory model. This was not an observed out-of-memory
  event and does not show ordinary full CMG actually exceeded 1 GiB. Recorded
  bounds describe admitted storage, not the rejected proposal or measured RSS.

All successful scientific/control signatures repeated exactly. The largest
accepted relative residual was `9.981490464158646e-9`. All 176 distinct successful
fixed-share RHS records obeyed their cumulative ledger, memory and per-RHS
iteration limits. All 63 successful arm/cells with no challenger iterations
matched their cheap-only result, iteration count, certificate count and residual
exactly. All nineteen successful full-controlled cells likewise matched ordinary
full. Independent Astra review reproduced the paired summaries to rounding
error and verified these invariants.

The table reports arithmetic mean batch milliseconds and median paired ratios
against the cheap starter. Brackets show the range across six balanced blocks,
**not confidence intervals**. Mixed batches have four RHSs; cut batches have one.
Different rows illustrate different fixed policies, not a selected universal
winner. Old trigger means the corresponding `diagonal-full` or `forest-full` arm.

| Cell | Protected arm | Cheap ms | Old trigger ms | Protected ms | Protected / cheap |
|---|---|---:|---:|---:|---:|
| expander/4096/mixed | diagonal-q14 | 16.945 | 70.498 | 26.475 | 1.581 [1.512, 1.604] |
| fringe/4096/mixed | forest-q14 | 13.554 | 128.684 | 19.003 | 1.460 [1.188, 1.465] |
| clusters/4096/cut | diagonal-q1 | 167.929 | 11.495 | 86.562 | 0.516 [0.509, 0.524] |
| clusters/4096/cut | forest-q1 | 176.719 | 12.407 | 70.391 | 0.399 [0.394, 0.400] |
| grid/4096/mixed | diagonal-q1 | 1532.401 | 34.535 | 495.751 | 0.321 [0.317, 0.338] |
| mosaic/8192/mixed | diagonal-q1 | 3717.911 | 132.443 | 1805.376 | 0.486 [0.482, 0.489] |

Protected spending reduces some old false-upgrade penalties and can buy useful
CMG convergence on hard cells. It still loses substantially to cheap-only on
easy cells and to the old trigger/full CMG on these hard examples. **Every one
of the four fixed-share arms independently fails the declared easy-overhead
screen on expander mixed.** Diagonal/forest quarter-share paired ratios are
1.581/1.437, above 1.30; full-share ratios are 2.522/2.479, above 2.05.
The panel cannot promote a policy, even setting aside its retained failures.

Construction is the principal measured bottleneck. For expander mixed,
ordinary full versus full-controlled takes 72.985 versus 681.158 ms in total,
and 43.813 versus 651.371 ms in setup. For grid/4096 mixed, the corresponding
numbers are 32.651 versus 194.147 ms total and 11.587 versus 172.967 ms setup.
On these and the representative fringe, cluster-cut and mosaic cells, controlled
setup is about fifteen times the ordinary setup cost. Setup already includes
graph construction; adding graph time again would double count. This localizes
the gap to the setup path, but does not identify callbacks, sorting, allocation,
synchronization or another primitive as its cause. The total-timer destruction
difference above also limits treating full-controlled as a perfectly isolated
builder-only comparison.

Completed hierarchies are reused, but unfinished construction is discarded.
Across successful cells, diagonal-q1 has eight completed, nine reused, 26
abandoned and one memory-refused RHS records. Each other protected arm has
three completed, three reused, 37 abandoned and one refused. Cumulative credit
preserves expenditure, **not construction progress**. The largest cumulative
build charge is only `6.267e9` against the `1e12` cap, so cap exhaustion does
not explain this panel's behavior.

### Separate valid larger-grid outcome

Job **8139211** ran the predeclared grid/8100/four-mixed-RHS supplement on
`scc-zj4`, using the identical binary. It finished at 2026-10-16 13:01:57 UTC
in 407 seconds, with scheduler `failed=0`, `exit_status=0`, the completion
marker, thirteen successful preflights and all **169** successful observations
(13 warmups, 156 measured). Forty isolation snapshots and matching initial/final
configuration checks passed. Exact signatures included the preflights; all
sixteen protected RHS ledgers passed. Maximum residual was
`9.968823268997952e-9`. Independent review reproduced the schedule and summaries.

| Arm | Mean batch ms | Median paired ratio to cheap starter [six-block range] |
|---|---:|---:|
| diagonal | 3992.216 | 1 |
| diagonal-q14 | 5081.886 | 1.274 [1.261, 1.280] |
| diagonal-q1 | 7236.343 | 1.814 [1.804, 1.817] |
| forest | 1324.061 | 1 |
| forest-q14 | 1701.633 | 1.285 [1.282, 1.290] |
| forest-q1 | 2327.520 | 1.757 [1.752, 1.763] |
| full | 109.827 | — |
| diagonal-full | 114.114 | — |
| full-controlled | 1015.063 | — |

All four protected policies abandoned construction on every RHS, with zero
upgrades or challenger iterations. Diagonal-q1 spent about 17.545 billion
optional units without installing CMG. Full-controlled/full has a median paired
ratio of 9.260 [9.083, 9.328]; its setup alone takes 967.155 versus 64.205 ms.
The memory limit and cumulative construction cap did not bind.
This separate cell supplies a counterexample to robustness across problem
sizes. It is **not** a cross-node scaling measurement, cannot repair the original
invalid cell and is not pooled with that panel. The favorable grid/4096 result
therefore does not establish broad adaptive speed.

### Next bounded experiment

The mechanism is implemented and evaluated; no production promotion is justified.
The next work should target the observed causes before tuning shares or adding
parallel solver races:

1. Profile ordinary and controlled construction on the same prepared operator,
   with identical numerical options and matched timer boundaries. Attribute
   setup to graph assembly, sorting/coalescing, coarsening, terminal construction,
   work/memory gates and allocation. Use a fixed small diagnostic set and budget.
   A CMG upstream change should reduce the identified control-path cost while
   preserving exact scientific signatures, bounded deterministic yield points
   and conservative admission. Check whether batching bookkeeping or using a
   cheaper continuation representation helps; the present data do not choose
   which implementation is responsible.
2. Retain the parked, incomplete constructor across **realized** RHSs so later
   earned credit advances the same build. Admit its retained memory, preserve
   each cheap recurrence, charge all work and cleanup, and grant no credit from
   speculative future RHSs. This needs a new frozen artifact and witnesses for
   RHS-order behavior, cancellation, budgets, exact repeats and installation.
3. Separately reproduce the cluster/8192 failure with per-RHS original-incidence
   residual and compatibility witnesses. Keep the existing scientific threshold
   and repair limit until the cause is understood. Failure across all prototype
   arms is not permission to relax either safeguard.

Only after those bounded diagnostics should a new discovery panel compare
routing and upgrading against the best feasible static alternatives. Broad
reliability must include easy-case overhead, hard-case completion, memory,
multiple RHSs and untouched confirmation; the present spending envelope alone
supplies none of those performance guarantees. The nine current harness tests
pass after adding dimension validation. The runtime evidence and the documented
repository-suite identity blocker above remain unchanged.


## Persistent-construction prototype: declared scope

The next prototype preserves a parked constructor across realized RHSs. Existing
`q14`/`q1` arms remain controls; new `p14`/`p1` arms retain construction progress
without forecasting future work. The same cumulative ledger, memory admission,
16000 combined iterations per RHS, original-incidence certificate and repair
limit apply. A retained grant remainder is reserved until terminal collection;
joining/refunding and graph totals occur once. Batch completion and failures
drain/drop the task within the total timer. Completed hierarchies are reused.
Numerical result changes are confined to new policies whose installation time
changes; original arms must replay exactly. Independent dense/energy witnesses,
cheap-only exactness, grant partition independence, retained-frontier and
zero-RHS lifecycle tests cover the affected scope. No production code changes.

Before choosing a control-path optimization, a bounded SCC instruction profile
uses the existing frozen `e0155f8b` executable: expander and grid at 4096, one cut
RHS, ordinary full and full-controlled, once each under Callgrind with a 120-second
limit per invocation. These instrumented runs are diagnostic, not wall-time
performance observations; scientific output and isolated allocation are checked.
The prior failed panels and larger-grid supplement remain unchanged.

The instruction diagnostic completed as job 8140049 on `scc-zj4` with clean
scheduler accounting in 58 seconds. Callgrind counted 8,219,911,614 instructions
for expander full-controlled versus 532,634,217 for ordinary full; grid counts
were 2,213,389,887 versus 184,982,721. Prefix admission, wrapper admission and
checkpoint bookkeeping jointly accounted for 80.02% and 78.37% of the respective
controlled totals. Dense numerical factorization accounted for another 16.17%
and 15.42%; controlled heapsort sift for 0.99% and 0.71%. These are whole-process
instruction profiles, including preparation and solving, not elapsed-time claims.

The resulting upstream change is opt-in `BuildControl::group_dense_work()`: eight
consecutive dense inner-product terms retain their original arithmetic order and
128-unit tariff each, with one prepayment of at most 1024 units. Allocation gates
are unchanged. Default primitive mode keeps the same scientific and frontier
signatures; its compiled loop structure changes, so frozen-binary comparisons
are included rather than assuming its timing is unchanged. New `c14`/`c1` arms
combine chunking with persistence; `p14`/`p1` isolate persistence; `full-chunked`
compares complete construction without a cheap track. Per-task boundary and
active-time fields are cumulative, not sums across RHSs. RHS telemetry records
start/end issued and charged work, retained reservation, task identity and pending
phase. New-policy recurrence failures additionally report RHS/track, iteration,
repair count, recursive and true incidence residual, RHS norm and target without
changing the tolerance, repair cap or accepted numerical path.

### Frozen persistent discovery protocol

The panel has eight cells: expander/4096 with four mixed and reversed RHSs,
fringe/4096/four mixed, clusters/4096/one cut, grid/4096 and grid/8100/four mixed,
mosaic/8192/four mixed, and hubs/8192/one random. Twenty-five arms include cheap
diagonal/forest, ordinary full, both old full-upgrade triggers, primitive and
chunked full construction, all twelve q/p/c share policies, and six arms from
the unchanged previous executable (full, full-controlled and four q shares).
Seed 20261019, one warmup and six randomized balanced forward/reverse blocks
give **2600 observations** (200 warmups, 2400 measured). Exact-cell preflights
are separate. Per-invocation timeout is 30 seconds; the internal budget including
preflight is thirty minutes, with a forty-five-minute scheduler request. There
is one attempt, no resampling, and every failure remains evidence.

Smoke requires the eighteen Rust witnesses, all small arm/case observations,
135 legacy numerical replays and eighty exact prior fixed-share control replays.
A whole 28-physical-core node is reserved and verified before/during/after the
serial panel; numeric work and the parked builder share one pinned physical
core. Candidate and frozen previous binaries are compared within that allocation.
A separate one-shot cluster/8192 diagnostic retains numerical failure context;
it does not enter timing selection or authorize a tolerance change.

The descriptive selection screen remains zero numerical failures/timeouts, easy
expander/fringe overhead within `1 + share + 0.05` of the corresponding cheap
solver, and hard grid/cluster time below half of that starter. Report all paired
block ratios, six-block ranges, setup costs, construction completion/retention,
resources and failures. Ranges are descriptive, not confidence intervals. A
passing discovery policy still requires untouched confirmation and end-to-end
coverage before any production promotion; no Mac claim follows from SCC.

Candidate validation: eighteen Rust tests and eleven Python harness tests pass,
along with formatting and strict Clippy. Independent Astra architecture review
found no correctness blocker. Production identity remains unchanged from
`e0f69a8d` (192 modules, 58 artifact inputs). The required repository script again
passes its Python suite then stops at the existing native plugin/source identity
mismatch; no plugin rebuild or production qualification is claimed. CMG's
experimental-component scientific suite and strict Clippy pass at pinned revision
`938b87e5a863ad333870413b61478966c56908b7`.

Review also identified and fixed a preflight deadline gap: the same budget and
isolation checks now run before every preflight and measurement invocation. A
started invocation retains its thirty-second timeout, so exhaustion can overrun
the internal boundary by at most one invocation; no further solve starts.


### Independent weak-cluster coordinate witness

A separate diagnostic copy of candidate `054804e0` captured one rejected Mac
`diagonal-c1` iterate for clusters/8192/four mixed RHSs. Capture occurs only on
the failure path and does not modify the iterate; the frozen SCC executable is
unchanged. Independent symmetric edge-flux evaluation converts the original
binary64 inputs exactly and uses 60- and 100-digit decimal arithmetic. Both
precisions give residual norm `2.9990618778690667e-6`, versus the certificate's
`2.9990618731931064e-6` and unchanged target `9.050966799187809e-7`. The residual's
constant component and RHS sum are zero. Thus the stored iterate itself fails
the residual criterion; certificate-evaluation error and RHS incompatibility do
not explain this failure. This is not a coefficient forward-error bound.

The failure is RHS0's challenger at iteration 21: the third failed certificate
follows two permitted residual replacements. Later RHSs are not reached. The
coordinate range is about 262.2 million and maximum coordinate ULP is `1.49e-8`,
supporting investigation of solution-coordinate accuracy. It does not establish
an unavoidable binary64 floor, identify a defective update, or show that
compensated updates fix it. The SCC failure string matches, which reproduces the
failure signature without establishing cross-platform identity of full iterates.
Any later split/compensated representation must certify the actual flattened
returned vector as well. Independent Astra review found no scientific blocker
in this witness. No tolerance, model, repair limit or production path changes.

### Persistent prototype SCC evaluation (2026-10-16)

Candidate `054804e0`, pinned CMG `938b87e5`, is implemented and pushed on the
experimental branches. [Compact evidence](persistent-discovery.json) retains all
200 arm/cell summaries, individual measured times, complete paired-block times,
failures, identities, accounting, instruction profiles and the coordinate witness.
Raw evidence and the reproducible audit are retained in SCC campaign
`/projectnb/welfgr/fereg/runs/20261016-persistent-cmg-e0f69a8d`, under `measure/`,
`smoke/`, `profile/`, `coordinate-diagnostic/` and `analysis/`; hashes resolve
relative to that campaign. No production integration or Mac performance claim.

Smoke job **8140146** passed all eighteen Rust tests, 125 small observations,
135 legacy replays and eighty prior fixed-share replays, with scheduler
`failed=0, exit_status=0`. The two larger-cluster diagnostic failures were
retained separately. Discovery job **8140234** ran on `scc-zj4` from
13:52:14 to 14:22:16 UTC, using one numeric core in the reserved 28-core node.
It exhausted the predeclared thirty-minute internal budget: `failed=0`,
`exit_status=1`, wall 1802 seconds, CPU 1862.136 seconds including process and
observer work, maximum virtual memory `327.812M` as reported by SGE. This is
**an incomplete experiment**, not a successful discovery or qualification.
No retry, extension, replacement or resampling was performed.

The retained prefix contains **2081/2600 observations**: 200 warmups and 1881
measured solves, plus all 200 separate preflights. Four complete balanced blocks
and part of the fifth are available; 519 observations are missing. Every row
matches the frozen randomized order, and every observed scientific/failure
signature matches its preflight and repeats. All 33 failed panel observations
are the same conservative hubs/8192 memory refusal in the three controlled-full
arms (eleven each, including warmups), with three matching preflight refusals;
no observed numerical failure or solver
timeout occurred in this timed prefix. These refusals do not establish physical
memory exhaustion. The excluded-from-timing clusters/8192 failures remain an
unresolved reliability limitation, not a successful outcome.

The audit verified 47 successful previous-binary arm/cells exactly and one
identical structured failure; controlled/chunked full solves matched ordinary
full scientifically in all seven successful cells. Ninety-two cheap-only cells
matched their starter's science. All 2388 observed successful persistent RHS
records satisfied grant continuity, reservation/refund, iteration and modeled
memory checks. Failed-path joining has separate Rust-test/source evidence, not
an observed failure ledger. The maximum accepted relative residual was
`9.981490464158646e-9`. Source and both executable hashes matched after the run.
Initial physical topology and 173 scheduler/process snapshots showed isolation,
with maximum snapshot gap 10.49 seconds. The budget exit skipped the final
configuration check; that missing prerequisite remains explicit.

The following ratios are medians of the available complete paired cell blocks,
with descriptive ranges; they are not confidence intervals or selection passes.
Incomplete pairs remain recorded but do not become completed pairs; arithmetic
means retain singleton observations and are descriptive only. Other cells
and comparisons remain in the compact evidence.

| Comparison (candidate / comparator) | Paired ratio [range] | Blocks |
| --- | --- | ---: |
| Grid/8100, diagonal persistent / current nonpersistent full-share policy | 0.346 [0.346, 0.347] | 5 |
| Grid/8100, diagonal persistent+chunked / frozen previous full-share policy | 0.218 [0.218, 0.218] | 5 |
| Grid/8100, diagonal persistent+chunked / diagonal PCG | 0.395 [0.390, 0.397] | 5 |
| Grid/8100, diagonal persistent+chunked / ordinary full CMG | 17.028 [16.984, 17.060] | 5 |
| Grid/4096, diagonal persistent+chunked / diagonal PCG | 0.226 [0.225, 0.227] | 4 |
| Expander/mixed, diagonal persistent+chunked quarter share / diagonal PCG | 1.450 [1.420, 1.484] | 5 |
| Expander/mixed, diagonal persistent+chunked full share / diagonal PCG | 2.549 [2.487, 2.630] | 5 |

Persistence works mechanically and can help materially: on grid/8100 the
retained diagonal constructor completes during RHS2 and is reused thereafter;
chunking moves installation from incumbent iteration 1232 to 264 on that RHS.
Forest persistence alone never finishes there, whereas chunking finishes on
RHS4. On grid/4096, forest construction finishes on RHS3 without chunking and
RHS2 with it. Cumulative telemetry is never summed across RHSs.

Grouping dense work reduces complete controlled-full solve time to 0.296–0.437
of the frozen previous controlled-full executable on the six dense-factor cells;
clusters is 0.933. This includes setup and solving, not a pure factorization
measurement. Nevertheless, chunked full remains 2.76–4.74 times ordinary full
CMG across the seven successful cells. The default primitive control path also
**regresses**: candidate full-controlled is 1.125–1.150 times its frozen previous
version on the six dense-factor cells (clusters 1.022). Exact arithmetic replay
does not excuse that slowdown. Keep the upstream change experimental until the
default-path regression is resolved and independently measured.

No policy is selected. The incomplete panel cannot pass the frozen six-block
screen, and every new policy's observed expander/mixed overhead already exceeds
its declared easy-case allowance (quarter share 1.30, full share 2.05). On the
larger grid, ordinary full CMG averages about 0.093 seconds, versus 1.575 seconds
for the combined diagonal policy and 3.992 seconds for diagonal PCG. Delayed
construction sacrifices a large opportunity even when it improves on PCG.
The next useful direction is a cheap, general estimate of setup versus solve
cost that can route early to ordinary full CMG, while preserving the cheap
solver and scientific certificates. It must avoid case-name routing fences.
Separately, recover primitive-control performance and investigate the stored
coordinate failure. These are further experiments, not production changes
justified by the present partial panel.


Independent Astra results review recomputed every retained block time, checked
all raw-file hashes and repeat groups, and confirmed the reported ratios and
lifecycle checks. The same qualitative findings hold in the four common complete
blocks; this diagnostic does not replace or complete the frozen panel. Null
block times explicitly distinguish missing/incomplete evidence from successful
observations through their counts and retained individual times. Documentation
references, finite JSON, artifact-size checks and `git diff --check` pass.
Production identity remains unchanged (192 modules, 58 artifact inputs); prior
runtime evidence and the documented installed-plugin identity blocker are reused.


### Probe-and-commit successor: declared scope and construction prerequisite

The next standalone candidate keeps the ordinary diagonal, forest, full and
old diagonal-to-full arms intact. Its numerical scope is the new selector's
iterates, restart/solver decisions and answers; production remains unchanged.
Original-incidence returned-vector certificates at 1e-8, the shared 16000-step
cap and two residual replacements remain mandatory. Independent witnesses are
existing dense grounded solves, energy/symmetry/positivity, exact static-arm
replay and the retained weak-cluster coordinate witness. No sample, model,
rank, omission or separation semantics change.

Before a selector panel, test one synchronous guarded-full constructor. It
uses the pinned controlled CMG APIs with eight-term dense chunks and only a
cancellation callback. There are no builder threads, grant ledgers, channels,
parking, callback traces or double admission. Requested-storage admission covers
input/output banks, solver scratch, graph tuples and compact banks, complete
CMG construction and workspace. This is a conservative modeled-storage budget,
not an RSS guarantee. Allocator/OS and cleanup latency remain outside the
upstream computational cancellation bound. Typed memory refusal is distinct
from interruption and numeric/overflow/allocation errors.

Frozen construction gate: `guarded-gate`, seed 20261020, full and full-guarded
on expander/4096/mixed4, fringe/4096/mixed4, clusters/4096/cut1,
grid/8100/mixed4, hubs/8192/random1. Ten preflights, ten warmups and six
randomized balanced blocks (120 measured calls), 300-second total budget,
30-second per-call timeout, isolated SCC one-core execution. No replacement
observations. Same ordinary numerical signatures are required on every success;
all refusals remain recorded. The four non-hub cases must have six complete
blocks and median paired setup and complete-solve costs no greater than ordinary
plus max(50%, 1 ms). This is a discovery prerequisite, not qualification. A hub
memory refusal is reported and blocks any full-static claim for that cell; it
is not discarded or relabeled success. A missing or failed gate blocks the
selector timing panel. Boundary cancellation and exact memory admission are
checked separately before timing. If this wrapper fails, retain its evidence
and diagnose the construction path before another declared candidate.


The first gate (job 8144754, source manifest
`33785f88e8f6a73f3ab7fb248f15d9cbed7ae1378df9d81b50e0c8446973559a`)
completed all 130 observations and failed the speed prerequisite. Guarded setup
ratios were approximately 2.11, 1.85, 3.71 and 1.90 for expander, fringe,
clusters and grid. Thirteen hub observations refused the conservative memory
bound. All successful ordinary/guarded science agreed exactly. This result is
retained; the selector panel did not run on that artifact.

The reviewed successor opts into CMG's cooperative prepaid work batching and
interruptible introsort. Its computational cancellation allowance is at most
65535 primitive allowance units, with allocations still checked immediately;
it preserves numeric operation order and default controlled frontiers. The
wrapper edge-count scan now checks cancellation every 64 workers. The second
artifact repeats the same frozen gate and criterion. Successful constructor
comparison is a prerequisite for the selector pilot/panel, not a claim of
production qualification. All five selector arms now include preconditioner and
graph destruction in their complete-solve timer; the first gate retains its
original endpoint.

The selector is `auto-commit`: active nonisolated neighbor-count upper bounds
saturate at three, forest requires at least half <=2 and clique bound <=4 times
incidence, with complete memory admission. It probes at 16 and 24, grants one
8-step extension only when the current residual contraction predicts at most
eight remaining steps, and commits at 24 or 32. It restarts in place without
resetting caps. A memory refusal preserves recurrence and disables all future
build attempts in the batch. Ready CMG is used immediately for later RHSs.
The operator is fixed for a batch; changing-weight reuse is outside this scope.

Frozen selector discovery: five arms (diagonal, forest, ordinary full, original
diagonal-full, auto-commit), fourteen cells in `ROUTER_CELLS`, seed 20261021.
The eight retained persistent-panel cells are supplemented with sparse and
weighted expanders in forward/reverse RHS order and mosaic component-separated
hard-first/easy-later and reversed batches. Sparse expanders add n random edges
to a ring; weighted expanders use three n random edges with log-uniform weights
from 0.1 to 10. These generators are not router features or routing rules.
A separate isolated pilot executes every exact arm/cell twice (preflight and one
observation) under a 300-second budget. The complete panel's budget is frozen
from pilot process costs before submission, capped at 1800 seconds. The panel
has 70 preflights and 910 observations including 70 warmups, six randomized
balanced blocks, 30-second per-invocation timeout, no replacement/resampling.
All observations and failures remain retained. Case/arm scientific signatures
must match preflight and repeat exactly; failed signatures are also checked.
Easy expander/hubs/fringe cells target max(5%,1 ms) overhead against their
appropriate cheap method. Remaining cells target at most twice the fastest
feasible static method, while retaining comparisons against the old trigger.
Report each block's paired excess and descriptive ratio; these discovery targets
do not substitute for untouched confirmation. Weak-cluster/8192 has a separate
mandatory diagnostic across all five arms and remains a reliability limitation
if any candidate fails. No tolerance or cap is changed to obtain success.

The cooperative constructor revision is `65ea7157f0184939d3181511e9a13bb7a60cd3a7` on upstream branch
`codex/cooperative-build-chunks`; only this standalone crate pins it.


The discovery comparison rule is fixed before panel data: expander (both RHS
orders), fringe and hubs compare against diagonal, forest and diagonal,
respectively. Each of the other ten cells compares against the static arm among
diagonal/forest/full with the lowest median of its six complete block geometric
mean times; ties prefer that listed order. A static arm is eligible only if every
preflight, warmup and measured invocation succeeds and its scientific signature
repeats. Any selector failure fails its cell. A missing required block makes a
comparison inconclusive. The static lower envelope is a retrospective comparator,
not a claim that its choices were inferable online or a production memory audit
of the legacy controls. For each easy cell report N-B-max(0.05B,0.001); for
other cells report N-2B, using paired block geometric means. The discovery target
passes only when all six block excesses are <=0. The all-six rule is descriptive
here: blocks share a session, so no independent-session confidence or release
qualification is claimed. Report the old trigger separately on every cell.

A harness review caught a deadline edge case before the second timing gate:
checking only before invocation could accept a final invocation that crossed
the campaign deadline. The repaired runner appends the row, then checks the
budget and observer, including after final configuration. Smoke evidence is
reused because this change affects only timing/preflight control, with focused
runner tests; executable source and solver binary are unchanged.

The pilot-based panel budget is the ceiling to 60 seconds of
`120 + 1.5 * 14 * sum(max(preflight_process, pilot_process))` over the 70
arm/cell pairs. The multiplier covers all fourteen calls per pair in the panel,
with conservative pilot choice and 50% process-cost plus 120-second overhead
headroom. A value exceeding 1800 seconds blocks the frozen panel instead of
silently reducing its observations. This formula is fixed before the pilot.

The second gate, job 8144981, also completed 130 observations and failed:
setup ratios 2.09, 1.83, 3.06 and 1.88 for expander, fringe, clusters and grid.
Its launcher exited 1 at the gate verdict; no selector pilot ran. Successful
science was exact and all thirteen hub refusals remained recorded.

One bounded four-invocation Callgrind diagnosis (job 8144989, 90 seconds per
invocation, no retries) compared ordinary/guarded expander4096/mixed4 and
clusters4096/cut1. All four succeeded with exact cross-arm science. The work
accounting function remained out of line: its 120.87 million exclusive
instructions explained 54% of the 223.74 million extra expander instructions;
32.49 million explained 68% of the 48.06 million extra cluster instructions.
These whole-process instruction observations are not wall-time claims.

The final bounded repair pins upstream
`73e2e43d046250e00319c45863ca609808e52c99`: an always-inlined funded-credit
consumption branch, with the complete prior admission body retained as the
out-of-line slow path. Numerical operations, allowances, callback streams,
allocation gates and errors are unchanged. Exact boundary/error/counter tests
cover primitive/cooperative modes, invalid units after funding, budget tails,
interruption and StopUnused. Primitive-control performance is not claimed
unchanged merely because its boundaries agree. The identical frozen gate is
repeated once on this changed artifact. If it fails, this prototype stops with
its evidence retained and no selector timing panel; there is no further repair
or sampling extension in this experiment.
