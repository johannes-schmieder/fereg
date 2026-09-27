# Lighter SCC OLS/PPML campaign: completed

All **568 registered groups completed**, including 12 retained pilot groups and
556 production groups. Production array **7747518/tasks 1–16** has clean
accounting, all 16 terminal application sentinels, no pending groups and no
integrity errors. No continuation or failed-fit reruns were needed. Completion
means complete evidence, not that every command or scientific comparison passed.

The 10% advantage over fixest remains a development objective, not an achieved
general claim. PPML often beats the strict comparable reference, but substantial
OLS and PPML losses remain. This campaign measures one frozen candidate, not an
old-versus-new estimator improvement.

## Identity, scope and resource use

- Estimator/controller: `58284f76a65b226e6df09a48cafa5caa68461af2`.
- Measurement fingerprint:
  `041bf47963205620c13f2137c17a25a47afd70d73c5d03c42a936bbb39ce29e6`.
- Linux plugin SHA-256:
  `1f59671842c4ff436d6975057eb1fcf2d32cec302bc6a64b8adc73d91588c023`.
- Production plan SHA-256:
  `80e2439f6d134f9c9e1afd2d701910d6b38886d148e3e32d9783463396f84702`.
- External continuation planner: `78b2eb0b`; ADR0226 and the separately bound
  adjudicated pilot gate remain in force. The original rejected pilot is intact;
  no `PILOT.ACCEPTED.json` was manufactured.
- Four warm repetitions across 68 OLS and 72 PPML configurations, plus eight
  separate cold anchors. N is 61,440, 983,040 or 3,932,160; K is principally 10,
  with selected K50 and slope K6 controls. Requested threads are 1/4/8/28.
- IID OLS and robust PPML, unweighted, with categorical and selected two-/three-
  partition absorbed-slope cases, low-memory controls and a zero-outcome appendix.
  **Fitted-mean publication is not timed.** These are not the earlier Mac
  robust-plus-fitted-output slope comparisons.
- Each task reserved a full 28-core E5-2680v4 node, 3G/core, 12 hours, concurrency
  eight. Stata control work was capped by the four-processor license; requested
  native/R threads are separate. No estimator/controller changes were made.
- Production took **119.987 node-hours**, versus 158.59 forecast (24.3% less).
  Tasks used 5.58–8.84 hours. Last task ended **2026-09-27 09:28:35 UTC**,
  16h09m after submission. Scheduler maxvmem ranged from 6.444G to 7.413G.
  These totals exclude setup, smoke, pilot and scientific adjudication. The
  stopped larger campaign is not a completed like-for-like wall-time baseline;
  no measured twofold speedup over that campaign is claimed.

The precise executed manifest, runtime inventory and unchanged automatic report
are under [generated/](generated/README.md). Production accounting and bundle
receipts are under [accounting/](accounting/7747518.json) and `bundles/`.
Later local PPML bias-correction development is outside this snapshot.

## Valid four-repeat performance

Ratios are fereg/fixest command time; below one favors fereg. First take the
median of the four same-job paired ratios within each configuration, then
summarize those configuration medians. This differs from dividing aggregate
timings. All four repetitions must pass the frozen scientific checks. Coverage
differs across references, so the rows are not matched populations.

| Comparison | Complete / registered configurations | Median ratio | Across-configuration Q1–Q3 | Range | Median at least 10% faster | Median slower |
|---|---:|---:|---:|---:|---:|---:|
| OLS / default fixest | 44 / 68 | 1.159 | 0.800–2.378 | 0.402–4.495 | 14 / 44 | 25 / 44 |
| PPML / default fixest | 51 / 72 | 1.157 | 0.763–1.999 | 0.267–5.751 | 16 / 51 | 30 / 51 |
| PPML / strict fixest | 55 / 72 | 0.806 | 0.428–1.180 | 0.035–3.712 | 34 / 55 | 18 / 55 |

Strict stock fixest 0.14.2 uses `glm.tol=1e-10`, `fixef.tol=1e-11`.
Default-reference accuracy failures remain separate. There are 13, 16 and 32
configurations, respectively, where **every** repetition beats its reference
by at least 10%; 23, 24 and 14 lose every repetition. Repetition losses within
the complete configurations are 98/176, 118/204 and 69/220. These distributions
are across four predeclared seeds, not confidence intervals or universal wins.

Against reghdfe, all 64 complete OLS configurations win every repetition
(median configuration ratio 0.088, range 0.013–0.766). Against ppmlhdfe, all 58
complete PPML configurations win every repetition (0.117, range 0.024–0.375).
Other commands and all exclusions remain in the automatic report; gtools
comparisons without exact sample evidence receive no timing rank.

### Absorbed slopes

For the dense two-partition N61,440 cases, median command seconds and median
paired ratios are:

| Model / mode | K / threads | Native seconds | Comparable fixest seconds | Ratio |
|---|---:|---:|---:|---:|
| OLS / fast | 10 / 28 | 0.4855 | 0.1505 default | 3.226 |
| OLS / low | 6 / 4 | 0.8675 | 0.2000 default | 4.445 |
| PPML / fast | 10 / 28 | 0.5545 | 1.2445 strict | 0.457 |
| PPML / low | 6 / 4 | 0.5870 | 1.8090 strict | 0.321 |

