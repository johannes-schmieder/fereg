# Comparing fereg with fixest

## October 24, 2026: beta.3 rerun

Fereg is faster in **29 of 35 scientifically eligible default-fixest
comparisons**: **15/18 OLS and 14/17 PPML**, versus 26/35 in the October 12
campaign. The other 21 default comparisons remain ineligible or unavailable.
These counts describe this fixed panel, including covariance/output and worker
variants; they are not probabilities of winning on an arbitrary model.

Campaign `20261024T135209Z-beta3-fixest-7a9ac5c4` measured frozen source
`c5582eb089485e36defd38694b0e566bab85f75c`, with the beta.3 computational
runtime selected at submission. Subsequent repository and solver-reporting
edits are outside this measurement. The Linux plugin SHA-256 is
`e0aaf262631cabe49f739b5f0136a74e7af5a67c8b85c024a4d9f58a0cdc5110`.

The rerun uses the exact 36 retained production datasets and the same 56
configurations, 16 controls, three repetitions, comparator profiles, scientific
tolerances and per-fit limits as October 12. Fixest remains 0.14.2; default and
strict profiles stay separate. The primary panel uses four workers, with
selected 1/8/16-worker variants, 100,000/1,000,000/4,000,000 rows, AKM and
one-/two-/three-FE dense/sparse designs, and covariance/saved-FE variants.

Selected results below are separate three-command medians in seconds. The
ratio is the median **paired fixest/fereg** time; above one favors fereg and
need not equal the ratio of separate medians. All rows use four workers.

| Model / design | Rows | fereg s | fixest s | Paired fixest/fereg |
| --- | ---: | ---: | ---: | ---: |
| OLS, AKM | 1,000,000 | 0.988 | 2.079 | 2.095 |
| PPML, AKM | 1,000,000 | 5.157 | 7.883 | 1.529 |
| OLS, two dense FEs | 4,000,000 | 2.654 | 3.713 | 1.401 |
| OLS, three dense FEs | 100,000 | 0.293 | 0.214 | 0.730 |
| OLS, three dense FEs | 1,000,000 | 1.561 | 1.467 | 0.939 |
| OLS, three dense FEs | 4,000,000 | 8.169 | 4.563 | 0.559 |
| PPML, two dense FEs | 1,000,000 | 4.749 | 4.407 | 0.928 |
| PPML, two dense FEs | 4,000,000 | 20.666 | 15.873 | 0.769 |
| PPML, three dense FEs | 4,000,000 | 21.320 | 21.026 | 0.987 |

The table includes all six eligible default-fixest losses. The two largest
remaining gaps are dense three-FE OLS and dense two-FE PPML at four million
rows. Their native ranges are 8.165-8.187 and 20.643-20.761 seconds, versus
fixest's 4.560-4.572 and 15.831-15.904. The three-FE PPML difference is about
1.3%; three repetitions do not establish a reliable small gap. Strict fixest
comparisons give 20/22 OLS and 4/4 PPML native wins.
Eligible reghdfe and ppmlhdfe comparisons give 20/20 and 17/17 native wins.

All **168 fereg fits pass**. Captured coefficients, covariance, objectives and
discrete fields repeat exactly at fixed worker count; cross-worker scientific
and discrete checks pass. All 168 historical native counterparts also agree on
sample/discrete fields and pass the coefficient, covariance and objective
tolerances. That historical check does not compare row-level saved fitted
arrays; each campaign's own requested-output checks remain in force. All 11
independent native smoke oracles pass. Reference smoke passes remain explicit:
default fixest 2/11, strict fixest 10/11, reghdfe 1/6 and ppmlhdfe 1/5.
Production eligibility does not imply every comparator passes every oracle.

There are **579 planned positions**: 534 successful captures, eight capture
errors, seven command timeouts and 30 repetitions skipped after a prior failure.
All failed observations are retained; no timing was retried. Of 193 arm cells,
154 are eligible, 32 have descriptive but ineligible timings and seven have no
completed timing. Four-million-row sparse three-FE PPML completes all three
native fits (median 141.187 seconds); fixest and ppmlhdfe time out, so there is
no eligible speed ratio for that case.

Setup `8269312`, smoke `8269313` and production `8269314` tasks 1-4 all have
complete successful scheduler accounting and application sentinels. The four
bundles used whole 28-core nodes `scc-uh4`, `scc-ue2`, `scc-ui1` and `scc-ui3`,
took 62-114 minutes and consumed 6.43 node-hours in total. Both packages used
the same socket-ordered CPU prefixes and NUMA memory binding. Scheduler
reservation/all-queue checks were replayed from the retained observations;
all five smoke/production host reports pass, with no known competing compute.
Build/ABI/numeric guards and all 355 benchmark-tooling tests pass on SCC.

Two setup attempts failed before campaign estimation: an inherited comparator
metadata filename, then Linux timeout-cleanup and wrapper-test shell defects.
The fixes preserve the benchmark policy and computational runtime. The focused
three-case Linux retest `8269306` passed with successful accounting before the
accepted submission. Failure summaries, identities and logs are retained.

Every corresponding October 12 configuration ran on a different host, and
the full rerun now uses the socket/NUMA placement policy introduced in H5.
Historical timing differences therefore do not isolate runtime improvements
from hardware and placement. This is descriptive Linux evidence from one seed
and three repetitions, with no confidence-qualified release or universal speed
claim. Phase clocks may nest; sampled process-tree RSS is not an allocation peak.

| Retained evidence | Contents |
| --- | --- |
| [Configuration summary](../bench/results/scc-20261024-summary.csv) | All 193 arm cells, medians/ranges, eligibility, paired ratios, hosts and sampled memory. |
| [Observations](../bench/results/scc-20261024-observations.csv) | All 579 planned positions, failures/skips, process/command times and available native phase clocks. |
| [October 12 comparison](../bench/results/scc-20261024-comparison.csv) | All 56 default-fixest configurations, both campaigns' hosts/times, eligibility and paired ratios. |
| [Inputs, identity and checks](../bench/results/scc-20261024-inputs.json) | Source/plugin/input identities, six successful accounting records, host checks, oracles, repeat/worker checks, 168 historical science checks and artifact hashes. |

Raw captures, compact collection, the checked 21-page PDF and export script
remain on SCC under
`/projectnb/welfgr/fereg/runs/20261024T135209Z-beta3-fixest-7a9ac5c4/`.

## Comparison protocol

Use the maintained [SCC package comparison](../bench/README.md#scc-package-comparison)
for OLS/PPML comparisons with fixest, reghdfe and ppmlhdfe. The campaign freezes
package versions, datasets, model options, worker counts and a fixed repetition
budget. Compare packages within the same isolated allocation on a general
eligible node.

Timing includes estimation, inference and requested fixed-effect recovery;
input generation, import and export are outside the measured command. Report
paired summaries by workload and worker count, including failed, timed-out and
scientifically ineligible comparisons. A fast result is eligible only if sample,
coefficient, covariance, objective and requested-output checks pass. Default
and strict comparator tolerances are separate profiles.

Benchmark results apply to the source revision and binaries measured. They do
not establish a speed ranking for a newer release or for another platform.
The accepted changes to the current OLS Auto policy and their qualification
limits are summarized in [VALIDATION](VALIDATION.md).

For a quick local development check, use [the small benchmark](../bench/README.md#local-benchmark).
For a change to fereg itself, use the [fixed-candidate qualification](../bench/README.md#fixed-candidate-qualification).
