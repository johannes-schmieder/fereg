# Current-source OLS/PPML sweep: validated launch

This immutable launch receipt registers a new development sweep of accepted
package source at **161b9491**, after the performance and accuracy repairs.
It is not a stopped-campaign resumption or release qualification. Production
results will be collected separately; submission is not completion.

## Identity and unchanged scope

Package snapshot **c09eeab353a8445aae85f75891f112964246e876c4865ec9e2f757d83922e236**,
259 bound inputs; Linux plugin
**59d0341ae2c760b4701381e1a2153f1238ab94d752457429bb8642380f503920**.
Setup reuses the previously accepted artifact after checking both the original
source and the extracted archive against every input hash. No new estimator
build or production-code change is part of this milestone.

Measurement fingerprint:
`fb2ad5407afcea22d56155aa7858203bfcaea590f750ca5d06eeccce746cffc6`.
The exact source inventory, controller/deployment hashes, artifact origin and
submitted plans accompany this report. Run label: `20260928-current-sweep-161b9491`.

The original **140 configurations** (68 OLS, 72 PPML), four warm seeds and eight
cold anchors remain: **568 groups**, N61,440/983,040/3,932,160, K10 with selected
K50/K6, requested threads 1/4/8/28, categorical and multi-partition slopes,
fast/low modes and zero outcomes. All scientific settings, seeds and generator
parameters match the completed original campaign. The generator bytes and pinned
Python environment also match. All eight pilot DTA/column/metadata receipts
match their original captured inputs exactly.

Commands are public `fereg` versus stock `feols` with IID VCE, and public `feppml`
versus default and strict stock `fepois` with robust VCE under the unchanged
normalization. Strict uses glm.tol=1e-10 and fixef.tol=1e-11. All inputs are
synthetic and unweighted. This sweep does not time clustering, saved FE or
fitted-mean publication; their separate scientific fixtures are not timing
coverage. Configured thread requests and selected native teams are captured
separately; a capped request is not actual additional-core scaling.

Only unchanged alternative commands are omitted: reghdfe/areg/greg and
ppmlhdfe/gglm/pyfixest retain their earlier evidence and unresolved adjudications.
**50 exact historical reference timeout attempts** are explicit budget exclusions,
not current failures or assumed timing wins. Successful neighboring seeds stay
live. The two strict three-FE slope pilot timeouts stay live because subsequent
stock adjudication completed those inputs. No native case is excluded, and
scientific discrepancies remain unranked. The original manifests and 77/16/111
frozen definitions are unchanged.

## Preflight and pilot evidence

Four new local registration tests pass, preserving the full grid/native coverage,
seed-specific skips and valid pilot selection, and rejecting incomplete or
duplicate history. Setup **7765906** passes **31 controller tests** under the
established SCC scientific runtime: 18 seconds, 1.451G maxvmem, four slots,
failed=0/exit_status=0. Local missing-dependency and collection-environment issues
are recorded in `preflight-notes.json`; they did not change any scientific test
or require a compute-job retry.

Whole-node smoke **7765908**: **72/72 commands complete**, all **28 independent
native-oracle groups pass**; 29/44 reference pairs pass and **15 remain scientific
discrepancies**. Accounting is clean: 405 s / 2.148G, 28 slots. Full covariance, standard
errors, exact sample, coefficients, objective and convergence are checked. The
separate saved-export comparison to the same independent oracles passes all
28 native, 16/28 default-stock and 13/16 strict-stock outputs. This does not change
pair eligibility or assign one cause to different sample/DoF/convergence failures.

For small weak-d3 PPML N7680/K10/T4, native and strict stock pass the independent
oracle. Default stock's maximum SE error is **1.283 times the permitted envelope**;
native is 2.87e-7 envelope units and strict stock 3.13e-5. This localizes this small
case to reference accuracy. It does not adjudicate the separate fresh
N30720/K6/seed2026092811 discrepancy. No new fit was run for this analysis.

Pilot **7765992**: **20/20 commands complete**, all eight configurations have a
valid reference pair; 10/12 pairs pass. Two default-stock slope PPML comparisons
remain NR; both strict pairs pass. Original inputs match byte-for-byte.
Accounting is clean: 268 s / 5.817G, 28 slots. The predeclared controller and scheduler
pilot limit was two hours; scientific groups/orders/per-arm limits were unchanged.

These are **single-seed pilot observations**, not four-repeat headline medians.
Ratios below one favor fereg; NR means failed scientific comparison.

| Model / design / section | N / K / requested threads | Native seconds | / default stock | / strict stock |
|---|---:|---:|---:|---:|
| ols / strong_d6 / main | 3,932,160 / 10 / 28 | 1.798 | 0.956 | — |
| ppml / strong_d6 / main | 3,932,160 / 10 / 28 | 11.355 | 1.498 | 1.292 |
| ppml / two_fe_dense / slopes_low | 61,440 / 6 / 4 | 0.453 | NR | 0.252 |
| ols / two_fe_dense / main | 983,040 / 10 / 28 | 0.576 | 0.517 | — |
| ppml / two_fe_dense / main | 983,040 / 10 / 28 | 3.851 | 1.029 | 0.656 |
| ppml / two_fe_dense / slopes_fast | 61,440 / 10 / 28 | 0.436 | NR | 0.340 |
| ols / two_fe_dense / slopes_low | 61,440 / 6 / 4 | 0.301 | 1.505 | — |
| ols / two_fe_dense / slopes_fast | 61,440 / 10 / 28 | 0.196 | 1.289 | — |

The large degree-six PPML and both OLS slope cases still breach 1.20 in this pilot.
The two PPML slope cases beat comparable strict stock substantially. Do not
transfer either conclusion to other seeds or earlier artifacts.

## Submitted production work

**7766045/tasks 1–4**, concurrency 4, each reserves 28 E5-2680v4 cores and 3G/core
with a 12-hour cap. Every native/reference group executes sequentially on one
reserved node. Setup, profiling, compilation and archiving are outside timings.
Pilot groups are reused only under the same frozen identity. The remaining
**560 groups/1,358 applications** plus 20 pilot applications yield 1,378 planned
main applications, versus 3,408 in the earlier broader-comparator campaign.

Forecast: **39.063 node-hours**, task forecasts 9.764/9.777/9.765/9.757 hours,
133/145/139/143 groups. These are planning estimates, not measured runtime savings.
The controller checkpoints complete groups and preserves pending groups near the
deadline. Never rerun completed groups or treat scheduler exit zero as complete
science. Require all task accounting, terminal markers, exact plan/fingerprint
bindings and the complete group audit before summarizing distributions.

The universal 1.20 ceiling, 0.90 aspiration and repeatable core-scaling goal remain
unmet. This sweep will identify current-source losses and failures; it does not
close comparator sample/VCE/extraction work, constrained-fit failures, static
allocation drift or physical-memory/stack qualification. The eight original
sparse-zero failures and three original PPML coefficient discrepancies already
have separate repair evidence; historical pending entries do not reopen them.
