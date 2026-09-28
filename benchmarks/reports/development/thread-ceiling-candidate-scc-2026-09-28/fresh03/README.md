# Fresh thread-ceiling confirmation and reference adjudication

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 19/19 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| fresh-larger-k10 | 0.865 / 0.942 / 0.950 | 0.675 / 0.688 / 0.727 |
| fresh-request-t8 | 0.946 / 0.995 / 1.022 | 0.837 / 0.893 / 1.004 |
| fresh-request-t28 | 0.797 / 0.811 / 0.827 | 0.745 / 0.822 / 0.862 |
| fresh-low-control | 0.920 / 1.053 / 1.088 | 0.568 / 0.618 / 0.657 |
| k12-strict-reference-adjudication | 0.822 / 0.860 / 0.973 | 0.069 / 0.073 / 0.086 |

fresh-larger-k10: selected native baseline / candidate teams = 28 / 8.

fresh-request-t8: selected native baseline / candidate teams = 8 / 8.

fresh-request-t28: selected native baseline / candidate teams = 28 / 8.

fresh-low-control: selected native baseline / candidate teams = 16 / 16.

k12-strict-reference-adjudication: selected native baseline / candidate teams = 28 / 8.

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

## Native thread scaling

| Lower → higher setting | Higher/lower min / median / max | Slower on every repetition |
|---|---:|---|

Independent-oracle failures: 0.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.

All **57 applications**, **19 native** and **19 stock** scientific comparisons
pass on unchanged candidate `8e2d5af2…`, snapshot `1f290d1c…`, versus baseline
`e9d9f706…`. Robust inference is included; optional saved observation outputs
are not requested. The first four cases use pinned unmodified **default**
fixest 0.14.2. Only the final K12 case uses the separately registered strict
reference (`glm.tol=1e-10`, `fixef.tol=1e-11`). Do not pool these references.

Fresh seed **202609285** confirms the policy at N983,040/K10: all three
candidate/baseline ratios are 0.865–0.950, median **0.942**. Marginal command
medians are 34.885/32.269 seconds; the paired stock ratio is **0.688**. Median
process peak falls about **728 MiB**. The five N245,760/K6 public-T28 pairs
improve **18.9%** at the paired median, all five faster (0.797–0.827), with
about **135 MiB** lower peak. Candidate/default-stock median ratio is **0.822**.

The public-T8 control already selected eight workers in baseline. Its paired
candidate/baseline median is **0.995**, spanning 0.946–1.022, two slower pairs.
Candidate/stock is **0.893**, with one individual ratio 1.004. For the same fresh
input, candidate command medians are **8.112 s at request8 / 8.082 s at request28**;
both select eight workers. The additional cross-request scientific comparison
passes every pair, including exact input/sample and native rank/DoF. Corresponding
repetitions give request28/request8 median **0.987**, range 0.955–1.020, one slower
pair. These are separately grouped request cases, not directly alternating
thread requests, so this is bounded evidence consistent with a plateau rather
than a universal monotonic-scaling guarantee. Process affinities still reflect
the public request.

The excluded low-memory control has mixed paired ratios **0.920 / 1.053 / 1.088**,
two losses. Its marginal command median falls 903→853 ms despite the losing
paired median; neither statistic may be omitted. Stock ratio is **0.618** and
median process peak is about 4 MiB lower. This block does not establish an
unchanged-path improvement or uniformly non-regressing controls. The separate
OLS control confirmation remains necessary.

The K12 adjudication repeats exactly the captured seed202609284 bytes and
keeps native source/settings unchanged. Strict stock agrees within **1.604e-13**
on coefficients and **1.409e-13** on standard errors in every repetition, with
sample, full covariance and objective checks passing. Default stock previously
differed by 2.563e-10 and failed the registered coefficient envelope. The stronger
reference's agreement points to its default stopping accuracy; it is not a new
independent large-design oracle. The prior default failures stay NR. Strict
stock is substantially slower (87.096 s marginal median versus candidate
6.476 s); the **0.073** paired ratio is strictly scoped to that stronger reference.

Job **7762780**: clean accounting, 1,243 seconds, 28 isolated slots on scc-wo4,
maxvmem 4.589G; rotation and wrapper sentinels pass. No compilation, profiling,
archiving or competing fits overlap this block. Scientific comparisons in this
block supplement the preceding independent Linux/Mac and small explicit-design
oracles; no new independent oracle was run on the large inputs.

The unchanged candidate is promising, with useful time and memory savings.
Production adoption remains pending the OLS regression confirmation and its
separate CPU attribution. Small categorical PPML, low-memory OLS, the remaining
original campaign adjudications and the full scaling/performance sweep remain
open. This does not complete the broader repair objective.
