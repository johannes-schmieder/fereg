# SCC evidence for optimization against fixest

The October 12 rerun has not closed the largest gaps: four-million-row dense
two-FE PPML takes 26.767 seconds against fixest's 15.068, and dense three-FE OLS
takes 8.253 against 4.224. The PPML relative gap is smaller than in the October
10 campaign; the OLS gap is slightly larger. Every corresponding configuration
ran on a different host between campaigns, so these observations do not isolate
the optimizations' effect from hardware. This report retains both campaigns
and the intervening diagnostics, without claiming a universal speed advantage.

## Optimized rerun, October 12

Campaign `20261012T020354Z-optimized-d9865e57` froze source
`d9865e57e07905750b47d1f12c573e4b4e11ae13`, containing qualified B, C/D and E1;
the stopped A1/A2/A3/F2 experiments were not integrated. The Linux plugin is
SHA-256 `75faa55005e95787d8b1a711af738340c36bcf97205c6c8bde1d977aaa41ae23`.
Fixest remains 0.14.2. All 36 production datasets reuse the October 10 bytes,
with matching decoded numeric-column hashes and topology. The same 56 model
configurations, three repetitions, worker budgets, timing boundary, scientific
tolerances and per-fit limits apply. The added strict OLS profile expands the
panel from 492 to 579 planned fit positions; default and strict results remain
separate.

Among eligible default-fixest comparisons, fereg is faster in **15/18 OLS and
11/17 PPML** configurations: **26/35 overall**, unchanged from October 10's
16/18 and 10/17. The other 21 default comparisons remain scientifically
ineligible or unavailable. Strict fixest gives 19/22 OLS and 4/4 PPML native
wins among eligible comparisons. These counts describe this panel, including
repeated workload variants; they are not general probabilities of winning.

The table revisits all nine previous losses. Times are current separate
three-fit command medians. The ratio is the median **paired fixest/fereg**
time, so a value above one favors fereg; it need not equal the ratio of the
separate medians. Both arms run on the same node within each campaign.

| Model / design | Rows | Workers | Current fereg s | Current fixest s | Previous paired ratio | Current paired ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| PPML, two dense FEs | 4,000,000 | 4 | 26.767 | 15.068 | 0.502 | 0.563 |
| OLS, three dense FEs | 4,000,000 | 4 | 8.253 | 4.224 | 0.530 | 0.510 |
| PPML, two dense FEs | 1,000,000 | 16 | 4.724 | 3.260 | 0.625 | 0.692 |
| PPML, two dense FEs | 1,000,000 | 4 | 6.073 | 4.398 | 0.645 | 0.724 |
| OLS, three dense FEs | 1,000,000 | 4 | 2.875 | 2.048 | 0.745 | 0.710 |
| PPML, three dense FEs | 4,000,000 | 4 | 23.230 | 21.166 | 0.836 | 0.910 |
| PPML, three dense FEs | 1,000,000 | 4 | 5.769 | 5.149 | 0.845 | 0.893 |
| PPML, one FE | 4,000,000 | 4 | 22.952 | 25.060 | 0.943 | 1.092 |
| PPML, AKM | 1,000,000 | 16 | 4.670 | 4.403 | 0.958 | 0.954 |

One of those nine losses becomes a native win; eight remain. A new small
observed loss is million-row one-FE OLS at four workers: 1.476 versus 1.409
seconds, paired ratio 0.955. Its native range is 1.314–1.783 seconds, overlapping
fixest's 1.406–1.411; three repetitions do not establish a reliable small gap.
The two largest gaps remain well separated: dense PPML 26.269–26.852 versus
15.028–15.440 seconds, and dense OLS 8.217–8.322 versus 4.190–4.257. Matching
these separate medians would require approximately 43.7% and 48.8% reductions
in native command time, respectively. Strict OLS fixest also wins the latter
case, at 5.272 seconds with paired ratio 0.639.