Default fixest passes none of the slope PPML pairs. Three-FE slope OLS has no
passing default-fixest pairs. Three-FE slope PPML has three passing strict
repetitions in each mode; the original warm-1 reference timeouts prevent a
four-repeat headline. Independent pilot adjudication validates those native
outputs scientifically, but its cross-job reference clocks never fill the
missing timing pair. No three-FE slope ratio is promoted here.

### Remaining measured losses

The largest comparable strict-PPML losses are not the dense slope cases:

| Design, N, K10, threads | Native median seconds | Strict fixest median seconds | Median paired ratio |
|---|---:|---:|---:|
| strong_d6, 3,932,160, T28 | 32.633 | 8.8875 | 3.712 |
| strong_d3, 3,932,160, T28 | 36.3255 | 13.1845 | 2.755 |
| strong_d6, 3,932,160, T4 | 35.1525 | 15.137 | 2.325 |
| weak_d3, 3,932,160, T28 | 93.803 | 47.0055 | 2.034 |

Large OLS strong_d3/T4 loses 4.495x (14.1465 versus 3.1525 seconds), and
strong_d6/T4 loses 3.737x. Many sparse OLS fixest comparisons are scientifically
unresolved and cannot be ranked. Thread-stratified medians have different
coverage; they do not alone establish a causal threading regression.

Command clocks exclude synthetic generation, host-language import, warm-up and
result extraction; they include the public command's own native preparation,
solve and requested inference. `runs.csv` retains whole-arm elapsed seconds
separately. Its difference from command time combines several costs and is
not a measured decomposition into loading, estimation and output phases.

Median process-tree RSS ratios across the same complete configurations are
0.679 for OLS/default, 0.781 for PPML/default and 0.738 for PPML/strict. Ranges
are 0.335–1.474, 0.373–1.652 and 0.373–1.968, respectively. RSS includes language
runtimes and is sampled every 10ms; it is not allocation volume. Successful
native fits peak at 2.988 GiB OLS and 6.489 GiB PPML.

## Failures and scientific limits

Native outcomes are **536 passes, 28 command timeouts and four worker failures**.
All failures are retained; no scientific or ambiguous failure was retried.

| Native family, design | N / threads | Four-seed outcome | Frozen command cap |
|---|---|---|---:|
| OLS, three_fe_sparse | 3,932,160 / T4 | 4 timeouts | 167s |
| OLS, strong_d2 | 3,932,160 / T4 and T28 | 8 timeouts | 120s |
| PPML, three_fe_sparse | 3,932,160 / T4 | 4 timeouts | 1481s |
| PPML, strong_d2 | 3,932,160 / T4 and T28 | 8 timeouts | 197s |
| PPML, three_fe_sparse zeros | 61,440 / T4 | 4 timeouts | 120s |
| PPML, two_fe_sparse zeros | 61,440 / T4 | 4 application failures, rc459 | — |

Timeouts are lower bounds at configuration-specific predeclared budgets, not
measured completion times or proof the fit would exceed 2400s. The four rc459
failures took about 109–111 seconds; the captured metadata does not identify
their numerical cause. Native robustness on sparse zero-outcome designs needs
its own bounded diagnostic, not a blind rerun of this campaign.

Across all commands: 2,936 passes, 227 worker failures, 151 command timeouts,
two startup timeouts, 12 historical budget exclusions and 80 unsupported-surface
exclusions. Across 2,748 attempted pair comparisons: 1,930 pass, 393 fail accuracy
checks and 425 are unavailable. These counts include cold anchors. Some passing
gtools comparisons still lack exact samples and are timing-ineligible.

Fixest-specific discrepancy counts are 85 OLS/default, 48 PPML/default and
three PPML/strict. The three strict failures concern coefficients: two
strong_d2/61,440 warm-1 comparisons at T4/T28, and dense two-FE zeros warm-4 at
T4. A pair disagreement does not by itself identify the erroneous estimator.
The complete failed-check list is in `generated/paired-ratios.csv`.

The independent accepted smoke and source-bound pilot QR/SVD/Poisson/Hessian
adjudication remain the independent scientific evidence. No new explicit-design
oracle was run for every large production cell. The post-run audit replays
the captured comparison checks; it does not replace an independent oracle.

## Evidence and next work

[The frozen controller audit](generated/audit.json) reports 568/568 groups and
zero integrity errors. [The captured-output audit](generated/captured-audit.json)
replayed all 2,748 comparisons exactly, verified input/runtime identities,
successful-arm sentinels, complete planned bundles and terminal logs. Its
11,674-file SHA-256 inventory binds remote inputs, compact outputs and logs;
raw licensed logs remain on SCC. `audit_captured.py` records this read-only
procedure. `summarize.py` regenerates [analysis.json](analysis.json) from the
collected immutable report. No estimator fits or solver changes were made in
this closeout.

The next development milestone should first localize the sparse-zero rc459
failure and the strongest timeout cases, using captured seeds and fresh
independent oracles. For speed, prioritize PPML on large strong/weak graph
designs at T28, where valid losses remain 2–3.7x even against strict fixest.
The dense OLS slope path is a separate clear 3–4.4x loss. Before optimizing
sparse OLS against fixest, adjudicate its output disagreements. These measured
targets are more informative than further tuning already-winning strict-PPML
dense slopes. This is development evidence, not qualification or a release.
