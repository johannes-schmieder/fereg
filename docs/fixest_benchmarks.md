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

At the collection cutoff (22:39 EDT), the subsequent holdout `8022206` was
queued and CPU probe `8022215` held. Neither is included as completed evidence.
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
