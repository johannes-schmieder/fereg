# SCC sweep completion and validation closeout — 2026-09-29

All **568/568 registered groups** are complete. The bounded continuation ran
only the 27 untouched PPML groups, with no new application failures. The
operational repairs and candidate validation reruns are recorded separately in
the [harness repair report](../scc-harness-repair-2026-09-29/README.md). Neither
solver candidate is adopted: their remaining numerical/control failures are
described below. This is a completed development measurement, not qualification
or completion of the broader performance objective.

## Identity, coverage and acceptance

These results measure package **161b9491 / c09eeab3…**, Linux plugin
**59d0341a…**, with measurement fingerprint
`fb2ad5407afcea22d56155aa7858203bfcaea590f750ca5d06eeccce746cffc6`.
They do **not** measure the newer bias-support and anchored-recovery commits.
Exact inputs, source and reference pins are in the
[launch receipt](../current-source-sweep-scc-2026-09-28/README.md) and
[collected provenance](generated/provenance.json).

Continuation **7778971** used one reserved 28-core E5-2680v4 node, 3G/core and
a three-hour cap. Scheduler accounting is clean, with **541 seconds** wall
time and **3.383G** maxvmem. Its terminal marker, complete bundle and exact plan
binding pass. All **541 earlier completed markers are unchanged**, and all
**568 input receipts match the original campaign**. The frozen audit has no
missing groups or integrity errors. [Acceptance](generated/continuation-acceptance.json)
records these checks; [collection hashes](collection-hashes.json) verify every
downloaded aggregate against SCC. Original failed attempts remain preserved.

The unchanged grid comprises 68 OLS and 72 PPML configurations, four warm seeds
each and eight cold anchors. N is 61,440, 983,040 or 3,932,160; K is mainly ten,
with selected six- and fifty-control cases. Requested threads are 1/4/8/28.
Coverage includes categorical and multi-partition slopes, fast/low memory and
zero outcomes. `strong_d2/d3/d6` are two-FE graph designs whose degree is
2/3/6; these labels do not mean two/three/six FE partitions.

The commands are public `fereg` versus stock `fixest::feols` with IID inference,
and public `feppml` versus stock `fixest::fepois` with robust inference. Default
and strict PPML are separate reference arms; strict sets `glm.tol=1e-10` and
`fixef.tol=1e-11`. Inputs are synthetic and unweighted. These clocks do not
cover clustering, saved FE or fitted-mean publication. Unchanged reghdfe, gtools,
ppmlhdfe and PyFixest commands were not rerun in this sweep; their earlier
receipts must retain their earlier source identities.

Command times exclude data generation/import, initialization and extraction;
[runs.csv](generated/runs.csv) separately retains whole-arm elapsed time and
sampled process-tree RSS. Fits in each group run sequentially under the frozen
order. The continuation was held behind all diagnostic/build jobs. This
follow-up ran no additional fits, builds or profiles alongside its headline clocks.

## Complete timing distributions

Ratios are fereg/reference, so smaller is better. Each configuration needs all
four scientifically valid warm pairs. The table reports the distribution of
those configuration medians, **not** a confidence interval or an unconditional
win rate over the entire grid. Every incomplete or scientifically invalid row
remains in the [full summaries](generated/summary.csv) and
[pair receipts](generated/paired-ratios.csv).

| Model / reference | Complete / registered | Median ratio | Q1–Q3 | Range | Wins | At least 10% faster | Losses over 20% |
|---|---:|---:|---:|---:|---:|---:|---:|
| OLS / default stock | 44/68 | 1.133 | 0.805–1.817 | 0.401–7.320 | 19/44 | 14/44 | 20/44 |
| PPML / default stock | 52/72 | 0.827 | 0.528–1.119 | 0.231–2.800 | 33/52 | 30/52 | 10/52 |
| PPML / strict stock | 58/72 | 0.548 | 0.269–0.833 | 0.019–1.555 | 49/58 | 46/58 | 4/58 |

