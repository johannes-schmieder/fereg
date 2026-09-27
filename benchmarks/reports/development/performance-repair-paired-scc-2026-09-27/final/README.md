# Schwarz admission: completed bounded SCC rotation

Job **7760466**, task 1, completed all **42 triplets / 126 estimator processes**
over 23 configurations on isolated 28-core scc-wo4. Accounting has failed=0,
exit_status=0 and 5,947 seconds wall time; both completion sentinels are present.
All native/baseline comparisons pass, and all four independent OLS/Poisson
oracle cases pass. Every application completed. Reference numerical
disagreements remain separate from application success.

This tests the frozen **e4317d8e** Schwarz memory correction against its captured
baseline and pinned stock fixest. It includes neither the later separation
repair nor the new OLS slope owner. `identity.json`, `plan.json`, `receipt.json`
and `summary.json` bind the artifacts, settings, source receipts and complete
numerical outputs. No build, profiling or other fits shared the timed node.
This is a bounded development screen, not qualification or the full 140-case
confirmation. Historical partial snapshots remain unchanged.

## Gains and memory costs

Median command seconds; the ratio column is the median of per-seed C/B ratios.
Each timing configuration has two registered seeds. Small oracle cases have
one seed and do not count toward the performance population.

| N983040 case | Threads | Baseline | Candidate | C/B | B/C median RSS GiB |
|---|---:|---:|---:|---:|---:|
| PPML sparse three FE, K10 | 8 | 32.027 | 32.402 | 1.012 | 2.102 / 2.070 |
| PPML sparse three FE, K10 | 28 | 195.574 | 34.106 | 0.179 | 1.016 / 2.817 |
| PPML sparse three FE, K50 | 4 | 131.948 | 126.549 | 0.960 | 5.491 / 5.489 |
| PPML sparse three FE, K50 | 28 | 745.004 | 63.166 | 0.085 | 3.187 / 7.280 |
| OLS strong degree-2 graph, K10 | 28 | 13.182 | 3.615 | 0.274 | 0.720 / 1.577 |
| OLS sparse three FE, K50 | 28 | 68.177 | 13.895 | 0.204 | 2.574 / 4.403 |

The complete [distributions](distributions.md) retain every gain and loss.
Admitting LSMR removes the severe high-thread route downgrade, but increases
actual memory relative to the former fallback. The K50/T28 PPML example uses
7.28 GiB versus stock's 3.88 GiB. These are sampled process-tree peaks, separate
from the 8.850-GiB scheduler maximum for the whole job.

All ten PPML timing configurations are scientifically comparable with stock
defaults. Nine have median C/stock <=0.90; the remaining dense N61440/K10/T4
control is **1.113**, with individual ratios **1.044–1.182**. Across the ten
configuration medians the range is 0.373–1.113, median 0.619. This selected panel
does not establish the objective over the original campaign.

All nine OLS timing configurations remain unranked against stock because of
coefficient/RSS and/or covariance differences. The native comparisons and
independent small OLS oracles pass. Earlier small-rank adjudication explains
some inference mechanisms; it does not authorize ranking these large outputs.

## Remaining scaling and control losses

Narrow PPML still slows with more cores, even after its route is repaired:

- N983040/K10: T28 is 3.8–6.8% slower than T8, paired median **5.3%**.
- Fresh N245760/K6: T16 is 1.7–12.5% slower than T8; T28 is a further
  10.0–17.9% slower than T16. Median times are 6.336, 6.779 and 7.713 seconds.
- The wide K50 case benefits: T28 takes approximately half of T4's time.
- OLS improves throughout its registered higher-core comparisons, including
  the fresh degree-two T4/T8/T28 ladder and fresh three-FE K20 control.

Two fresh controls lose more than 5% against baseline on both seeds:
three-FE PPML K6/T16 (7.0–8.1%) and dense two-FE PPML T4 (6.0–7.6%). Their
reported route, iteration, rank, fallback and thread metadata are unchanged.
Those observations do not establish a cause; they require the planned focused
alternating confirmation. A small one-seed dense PPML oracle also loses
70 to 89 ms and remains visible as a diagnostic, not a headline configuration.

Adaptive thread ceilings remain necessary, with width and topology controls:
a blanket cap would discard the measured K50 speedup. The next focused screen
should resolve the narrow-family concurrency choice and the unchanged-route
control losses before the full sweep. No tolerance, sample, rank policy or
original frozen benchmark definition changed.
