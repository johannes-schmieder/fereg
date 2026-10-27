# Comparing fereg with fixest

The separate [Veneto AKM comparisons](veneto_benchmarks.md) use the real long
and short fevc panels at 1/2/4/8/16 cores. On October 26 they ran on the beta.7
runtime with PyFixest, both with and without saved worker/firm effects.
Without saved effects every comparator except two timed-out cells passes the
scientific gates; with saved effects all comparator timings stay descriptive.

## October 26, 2026: beta.7 rerun with PyFixest and areg

Fereg is faster in **all 35 scientifically eligible default-fixest
comparisons**: **18/18 OLS and 17/17 PPML**, versus 27/35 in the beta.6 rerun.
The strongest comparator is now PyFixest. Against its pinned main revision,
which defaults to the `within` LSMR demeaner, fereg is faster in **21 of 35**
eligible comparisons (13/18 OLS, 8/17 PPML); against the PyFixest 0.60.0
release, in 22 of 35 (15/18, 7/17). Fereg is faster than `areg` in all three
one-FE OLS configurations (1.12-2.06), and in every eligible strict-fixest
(22/22 OLS, 4/4 PPML), reghdfe (20/20) and ppmlhdfe (17/17) comparison. These
counts describe this fixed panel;
they are not probabilities of winning on an arbitrary model.

Campaign `20261026T184833Z-beta7-fixest-a4bd2cce` measured frozen source
`a4bd2ccea1d09fdd44349fd591598cd3b5a9fc6a`. Its runtime identity is the
v0.5.0-beta.7 package's (`9713d640...`); the SCC-built Linux plugin SHA-256 is
`cc1920c2b4bd258a706bebd1d44f309e703eb0ea75258920ce935dcf30b0e92a`. Datasets,
the 56 configurations, fixest/reghdfe/ppmlhdfe profiles, repetitions,
scientific tolerances, per-fit limits and socket/NUMA placement are unchanged.
New arms:

- **PyFixest 0.60.0**, the current PyPI release, with its default Rust MAP
  demeaner, in every configuration.
- **PyFixest main** at `263cf2d3` with within-py 0.4.0, whose default is the
  `within` LSMR demeaner with adaptive preconditioning. This default is merged
  but not yet in a PyFixest release. It also runs in every configuration.
- **areg** in the three four-worker one-FE OLS configurations. Its absorbed
  count is compared after adding back the intercept it reports separately.
  Stata 19's `areg` also absorbs several effects; restricting it to one-FE cells
  was a harness error, and a supplement adds it to the multi-FE OLS cells.

Both PyFixest arms keep default estimation tolerances, recover saved FE at
1e-11 like the fixest arms, and use locked Linux environments built in the
setup job and hashed into the runtime inventory. The schedule grew from 579 to
924 planned fits in eight bundles with a six-hour walltime.

Selected results are three-command medians in seconds; ratios are the median
**paired comparator/fereg** time within the same node, so values above 1 mean
fereg was faster. Rows use four workers. `*` marks a completed but
scientifically ineligible time; NR means no completed time.

| Model / design | Rows | fereg s | fixest s | fixest / fereg | within s | within / fereg | PyFixest 0.60 s | 0.60 / fereg |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| OLS, AKM | 1,000,000 | 1.73 | 2.67 | 1.54 | 2.81 | 1.62 | 5.81 | 3.31 |
| OLS, AKM | 4,000,000 | 4.03 | 9.16 | 2.28 | 10.79 | 2.70 | 27.52 | 6.89 |
| OLS, one FE | 1,000,000 | 0.74 | 1.01 | 1.36 | 0.76 | 1.01 | 1.06 | 1.42 |
| OLS, two dense FEs | 4,000,000 | 2.67 | 3.71 | 1.39 | 3.37 | 1.27 | 4.79 | 1.80 |
| OLS, three dense FEs | 4,000,000 | 4.48 | 4.55 | 1.02 | 3.98 | 0.89 | 5.34 | 1.19 |
| OLS, three sparse FEs | 4,000,000 | 21.27 | 122.51* | – | 15.58* | – | NR | – |
| PPML, AKM | 1,000,000 | 4.24 | 7.95 | 1.89 | 9.96 | 2.31 | 27.00 | 6.29 |
| PPML, AKM | 4,000,000 | 19.05 | 43.53 | 2.28 | 49.21 | 2.59 | 134.56 | 7.11 |
| PPML, two dense FEs | 4,000,000 | 15.23 | 15.82 | 1.04 | 14.44 | 0.95 | 12.50 | 0.82 |
| PPML, two sparse FEs | 4,000,000 | 27.40 | 195.29* | – | 51.87* | – | NR | – |
| PPML, three dense FEs | 4,000,000 | 17.27 | 21.07 | 1.22 | 15.91 | 0.92 | 13.70 | 0.80 |
| PPML, three sparse FEs | 1,000,000 | 31.46 | 149.42* | – | 18.99* | – | NR | – |