OLS loses 25 complete configuration medians, PPML default 19, and PPML strict
nine. Across the four repetitions of these complete rows, the loss counts are
102/176, 75/208 and 34/232, respectively. Neither the universal 1.20 ceiling
nor the 0.90 aspiration is met.

Selected remaining losses show where another milestone can matter. Ratios are
medians of paired ratios; separately reported command medians need not divide
to the same value.

| Model / design / reference | N / K / threads | Native median s | Reference median s | Paired median | Pair Q1–Q3 |
|---|---:|---:|---:|---:|---:|
| OLS strong_d3 / stock | 3,932,160 / 10 / 4 | 23.381 | 3.185 | 7.320 | 7.050–7.392 |
| OLS strong_d6 / stock | 3,932,160 / 10 / 4 | 15.908 | 4.141 | 3.801 | 3.771–3.857 |
| OLS dense two-FE / stock | 61,440 / 10 / 4 | 0.221 | 0.075 | 2.933 | 2.819–2.948 |
| PPML weak_d3 / default | 3,932,160 / 10 / 28 | 46.039 | 17.096 | 2.800 | 2.286–3.354 |
| PPML strong_d3 / strict | 3,932,160 / 10 / 28 | 21.557 | 13.385 | 1.555 | 1.083–2.060 |
| PPML dense two-FE zeros / strict | 61,440 / 10 / 4 | 1.039 | 0.801 | 1.299 | 1.283–1.318 |
| PPML strong_d6 / strict | 3,932,160 / 10 / 28 | 11.536 | 8.919 | 1.293 | 1.293–1.318 |
| PPML one-FE / strict | 3,932,160 / 10 / 4 | 17.427 | 13.153 | 1.288 | 1.224–1.353 |

Slope evidence is materially better for PPML than for OLS:

| Model / design / mode | K / threads | Comparable reference | Median ratio | Coverage |
|---|---:|---|---:|---|
| OLS dense two-FE / fast | 10 / 28 | stock | 1.219 | 4/4 valid |
| OLS dense two-FE / low | 6 / 4 | stock | 1.409 | 4/4 valid |
| OLS sparse three-FE / fast and low | 10 / 28; 6 / 4 | stock | NR | All eight pairs fail science |
| PPML dense two-FE / fast | 10 / 28 | strict stock | 0.348 | 4/4 valid |
| PPML dense two-FE / low | 6 / 4 | strict stock | 0.255 | 4/4 valid |
| PPML sparse three-FE / low | 6 / 4 | strict stock | 0.024 | 4/4 valid; 9.301 versus 451.985 s |
| PPML sparse three-FE / fast | 10 / 28 | strict stock | Unranked | One reference timeout prevents a full quartet |

All these slope rows have N=61,440. Default PPML slope discrepancies remain
unranked. The very large strict-stock loss on sparse three-FE low mode applies
to this narrow, difficult design; it is not a general slope speedup estimate.

## Missing rankings, scientific discrepancies and application failures

There are **no missing scheduled groups**. There are 1,370 successful application
executions, eight command timeouts and 50 explicit historical reference-budget
exclusions. Application success does not imply scientific parity. The 860
planned reference comparisons comprise **653 passes, 149 accuracy failures,
eight unavailable pairs and 50 exclusions**.

| Comparison | Pass | Accuracy failure | Unavailable | Historical exclusion |
|---|---:|---:|---:|---:|
| OLS / stock | 179 | 94 | 3 | 0 |
| PPML / default stock | 228 | 55 | 2 | 7 |
| PPML / strict stock | 246 | 0 | 3 | 43 |

These counts include cold anchors. NR is a failed scientific comparison,
not a missing execution or an automatic fereg bug. OLS discrepancies include
coefficient, covariance/SE, residual-sum-of-squares and reference-convergence
checks. They concentrate in sparse two-/three-FE and degree-two designs.
The comparison alone cannot assign the faulty side. The accepted independent
smoke establishes its own small designs; it does not adjudicate every large
grid discrepancy.

