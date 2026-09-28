# Equal-request thread-ceiling candidate screen

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 27/27 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| prior-narrow-k6 | 0.816 / 0.856 / 0.926 | 0.556 / 0.613 / 0.652 |
| fresh-narrow-k12 | 0.819 / 0.896 / 0.971 | NR |
| fresh-boundary-k15 | 0.891 / 0.927 / 0.984 | 0.548 / 0.591 / 0.612 |
| fresh-boundary-k16 | 0.946 / 0.985 / 1.016 | 0.557 / 0.607 / 0.622 |
| fresh-wide-k32 | 0.973 / 0.996 / 1.037 | 0.700 / 0.701 / 0.707 |
| fresh-small-two-fe | 0.803 / 0.820 / 0.914 | 1.853 / 1.877 / 2.132 |
| fresh-zero-three-fe | 0.899 / 0.941 / 0.970 | NR |
| fresh-ols-control | 1.052 / 1.055 / 1.081 | 0.786 / 1.253 / 1.655 |
| fresh-small-oracle | 0.914 / 0.928 / 0.955 | 0.344 / 0.344 / 0.438 |

prior-narrow-k6: selected native baseline / candidate teams = 28 / 8.

fresh-narrow-k12: selected native baseline / candidate teams = 28 / 8.

fresh-boundary-k15: selected native baseline / candidate teams = 28 / 8.

fresh-boundary-k16: selected native baseline / candidate teams = 28 / 28.

fresh-wide-k32: selected native baseline / candidate teams = 28 / 28.

fresh-small-two-fe: selected native baseline / candidate teams = 4 / 4.

fresh-zero-three-fe: selected native baseline / candidate teams = 28 / 8.

fresh-ols-control: selected native baseline / candidate teams = 8 / 8.

fresh-small-oracle: selected native baseline / candidate teams = 16 / 8.

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

## Native thread scaling

| Lower → higher setting | Higher/lower min / median / max | Slower on every repetition |
|---|---:|---|

Independent-oracle failures: 0.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.

Native baseline is the accepted portable plugin `e9d9f706…`; candidate is
`8e2d5af2…`, snapshot `1f290d1c…`. Both use identical public requests, Stata
processors and process affinity. Only eligible three-FE PPML selects a smaller
native team. Stock is the pinned unmodified **default** fixest 0.14.2. PPML
includes robust inference, OLS IID inference, with no optional saved observation
outputs. This is not the earlier strict-reference or saved-output workload.

All **81 applications** and **27 native science pairs** pass, including actual
requested/effective teams. The small explicit-design Poisson/covariance/objective
oracle passes for both native arms in all three repetitions. Twenty-one stock
science pairs pass; six remain NR: K12 fails the coefficient envelope by a
maximum 2.563e-10 despite matching sample, covariance and objective; the zero
case differs in sample, coefficients and deviance. A pair discrepancy is not
proof of a native bug. The exact K12 input is separately registered for strict
stock adjudication; the original failures stay unchanged.

The requested28/selected8 policy improves K6/K12/K15 paired medians by
**14.4% / 10.4% / 7.3%**, with no slower pair in those nine comparisons. K15
versus comparable default stock is **0.591**; K6 is **0.613**. K12 stays NR.
Median process peaks fall approximately **120 / 207 / 80 MiB**, respectively.
The zero-outcome native median improves 5.9%; its two package samples differ,
so there is no stock timing rank. The independent small oracle median improves
7.2%, but it is science evidence, not a headline performance target.

Excluded K16/K32 medians are **0.985 / 0.996** of baseline; both contain one
slower pair. The two-FE control is 0.820, but the cap does not run there: do not
attribute its timing change to the selection mechanism. It still takes **1.877
times default stock** on this fresh input. The excluded OLS control is **1.055**
of baseline, slower in all three pairs (174/187 ms marginal medians), and takes
**1.253** of stock with broad 0.786–1.655 paired ratios. Its suspected native
regression requires a nine-pair confirmation. The candidate is not yet accepted
as regression-free. No native solver or tolerance will be tuned to these seeds.

The original K6 input must match its recorded hash before fits; all other cases
use predeclared fresh seed 202609284. The follow-up uses seed 202609285 for a
larger sample, equal-selected-team requests8/28 and a low-memory control. It
does not relabel this screen as fresh after observing it.

Job **7762740**: failed=0, exit_status=0, 677 seconds, 28 isolated slots on
scc-wo4, maxvmem 3.113G. Rotation and wrapper sentinels pass. Builds, profiling,
archiving and competing fits did not overlap the headline block. Full process
clocks and sampled per-process RSS remain separate in the JSON.

Decision at this checkpoint: retain the source candidate for bounded fresh
confirmation and the OLS regression check. The broad 0.90 goal, 1.20 ceiling,
all-core scaling guarantee, small categorical loss, low-memory OLS loss and
remaining comparison adjudications have not passed.
