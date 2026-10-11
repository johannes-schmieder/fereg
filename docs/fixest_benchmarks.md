# SCC evidence for optimization against fixest

The largest validated gaps are four-million-row dense two-FE PPML and dense
three-FE OLS. Start with FE projection and PPML's strict final solve. Increasing
the worker budget alone leaves substantial gaps. This report preserves the
completed October 10 campaign and the later completed phase probe; it does not
claim a new implementation or a universal speed advantage.

## Retained results and provenance

The full campaign is `20261010T192805Z-ols-repair-e29164e6`, source
`e29164e672af026634dab3f26acadd77d27f5313`, Linux plugin SHA-256
`57cc9789efd6ecc810d6dd82c55822dac3bd537bd7be32bb7a4bcd82e2a574a1`.
It used fixest 0.14.2. The subsequent diagnostic job `8022196` used the same
plugin and nine of the campaign's actual frozen datasets.

The command stopwatch includes estimation, inference and requested individual
FE recovery, including fixest's lazy extraction and observation-level
materialization. Input generation/import, small initialization and disk export
are excluded. PPML covariance conventions are aligned inside timing as described
in the [benchmark protocol](../bench/README.md#packed-scc-comparison). Each
production task reserved all 28 physical cores of a 28-core node, ran fits
sequentially and bound their affinity. Reported workers are native/fixest worker
budgets; licensed Stata itself was capped at four processors.

| Committed evidence | Contents |
| --- | --- |
| [Configuration summary](../bench/results/scc-20261010-summary.csv) | All 164 arm cells, medians, ranges, eligibility, failure reasons, paired ratios, host and sampled memory; byte-identical to the SCC report. |
| [Observations](../bench/results/scc-20261010-observations.csv) | All 492 planned fit positions, including skipped and failed fits, command/process times and memory sweep counts; byte-identical to the SCC report. |
| [Inputs and identity](../bench/results/scc-20261010-inputs.json) | Full source/artifact hashes, six compact scheduler records, campaign policy and configurations, dataset topology and numeric-column hashes, validation outcomes. |
| [Phase diagnostics](../bench/results/scc-20261010-diagnostics.json) | All 32 later probe fits, timings, iteration counts, phase measurements, numerical comparison results, source/input hashes and successful scheduler accounting. |
| [General scaling diagnostics](../bench/results/scc-20261011-scaling-diagnostics.json) | The 32 holdout fits, six CPU-sampled fits, independent weak-network OLS witness, strict-reference outcomes, including timeouts, and full input/capture/accounting hashes. |

Full raw captures and the previously reviewed 19-page PDF remain on SCC under
`/projectnb/welfgr/fereg/runs/20261010T192805Z-ols-repair-e29164e6/`.
The probe remains under
`/projectnb/welfgr/fereg/dev/20261011-fixest-general-scaling/`.
The retained JSON binds their files by full SHA-256. No dataset, raw log, binary
or local temporary path is retained in this commit.

Collection on October 10 reverified the frozen runtime, source and result hashes,
all six successful campaign accounting records, application sentinels and all
492 manifest positions. Recomputing the summary reproduced all 164 rows (allowing
only CSV float parsing roundoff). Native repeatability and cross-worker checks
pass on 55 completed configurations; the remaining native configuration timed
out. There are 131 eligible, 25 descriptive-discrepancy and eight untimed arm
cells. All 11 native small-design oracles pass. Reference smoke failures remain
visible: default fixest passes 2/11, strict fixest 5/5, reghdfe 1/6 and ppmlhdfe
1/5. Production eligibility is the declared comparison gate, not a claim that
every reference has passed every independent oracle.

The later holdout `8022206` and CPU probe `8022215` are now complete. Their
generalization evidence and limits are described below; the original campaign
and phase-probe files above remain unchanged.
The existing [validation report](VALIDATION.md#packed-scc-campaign-after-the-ols-repair)
records the preceding repair and cross-campaign input audit.

## Where fixest remains faster

Among eligible default-fixest comparisons, fereg is faster in 16/18 OLS and
10/17 PPML configurations. The other 21 default-fixest configurations have
discrepancies or unavailable results and supply no valid speed ratio. These
counts describe this panel, with repeated workload variants, not a probability
of winning on an arbitrary model.

The following table contains every eligible configuration where fixest wins.
Times are separate three-fit command medians in seconds. Required reduction is
`1 - median(fixest) / median(fereg)`: the approximate reduction in fereg elapsed
time needed to tie this observation. It is not a promised gain or confidence
bound. The CSV's `speedup` instead means median **paired** `fixest / fereg`, so
it need not equal the ratio of these separate medians.

| Model / design | Rows | Workers | fereg s | fixest s | Reduction to tie |
| --- | ---: | ---: | ---: | ---: | ---: |
| PPML, two dense FEs | 4,000,000 | 4 | 32.107 | 16.121 | 49.8% |
| OLS, three dense FEs | 4,000,000 | 4 | 8.742 | 4.667 | 46.6% |
| PPML, two dense FEs | 1,000,000 | 16 | 4.884 | 3.054 | 37.5% |
| PPML, two dense FEs | 1,000,000 | 4 | 6.479 | 4.132 | 36.2% |
| OLS, three dense FEs | 1,000,000 | 4 | 1.874 | 1.371 | 26.8% |
| PPML, three dense FEs | 4,000,000 | 4 | 25.817 | 21.526 | 16.6% |
| PPML, three dense FEs | 1,000,000 | 4 | 6.164 | 5.208 | 15.5% |
| PPML, one FE | 4,000,000 | 4 | 13.779 | 12.998 | 5.7% |
| PPML, AKM | 1,000,000 | 16 | 4.966 | 4.698 | 5.4% |

All rows use 16 regressors, unit weights and no requested FE output; OLS uses
IID inference and PPML uses robust inference. Dense two-FE topology at four
million rows has 1,000 levels per effect; dense three-FE topology has 99 levels
per effect. The input JSON preserves exact topology and hashes. The two largest
gaps also have separated observed timing ranges: dense PPML 32.101–32.155 versus
16.117–16.165 seconds, and dense OLS 8.515–8.805 versus 4.588–4.697 seconds.
Three repetitions do not establish a narrow uncertainty bound for small gaps.

## Scaling and memory constraints

On the same AKM inputs within each model's assigned node:

| Workers | OLS fereg s | OLS fixest s | PPML fereg s | PPML fixest s |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 2.232 | 3.762 | 12.807 | 19.444 |
| 4 | 1.292 | 1.861 | 6.417 | 7.905 |
| 8 | 1.043 | 1.511 | 5.324 | 5.879 |
| 16 | 0.955 | 1.325 | 4.966 | 4.698 |

The OLS repair is already present: 8→16 workers saves only 8.4% for OLS and
6.7% for PPML. Four-to-sixteen-worker dense two-FE PPML improves only from
6.479 to 4.884 seconds, while fixest improves from 4.132 to 3.054. General
stage-level worker policy and memory traffic merit investigation; these timings
alone do not identify a hardware bottleneck.

Memory also distinguishes the priorities. Four-million-row dense three-FE OLS
uses 3.856 GiB sampled peak RSS versus fixest's 2.924 GiB. Dense two-FE PPML
uses 4.239 versus 5.295 GiB: its timing deficit does not imply a larger RSS.
The maximum resolved native RSS is 5.032 GiB on four-million-row sparse two-FE
PPML. These are sampled estimator-process-tree peaks, not exact allocation
peaks; unresolved cells stay empty/NR. OLS and PPML AKM run on different hosts,
so compare worker scaling within each model rather than across hosts.

## Later phase probe: useful directions, limited timing evidence

Job `8022196` completed all 32 fits on `scc-zj4`, a Xeon Gold 6132, reserving
28 physical cores. Accounting has failed=0/exit=0, 284 seconds wall time and
6.629G maxvmem for the whole job. Frozen scripts, plugin/input hashes and the
completion sentinel pass. All recorded coefficient, covariance, objective and
discrete checks pass; all four repeated fixed-worker comparisons are exact.
These checks are narrower than the full campaign's sample/output checks.

Tracing was enabled; most configurations have one fit and selected ones two.
The small warm-up is excluded from the retained last phase occurrence. Reported
phases can nest: do not sum them as a disjoint elapsed-time decomposition.

| Observation | Implication for investigation |
| --- | --- |
| Four-million-row dense three-FE OLS, four workers: 8.356 s command; projection 4.889 s, finite solve 1.279 s, residual absorption 0.415 s. | FE projection is the first target, with finite products and residual work secondary. |
| Same OLS design at one million rows: auto 1.791/1.814 s, explicit MAP 1.537 s, explicit LSMR 1.796 s, fixest 1.383 s at four workers. | Test mathematically general topology/solver choices and setup costs. One MAP observation does not justify a default switch. |
| Four-million-row dense two-FE PPML, four workers: 28.200 s command; reported FE projection 11.437 s, strict final solve 7.580 s, finite solve 2.090 s, inference 0.200 s. | Optimize projection and strict-final work before covariance assembly; preserve the strict final certificate. |
| Same PPML input at sixteen workers: 22.629 s; projection 8.705 s and strict final solve 6.446 s. | More workers help modestly; inspect ordered reductions, data movement, reuse and serial work. |
| Million-row dense two-FE PPML, four workers: auto 6.280/6.231 s, MAP 6.524 s, LSMR 6.194 s, CMG 8.197 s; fixest 4.150 s. | A simple solver override does not close the observed gap. CMG's finite-solve field rises to 2.520 s versus auto's 0.547 s. |
| Million-row sparse two-FE PPML: nine outer / 2,273 inner iterations, 26.384 s command, 14.962 s FE projection and 7.921 s strict final solve. | Retain as a difficult-solver control so dense-model improvements do not trade away sparse convergence. |

The probe uses frozen campaign bytes and the same binary, but some inputs were
originally timed on other hosts. Its elapsed times must not be interpreted as
improvements over the full campaign. It contains no candidate implementation.

## Generalization follow-up

The same qualified plugin was tested on 32 new fits: four and 28 workers,
one and sixteen million rows, four and 64 finite regressors, fixed-cardinality
three-FE designs, highly skewed group sizes and a weak worker-firm network.
Job `8022206` completed on `scc-zj4` with failed=0/exit=0, 519 seconds wall
time and 13.340G whole-job maxvmem. Each input was frozen and checked after
Stata-format round-trip; reference imports verified every numeric-column hash.
These are single diagnostic fits, not confidence-bounded performance claims.

| New input | Workers | fereg s | fixest s | fereg / fixest |
| --- | ---: | ---: | ---: | ---: |
| OLS, 1m rows, 4 controls, three FEs with 64 levels each | 4 / 28 | 0.862 / 0.671 | 0.601 / 0.593 | 1.43 / 1.13 |
| OLS, 16m rows, same FE cardinalities and 4 controls | 4 / 28 | 20.441 / 11.758 | 7.427 / 6.829 | 2.75 / 1.72 |
| PPML, 16m rows, same FE cardinalities and 4 controls | 4 / 28 | 63.258 / 54.484 | 31.992 / 25.888 | 1.98 / 2.10 |
| OLS, 1m rows, skewed two-FE groups, 4 controls | 4 / 28 | 0.591 / 0.565 | 0.550 / 0.554 | 1.07 / 1.02 |
| PPML, 1m rows, skewed two-FE groups, 4 controls | 4 / 28 | 3.171 / 2.749 | 2.267 / 2.105 | 1.40 / 1.31 |
| OLS, 250k rows, three FEs, 64 controls | 4 / 28 | 1.848 / 1.166 | 1.459 / 0.971 | 1.27 / 1.20 |

The twelve comparisons above pass the captured coefficient, covariance,
objective, retained-count, omission and FE-degree checks. Native cross-worker
checks pass on all eight new inputs. The four weak-network reference fits are
excluded from these ratios: their reported FE counts disagree with fereg, and
default OLS also fails coefficient/objective agreement. Operational completion
does not turn those disagreements into passing comparisons.

An independent weak-network OLS audit eliminates worker effects, solves a
grounded sparse firm Schur system with SciPy direct LU, reconstructs row
residuals, and solves the four-control regression. It finds 282 graph components,
FE rank 52,218 and residual DoF 947,778. Fereg's largest slope difference is
1.11e-13 and RSS differs by 7.22e-9; slope covariance and DoF checks pass.
Maximum reconstructed worker/firm scores are 1.69e-14/8.39e-12. This independent
double-precision witness supports this case, not a high-precision proof or a
PPML oracle. Default fixest instead reports 52,499 FE parameters and its OLS
RSS differs from fereg by 0.1524. Tightening its FE tolerance to 1e-11 removes
the coefficient/objective disagreement; convergence and FE-counting options
must be assessed separately. Adding `ssc(K.exact=TRUE)` also restores the OLS
covariance degrees of freedom and brings its largest covariance difference
down to 3.45e-18. The raw `fit$nparams` count stays approximate even with that
option, so the audit retains it separately from the covariance's actual DoF.
These stricter reference arms ran on another node; their times are not paired
speed comparisons with the original holdout. Both strict PPML reference arms
hit the predeclared 120-second process cap and remain unavailable. Audit
`8022296` completed with failed=0/exit=0, 318 seconds and 1.947G maxvmem;
that means the bounded audit completed, not that its timed-out references passed.
The initial launcher `8022280` exited 1 before the oracle because a reused
whole-node CPU helper rejected its four-slot allocation. The narrow affinity
selection repair and failed receipt are retained. No estimator changed.

The sixteen-million-row OLS projection takes 12.842/5.173 seconds at four/28
workers. The regressor solves take three iterations, versus four at one million
rows, so growing iteration counts do not explain the worse per-row cost.
The FE cardinalities stay below the 256-slot preconditioner boundary in this
holdout. Data traversal, intermediate storage and parallel granularity remain
concrete targets; these observations alone do not distinguish cache capacity,
bandwidth and address-gather effects. PPML projection takes 26.345/23.286 seconds
and preparation 8.889/8.587 seconds, explaining why extra cores alone help little.
Sampled native peak RSS reaches 6.422 GiB for OLS and 10.614 GiB for PPML;
corresponding reference peaks are 4.796 and 9.446 GiB. Sampling can miss peaks.

## Measured causes and general remedies

Six further fits in job `8022215` collected user-cycle function samples at
199 Hz with the unchanged plugin. Accounting is failed=0/exit=0, 90 seconds,
6.757G maxvmem; all six captures and scientific comparisons pass, with no lost
samples reported. Samples cover the entire process, including import and warm-up,
whereas the command clock excludes those operations. Percentages are CPU-cycle
shares, not elapsed-time fractions; inlined work in Rayon symbols must not be
misreported as thread-scheduler overhead.

**Dense two-FE PPML constructs expensive weighted geometry serially.** In
[canonical products](../crates/fereg-core/src/ols/canonical_products.rs), the
builder aggregates FE-pair weights and visits every pair of neighbours of each
eliminated group. The fully crossed 500-by-500 design requires 62.375 million
pair visits per frame; 1,000-by-1,000 requires 499.5 million. Thus this setup work
grows about eightfold when the campaign row count grows fourfold. Solver setup
increases from 1.260 to 8.875 seconds at four workers, and remains 8.962 seconds
at sixteen workers on the larger input. These aggregate setup clocks include
other work and nest within the fit phases.

The builder accounts for 17.04%/16.07% of whole-process sampled user cycles at
four/sixteen workers. Instruction annotation places 96.33% of its four-worker
local cycle weight in the neighbour-pair loop, including guarded weight arithmetic and
four matrix updates. This supports the serial clique-assembly diagnosis. These
frames report `iterative=1, implicit=0`: iterative solving skips factorization
but still constructs the dense matrix.

The strongest scalable experiment is a general choice between explicit and
implicit weighted geometry based on setup plus predicted solve work and memory.
The existing implicit operator avoids pair expansion; it is currently selected
by storage pressure. Its admission also needs independent costing, since an
earlier gate uses the dense-pair work estimate. Another option is matrix-row
ownership with ordered contributions, parallelizing dense assembly without
unordered reductions. That retains quadratic neighbour-pair work and needs
additional admitted edge storage, so it addresses core scaling more than
asymptotic growth. Both need fresh original-input certificates; implicit
subtraction and any changed arithmetic require declared scope and cancellation,
weak-network and unequal-weight witnesses. Removing all nine setup seconds is
an unattainable upper bound, and even that alone would not erase the entire
observed PPML gap.

**OLS needs cheaper projection and less repeated row traversal.** On the
four-million-row dense three-FE case, sampled auto/MAP command times are
8.024/6.262 seconds at four workers and 5.405/4.095 at sixteen: about 22%/24%
less time for MAP, with scientific agreement. No default was changed. A general
choice based on geometry, observed contraction and total work is preferable to
a blanket MAP switch: the sparse controls require many more iterations.
The primary four-million-row fit spends 4.889 of 8.356 seconds in projection.
Samples identify `within` gathers and weighted column norms as substantial
consumers. Retaining OLS setup alone has a much smaller ceiling: final-residual
setup is about 0.125 seconds, roughly 1.5% of that fit.

The pinned `within` batch path currently solves separate right-hand sides and
copies between row and column representations. Shared traversal across bounded
column tiles, caller-owned outputs, retained immutable FE topology and fewer
duplicate norm/validation passes are general candidates. Reuse numerical
weighted geometry only while its weights and accepted frame remain valid;
PPML changes them. Preserve each certificate's information and original-data
authority when fusing passes. Changing reduction order is a numerical change,
not an arithmetic-preserving optimization.

**Some limits are structural, but the whole gap is not.** Stata SPI loading
stays on its main thread; low-regressor column parallelism cannot occupy all
cores; sparse gather locality and memory bandwidth eventually constrain both
packages. Fereg additionally requires independent original-input certificates,
stable rank/sample decisions and a fresh strict PPML final solve. Fixest uses
accelerated demeaning and different stopping/counting policies (pinned sources:
[demeaning](https://github.com/lrberge/fixest/blob/v0.14.2/src/demeaning.cpp),
[estimation](https://github.com/lrberge/fixest/blob/v0.14.2/R/estimation.R),
[covariance adjustments](https://github.com/lrberge/fixest/blob/v0.14.2/R/VCOV.R)); it also passes
the dense comparisons above, so its advantage cannot simply be dismissed as
inaccuracy. Keep the scientific contract and reduce the work needed to meet it.
At 100 million rows and 16 regressors, one double-precision regressor bank alone
is 12.8 GB, before outcomes, FE codes, Stata storage or solver workspaces.
Bounded column windows and memory traffic matter more there than another small
thread-count adjustment. No experiment here tests 100 million rows, more than
28 cores, arbitrary weights, every slope-FE design or every graph.

## Code areas and safeguards for the next changes

1. **Dense PPML projection and strict final work.** Inspect
   [prepared IRLS](../crates/fereg-core/src/ppml/prepared_irls.rs), the
   [linear session](../crates/fereg-core/src/ols/linear_session.rs), and
   [borrowed FE operations](../crates/fereg-core/src/fe/borrowed_map.rs).
   Measure repeated setup, weight-dependent rebuilds, projection/certificate
   passes and retained-buffer traffic. Only reuse work when its sample,
   topology, weights and accepted-frame dependencies still hold.
2. **Dense three-FE OLS projection and memory.** Inspect the
   [fit lifecycle](../crates/fereg-core/src/ols/prepared_fit/one_shot.rs),
   [FE operations](../crates/fereg-core/src/fe/borrowed_map.rs) and pinned
   `within`/`schwarz-precond` gather, sparse-product and setup paths described in
   [architecture](ARCHITECTURE.md). Follow dependency changes with a pinned
   revision bump. Retain sparse and high-worker controls before changing a
   general solver policy; do not dispatch on benchmark names or sizes.
3. **Scaling after the substantive gaps.** Inspect
   [worker planning](../crates/fereg-core/src/parallel.rs), deterministic
   reductions and phase-specific memory bandwidth. Preserve private norm
   accumulators from the existing repair. Profile before assuming thread-launch
   overhead or false sharing explains the remaining limit.

The four-million-row three-sparse PPML case is a separate completion problem:
fereg, fixest and ppmlhdfe each hit the 600-second command timeout. Native rows
after the first failure were skipped. It is not a timed fixest win or loss.
Default fixest's sparse comparisons fail absorbed-DoF checks, with additional
OLS coefficient/covariance/objective discrepancies in some cells. Its saved-FE
comparisons also have prediction failures; two strict-profile saved PPML cases
pass. Preserve those failure flags and keep strict/default profiles separate.

Use the committed gap rows as a short profiling panel, then validate on different
seeds/topologies and the unchanged standard workloads. Before each experiment,
declare artifacts, workers, repeats, timeout and stopping budget. Keep phase
tracing out of qualification timing. Declare affected numerical fields and
independent witnesses before arithmetic, algorithm or storage changes, following
[development](development_workflow.md) and [SPEC](SPEC.md). Do not loosen
tolerances or alter sample, rank, omission or separation semantics to match a
reference. No result here replaces the required Mac timing and science checks.

## Reading and reproducing the retained tables

`seconds` is populated when all three commands have successful captures, even
if their scientific comparison fails; it must be read with `eligible`.
`observed_seconds` also permits a completed command with a capture error.
`timing_discrepancy` marks such descriptive, ineligible timings. A paired speed
ratio requires three scientifically eligible pairs. Blank ratios and memory
remain unavailable, never zero. `wall_seconds` in observations includes process
overhead and differs from the timed estimation command.

The maintained [collector and report](../bench/README.md#packed-scc-comparison)
can recollect the remote run using its `collected/context.json`. Run the report's
`load_campaign` and `summarize` to reproduce the summary, and verify the committed
CSV hashes against `artifacts` in the inputs JSON. Dataset generation uses
transcendental functions: reuse the frozen inputs for exact A/B comparisons, or
verify every numeric-column hash rather than relying on the seed alone.