**Against fixest.** All eight beta.6 default-fixest losses are now wins. The
five PPML flips reflect beta.7's PPML two-FE protocol; dense two-FE PPML at four
million rows moved from 0.761 to 1.039. The three OLS flips (one FE at one
million rows and three dense FEs at one and four million) are not runtime
effects: every OLS fit is bit-identical to beta.6, and those cells were within
3% of parity. Default fixest still loses most where graphs are sparse, but
those comparisons stay ineligible because default fixest does not count
disconnected components exactly.

**Against PyFixest.** The `within` default is faster than fereg on dense
three-FE designs (0.64-0.89 OLS, 0.80-0.92 PPML), dense two-FE PPML
(0.90-0.98), one-FE PPML up to one million rows (0.76-0.98) and the
100,000-row one-FE and dense two-FE OLS cells. On sparse three-FE designs its
medians are 0.44-0.87 of fereg's (descriptive). Fereg is faster on AKM designs
(1.28-2.70 OLS, 1.93-2.73 PPML) and large dense two-FE OLS (1.15-1.29); on
sparse two-FE designs the descriptive median ratios are 0.99-2.15. PyFixest
0.60's MAP demeaner is the fastest package on dense two- and three-FE PPML
(0.68-0.88) and one-FE PPML (0.71-0.93), but it failed to converge after
10,000 iterations in four sparse cells and hit the command limit in six more.
With saved FE, PyFixest recovers effects with scipy's LSQR: the within arm took
10.2-10.5 seconds in the four-worker saved-FE AKM OLS cells against fereg's
2.2-2.4, and those cells fail the saved-prediction gate. PyFixest's sparse
cells are ineligible for the same reason as default fixest's: it counts
absorbed degrees of freedom by levels, not exact components, so DoF and the
covariance scale differ.

**Science and coverage.** All 168 native fits pass, repeat exactly at fixed
worker count and agree across workers. Against beta.6, all 168 pass the
sample/discrete, coefficient, covariance and objective checks; all 87 OLS and 9
one-FE PPML fits are bitwise identical, and the 72 multi-FE PPML fits stay
within 1.9e-5 (coefficients) and 2.4e-9 (covariance) of the gate envelopes.
Independent smoke oracles pass for fereg 11/11, strict fixest 10/11, PyFixest
within 8/11, PyFixest 0.60 3/11, default fixest 2/11, reghdfe 1/6 and
ppmlhdfe 1/5. The 924 planned positions have 831 successful captures, 14
capture errors, 13 command timeouts (six PyFixest 0.60), four PyFixest 0.60
demeaning failures and 62
repetitions skipped after a prior failure. Of 308 arm cells, 227 are eligible.
All failed observations are retained; no timing was retried.

Setup `8289446`, smoke `8289447` and production `8289448` tasks 1-8 all have
successful scheduler accounting and application sentinels. Bundles used whole
28-core E5-2680 v4 nodes (`scc-wo4`, `scc-wm1`, `scc-wn2`, `scc-wg3`, `scc-va1`,
`scc-wh4`, `scc-wh1`, `scc-wm2`), took 0.82-2.32 hours and 10.28 node-hours in
total; all nine host reports pass. Same-model nodes still differ by up to about
2.4x (see the [Veneto runs](veneto_benchmarks.md)), so only paired ratios within
a node are compared. This is descriptive Linux evidence from one seed and three
repetitions, with no confidence-qualified or universal speed claim.