The continuation adds three default-PPML SE discrepancies on weak_d3,
N=61,440/K10 (T4 repetitions two/four and T28 repetition four); the corresponding
strict pairs pass. There were 146 accuracy failures before continuation and
149 after it. Existing small independent-oracle evidence supports default
reference accuracy as a cause in some weak-d3 cases, but these new large
receipts are not independently adjudicated merely by that analogy.

All eight application timeouts predate continuation and remain untouched:

- Native OLS strong_d2, N=3,932,160/K10/T4, warm repetitions 2–4: three
  720-second command bounds. This is a real native performance/failure issue.
- Default PPML sparse two-FE zeros, N=61,440/K10/T4, repetition 3: 120 seconds.
- Default and strict PPML sparse three-FE fast slopes, N=61,440/K10/T28,
  repetition 3: 120 and 398 seconds, respectively.
- Strict PPML sparse three-FE, N=3,932,160/K10/T28, repetition 3, and sparse
  two-FE at the same size/threads, repetition 4: 2,400 seconds each.

The historical exclusions are budget decisions, not newly observed failures
or timing wins. No completed failure or unfavorable seed was rerun.

## Threads and memory

The [read-only thread inventory](generated/thread-inventory.json) validates
selected/requested metadata for every successful native command. Twelve PPML
requests for 28 threads select eight; the other successful calls select their
requested team. A request increase with the same selected team is not evidence
about adding native workers.

Of 36 adjacent requested-thread transitions per model, 24 OLS and 31 PPML have
all four scientifically eligible observations. None of the 24 OLS transitions
has a slower median. Two PPML transitions do: weak_d3 at N=61,440 and 983,040,
from 4 to 28 actual native threads, with median time ratios **1.178** and
**1.344**. The smaller case is slower on all four repetitions (1.169–1.197);
the larger on three. These are observational cross-case clocks, sometimes
from different tasks/nodes. They identify a routing investigation; they do not
establish causality or satisfy a claim that more cores never lose time.
Even OLS has individual slower repetitions despite its favorable medians.

Successful-command absolute process-tree RSS ranges from 0.081 to 7.392 GiB
for fereg OLS and 0.094 to 9.086 GiB for feppml. These are observed process
peaks, not the solver's admission estimate or allocator-only memory. Full
per-arm observations and paired RSS ratios remain in the CSVs. This milestone
changes no estimator memory layout and establishes no candidate memory gain.

## Candidate decisions and next bounded work

The [repaired diagnostics](../scc-harness-repair-2026-09-29/README.md) reached
scientific gates successfully. CG improves its known-within core screen from
58/72 passing baseline cases to 72/72, with ten controls passing on each arm.
Its plugin build/ABI checks and eight existing public fixtures pass. The new
public near-within oracle nevertheless returns r(9); its exact assertion is
not localized. Baseline fails earlier with r(459), negative CG Hestenes
improvement potential. This is useful progress, not an acceptable production
replacement yet.

Schwarz passes 119/120 core tests, including its three memory-bound tests, but
fails the canonical-rank test's solver/fallback reporting assertion. It is
refused before public builds. The failure has not been attributed to the
patch without a baseline adjudication. The separate memory probe establishes
that an admission limit changes the baseline from a 19.218-second LSMR finish
to a 180-second timeout on identical input; it does not time a passing Schwarz
candidate. Neither candidate is adopted, and neither failed numerical test
was automatically retried.

The comparator retest completes all 12 PyFixest applications and four gtools
sample-publication checks. Default PyFixest SE failures remain scientific
discrepancies; all eight tight-profile diagnostic fits pass. Reconstructed
current-weight covariance is diagnostic and never substituted into stock
timing results. These checks fix the validation path, not the comparators'
historical ranked results.

The next milestone should first localize the CG public assertion and compare
the Schwarz reporting failure against its exact baseline. That is the shortest
route to resolving the measured degree-two/admission failure without weakening
science. After green public gates, use a small same-node performance screen
around the large OLS degree-three/six losses and the two weak-d3 PPML thread
regressions. Profile large one-FE PPML and dense zero-outcome overhead next if
those costs remain. There is stronger evidence for these targets than for
reopening the already fast comparable PPML slope cases. No extra jobs or
solver changes were made as part of this collection closeout.