The new phase observations still point to substantive solver work. Four-million
row dense PPML reports median FE projection of 10.309 seconds and strict final
solve of 7.000 seconds. Dense three-FE OLS reports input preparation of 1.071
seconds and native work of 7.025 seconds. Phase clocks may nest and must not
be summed. Their differing hosts and instrumentation prevent a direct causal
comparison with the earlier phase probe.

All **168 native fits pass**, including three completions of four-million-row
sparse three-FE PPML, which previously timed out. Its native median is 278.480
seconds, range 186.330–282.094; fixest and ppmlhdfe time out, so there is no
eligible speed ratio. Native exact repeatability passes in all 56 configurations,
and cross-worker scientific and discrete checks pass. All 165 completed
historical native counterparts agree on sample/discrete fields and pass the
coefficient, covariance and objective tolerances. That historical check does
not compare row-level saved fitted arrays; each campaign's own requested-output
checks remain in force.

Setup `8028839`, smoke `8028840` and array `8028841` tasks 1–4 all have
`failed=0`, `exit_status=0`, complete accounting and application sentinels.
Production tasks used 28 slots on 28-core nodes `scc-wo4`, `scc-yj3`, `scc-wk3`
and `scc-wj1`, with sequential affinity-bound fits. They took 76–115 minutes;
whole-job maximum virtual memory was 7.168G–10.695G in SGE accounting. Build/ABI/load/numeric
guards, 42 benchmark tooling tests and all 11 independent native smoke oracles
pass. Reference smoke oracle results remain explicit: default fixest 2/11,
strict fixest 10/11, reghdfe 1/6 and ppmlhdfe 1/5. Strict fixest's remaining
smoke failure is clustered OLS covariance. Production eligibility does not
assert that every comparator passes every independent oracle.

There are 193 arm cells: 154 eligible, 32 with descriptive but ineligible
timings and seven without a completed timing. No failed or skipped observation
was dropped and no scientific gate was relaxed. This is descriptive Linux
evidence, not quiet-Mac release performance qualification.

| Retained rerun evidence | Contents |
| --- | --- |
| [Configuration summary](../bench/results/scc-20261012-summary.csv) | All 193 arm cells, medians, ranges, scientific eligibility, paired ratios, hosts and sampled memory. |
| [Observations](../bench/results/scc-20261012-observations.csv) | All 579 planned positions, failures/skips, command/process times and available native phase clocks. |
| [Historical comparison](../bench/results/scc-20261012-comparison.csv) | All 56 default-fixest configurations, both campaigns' hosts and times, eligibility and paired ratios. |
| [Inputs, identity and checks](../bench/results/scc-20261012-inputs.json) | Dataset identities, source/plugin/launcher hashes, six scheduler records, smoke and production comparisons, repeat/worker verdicts, all 165 historical science checks and capture hashes. |

The raw captures, compact collection and retained summaries remain on SCC under
`/projectnb/welfgr/fereg/runs/20261012T020354Z-optimized-d9865e57/`.
All October 10 evidence below remains unchanged.

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

## October 10: where fixest remained faster

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