| Retained evidence | Contents |
| --- | --- |
| [Configuration summary](../bench/results/scc-20261026-beta7-summary.csv) | All 308 arm cells, medians/ranges, eligibility, paired ratios, hosts and sampled memory. |
| [Observations](../bench/results/scc-20261026-beta7-observations.csv) | All 924 planned positions, failures/skips, process/command times, PyFixest identities and native phase clocks. |
| [Beta.6 comparison](../bench/results/scc-20261026-beta7-comparison.csv) | All 56 default-fixest configurations with both campaigns' hosts, times and ratios, plus the PyFixest and areg cells. |
| [Inputs, identity and checks](../bench/results/scc-20261026-beta7-inputs.json) | Source/plugin/input identities, PyFixest pins and freezes, accounting, host checks, oracles, repeat/worker checks, 168 historical science checks and artifact hashes. |

Raw captures, the compact collection and the checked 29-page PDF remain on SCC
under `/projectnb/welfgr/fereg/runs/20261026T184833Z-beta7-fixest-a4bd2cce/`.
The PDF's provenance table lists the old 3 h 50 min / 4 h stopping limits; the
campaign used 5 h 50 min and 6 h, as `campaign.json` records, and the renderer
now prints the policy's values.

## October 26, 2026: beta.6 rerun

Fereg is faster in **27 of 35 scientifically eligible default-fixest
comparisons**: **15/18 OLS and 12/17 PPML**, versus 29/35 in the beta.3
rerun. The other 21 default comparisons remain ineligible or unavailable.
These counts describe this fixed panel; they are not probabilities of winning
on an arbitrary model. Two of the four bundles ran on markedly slower nodes
than beta.3 (below), so count changes do not isolate runtime effects.

Campaign `20261026T012706Z-beta6-fixest-fe67c26c` measured frozen source
`fe67c26c664cdb6969f28d0fd220395c25293da0`, the v0.5.0-beta.6 runtime with the
many-FE OLS geometry policy active. Its runtime identity matches the package;
the SCC-built Linux plugin SHA-256 is
`acb42877d6b2b7c38fc6642bfaddd2d95979715c95368f61a976a43b3edc49e9`. Datasets,
the 56 configurations, comparators, repetitions, scientific tolerances,
per-fit limits, launchers and socket/NUMA placement are unchanged from the
beta.3 rerun.

Selected results are three-command medians in seconds; the ratio is the
median **paired fixest/fereg** time within the same node. Rows use four
workers unless noted.

| Model / design | Rows | Node | fereg s | fixest s | Paired fixest/fereg |
| --- | ---: | --- | ---: | ---: | ---: |
| OLS, AKM | 1,000,000 | wo4 | 1.761 | 2.675 | 1.533 |
| PPML, AKM | 1,000,000 | wd1 | 12.121 | 18.487 | 1.533 |
| OLS, one FE | 1,000,000 | wo4 | 1.413 | 1.387 | 0.981 |
| OLS, two dense FEs | 4,000,000 | yk1 | 2.471 | 3.404 | 1.378 |
| OLS, three dense FEs | 100,000 | wd1 | 0.352 | 0.431 | 1.224 |
| OLS, three dense FEs | 1,000,000 | wo4 | 2.073 | 2.028 | 0.978 |
| OLS, three dense FEs | 4,000,000 | wd1 | 9.731 | 9.455 | 0.972 |
| PPML, two dense FEs | 1,000,000 | yk1 | 4.299 | 4.071 | 0.944 |
| PPML, two dense FEs, 16 workers | 1,000,000 | yk1 | 3.314 | 3.270 | 0.987 |
| PPML, two dense FEs | 4,000,000 | wd1 | 47.500 | 36.151 | 0.761 |
| PPML, three dense FEs | 1,000,000 | yk4 | 5.268 | 5.200 | 0.990 |
| PPML, three dense FEs | 4,000,000 | yk1 | 20.259 | 18.954 | 0.936 |

The table includes all eight eligible default-fixest losses. Five are within
3%, which three repetitions do not establish reliably. The clear gaps remain
dense two-FE PPML (0.761 at four million rows, 0.944 at one million) and dense
three-FE PPML at four million rows (0.936). Dense three-FE OLS, the geometry
policy's target, moved from 0.730/0.939/0.559 to 1.224/0.978/0.972 at
100,000/1,000,000/4,000,000 rows. At four million rows fixest took 2.07 times
its beta.3 time on the slower node while fereg took 1.19 times. Strict fixest
comparisons give 21/22 OLS and 4/4 PPML native wins. Eligible reghdfe and
ppmlhdfe comparisons give 20/20 and 17/17 native wins.

All **168 fereg fits pass**. Captured coefficients, covariance, objectives and
discrete fields repeat exactly at fixed worker count; cross-worker scientific
and discrete checks pass. All 168 also pass the sample/discrete, coefficient,
covariance and objective checks against their beta.3 counterparts. The 147 fits
outside the seven three-FE OLS configurations are bitwise identical to beta.3.
All 11 independent native smoke oracles pass; reference smoke counts are
unchanged (default fixest 2/11, strict fixest 10/11, reghdfe 1/6, ppmlhdfe 1/5).

The 579 planned positions have the same outcome counts as beta.3: 534
successful captures, eight capture errors, seven command timeouts and 30
repetitions skipped after a prior failure. All failed observations are
retained; no timing was retried. Of 193 arm cells, 154 are eligible, 32 have
descriptive but ineligible timings and seven have no completed timing.
Four-million-row sparse three-FE PPML completes all three native fits (median
162.240 seconds); fixest and ppmlhdfe time out.

Setup `8280462`, smoke `8280463` and production `8280464` tasks 1-4 all have
complete successful scheduler accounting and application sentinels, with no
failed setup attempt. Bundles used whole 28-core nodes `scc-wo4` and `scc-wd1`
(Xeon E5-2680 v4, the scheduler's CPU type for all four beta.3 nodes) and
`scc-yk1` and `scc-yk4` (Xeon Gold 6132). They took 72-210 minutes and 8.11
node-hours. All five smoke/production host reports pass with no known
competing compute, and all 377 benchmark-tooling tests pass on SCC.

Absolute times on `scc-wd1` and `scc-wo4` are not comparable with beta.3.
On `scc-wd1`, configurations with bitwise-unchanged native arithmetic took a
median 2.3 times their beta.3 time, with every native phase slower, including
input handling; fixest took 2.2 times. On `scc-wo4` the factors were 1.8 and
1.4, which lowered fereg's AKM ratios (four-core OLS from 2.10 to 1.53). On the
Gold 6132 nodes both packages took 0.9-1.0 times their beta.3 times. The
campaign does not identify why same-model nodes differ. This is descriptive
Linux evidence from one seed and three repetitions, with no
confidence-qualified release or universal speed claim.

| Retained evidence | Contents |
| --- | --- |
| [Configuration summary](../bench/results/scc-20261026-summary.csv) | All 193 arm cells, medians/ranges, eligibility, paired ratios, hosts and sampled memory. |
| [Observations](../bench/results/scc-20261026-observations.csv) | All 579 planned positions, failures/skips, process/command times and available native phase clocks. |
| [Beta.3 comparison](../bench/results/scc-20261026-comparison.csv) | All 56 default-fixest configurations, both campaigns' hosts/times, eligibility and paired ratios. |
| [Inputs, identity and checks](../bench/results/scc-20261026-inputs.json) | Source/plugin/input identities, accounting, host checks and classes, oracles, repeat/worker checks, 168 historical science checks and artifact hashes. |

Raw captures, compact collection, the checked 21-page PDF and export script
remain on SCC under
`/projectnb/welfgr/fereg/runs/20261026T012706Z-beta6-fixest-fe67c26c/`.

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