H6 item 1 avoids same-call hint rescans by keeping early OLS FE columns owned
until authoritative preparation. Full unweighted transfers reuse their exact
encodings; weighted/classified, borrowed and PPML hints remain checked. All
73 exact replay cases, 292 worker checks and seventeen licensed fixtures pass.
The owner costs 56 descriptor bytes per early source on the Mac, with no raw
buffer copy and unchanged moment-cache admission; 275 production lines and
five dispatch candidates are added. The initial weighted-refactorization cost
was caught and removed before integration. Its timing effect remains unmeasured
until the H boundary. See [validation](VALIDATION.md#h6-owned-load-hints-2026-10-12)
and [compact evidence](../bench/results/h-20261013-h6-load-hint.json).

H6 item 4 removes a repeated initial-OLS finite scan by preserving the exact
RHS builder's immutable owner. Raw, final-residual and PPML checks remain.
All 73 exact plugin cases, 292 worker checks and seventeen licensed fixtures
pass with unchanged science and solver signatures; focused witnesses also
compare checked/raw projection and diagnostic ordering. The change adds 52
production lines but no allocation. Its effect on the fixest gap is unmeasured
until the full H Mac panel. See
[validation](VALIDATION.md#h6-checked-initial-rhs-2026-10-12) and
[compact evidence](../bench/results/h-20261013-h6-finite-inputs.json).

H6 item 3 removes repeated membership validation after checked single-source
transfer and during absorbed DoF. Immutable ownership preserves raw-adapter
checks and foreign-owner rejection; rebuilt interaction validation keeps its
original scratch lifetime. All 73 exact plugin cases, 292 worker checks and
seventeen licensed fixtures pass. An early scratch-lifetime regression was
caught in review and repaired; no timing claim precedes the full H Mac panel.
See [validation](VALIDATION.md#h6-membership-validation-reuse-2026-10-12) and
[compact evidence](../bench/results/h-20261013-h6-membership.json).

H6 item 2 removes the serial complete-sample validation bottleneck using the
existing worker policy, and row-partitions finiteness checks even for narrow
models. All 73 exact plugin cases, 292 worker checks and ten licensed fixtures
pass with unchanged solver signatures. Malformed inputs retain the old error
order after an additional shortcut probe. Timing remains pending the whole-H
Mac boundary, so this is not yet a measured reduction in the fixest gap. See
[validation](VALIDATION.md#h6-sample-scans-2026-10-12) and the
[compact record](../bench/results/h-20261013-h6-sample-scans.json).

H5's socket/NUMA-controlled AKM 1M panel uses the same mask and memory nodes
for fereg and fixest on one node. OLS speedups at 4/8/16 workers relative to
one worker are 2.083/2.851/3.055 for fereg and 1.937/2.427/2.762 for fixest;
PPML gives 2.054/2.554/2.441 and 2.381/3.263/3.923. Native PPML still slows
4.64% from eight to sixteen workers, from 4.248 to 4.445 seconds. All 48 fits,
24 scientific comparisons, sixteen fixed-worker repeat groups and both tools'
cross-worker checks pass without retries. The tooling now groups allowed
physical cores by socket and binds memory explicitly; estimator inputs,
tolerances and production artifacts are unchanged. These are descriptive
same-node speedups, not a controlled old/new placement comparison. See
[all command times and checks](VALIDATION.md#h5-socket-and-numa-placement-2026-10-12)
and [compact evidence](../bench/results/h-20261013-h5.json).

H4's bounded overlap check does not reproduce load inflation: in all five
four-worker pairs, overlap-on load is lower, averaging 299.695 versus
311.857 ms (paired-median reduction 15.219 ms). Sixteen-worker load similarly
averages 273.927 versus 287.376 ms; the one-worker negative control is mixed
within noise. Native on/off means at four/sixteen workers are
0.906805/0.900010 and 0.464803/0.463340 s, so the small native costs remain
visible. All fifteen comparisons and six fixed-worker repeat groups retain
exact captured science and solver signatures. The declared inflation rule
fails; the earlier C1 clock pattern is attributed to host variability for
this check, and production is unchanged. See the
[RRUNTIME table](VALIDATION.md#h4-overlap-check-no-load-inflation-2026-10-12)
and [all thirty observations](../bench/results/h-20261013-h4.json).

H2's approved dense three-FE MAP prototype passes its independent science,
replay and transaction checks, but fails the sparse-cost requirement: all six
sparse geometry observations are 29.8–30.9 ms against the declared 21 ms cap.
It is retained as a checked patch and **not adopted**; production stays at the
H8 route. No fit is retried. Dense 4M native means fall 22.806 → 16.315,
7.407 → 5.981 and 4.372 → 3.712 seconds at 1/4/16 workers; 1M changes are
-1.70%/-3.52%/+3.05%. Sparse remains on LSMR with exact captured science and
unchanged solver signatures. At 4M/four workers command time is 8.684 → 7.244 s,
leaving 3.020 s relative to fixest's retained 4.224 s (different host/source).
Two pairs per cell are descriptive rather than a release performance claim.
The geometry cost, candidate-copy/workspace cost, small-input result and failed
gate are retained in [validation](VALIDATION.md#h2-dense-three-fe-prototype-sparse-cost-gate-failed-2026-10-12),
[all observations](../bench/results/h-20261013-h2.json) and the
[prototype source](../bench/results/h-20261013-h2.patch).

Workstream H8 makes ordinary two-FE OLS aim at the shared hundredth-request
recurrence target while preserving the requested certificate, PPML and final
projection policies. The frozen 48-fit SCC panel reports the accuracy cost:
sparse 1M native time rises 13.31%/12.17%/10.95% at 1/4/16 workers, with main
iterations 92 → 120 and final iterations 11 → 0. AKM main iterations rise
14 → 18; its costs rise 5.71%/9.26%/4.68%. Dense iterations remain one; its
observations range from -11.08% to +5.06%, without a confidence-qualified gain
claim. All paired science/discrete checks and fixed-worker repeats pass.
The over-ten-percent sparse costs were reported before integration, and the
approved rule is kept unchanged. Retained F1 QR checks pass, while the unchanged
strong-F witness still fails (barbell 2.480x, weak 1.076x the bound); the weak
F error improves from 1.440e-5 to -2.603e-8. H8 does not answer the stronger
certificate question. See [validation](VALIDATION.md#h8-recurrence-consistency-with-measured-sparse-cost-2026-10-12)
and [all bounded observations](../bench/results/h-20261013-h8.json).

Workstream H3's preserving row-owned assembly reduces dense two-FE PPML
native time from 24.615 to 19.401 seconds at 4M/four workers and from 19.540
to 11.636 at 4M/sixteen workers. At 1M the corresponding changes are
5.303 to 4.900 and 3.991 to 3.075 seconds. Two alternating pairs per cell
give reductions of 21.2%/40.5% at 4M and 7.6%/23.0% at 1M. One-worker changes
are −0.1%/0.8%, below the allowance; repeated neighbour scans and two pair
evaluations are retained costs. All twelve A/B comparisons have exact captured
science and unchanged solver signatures. Raw matrix equality on A3 kernel
geometries and the hash-identical 1M/4M inputs preceded timing. The older A3
current/implicit sixteen-worker observations were 3.806/2.721 seconds at 1M
and 18.427/10.896 at 4M. Those use a different host/source/harness and are not
pooled with this panel; implicit routing and its cost-model calibration remain
unchanged. The [validation record](VALIDATION.md#h3-ordered-parallel-assembly-2026-10-12)
and [compact evidence](../bench/results/h-20261013-h3.json) retain all costs,
checks and attempts. Standard Mac timing is still due at the H boundary.

Workstream H1's bounded follow-up on 2026-10-12 refutes the proposed
locality-sort explanation under its predeclared 20% gate. On the same frozen
16M fixed64 input, pre-sorting by the selected dominant factor `g3` changes
projection from 11.790 to 12.439 seconds at four workers and from 4.735 to
4.805 at 28 workers. The 1M controls improve only 9.2%/12.3%. Sorting permutes
sample order: coefficients/covariance pass SPEC tolerances and every captured
discrete model and indexed solver decision is identical, including across
workers. Two Solvers are constructed per fit. Frame gathers and permutations
are measurable, but eliminating them in the sorted arms does not produce the
predicted projection gain. H1 stops without a runtime or dependency change;
H3 follows. All ten fits completed on `scc-zj4` (job `8034071`, failed=0/exit=0).
See [validation](VALIDATION.md#h1-locality-sort-confirmation-hypothesis-refuted-2026-10-12)
and the [compact record](../bench/results/h-20261013-h1.json). These are single
diagnostic fits on the current runtime with trace-only instrumentation; they
do not revise the historical fixest ratios below or establish a release claim.

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
