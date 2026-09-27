# Original campaign issue inventory

This immutable inventory covers all 3,408 arm records and 2,840 pair records
from source `58284f76`. It indexes **472 unsuccessful/excluded arm records**
and **1,258 timing-ineligible pair records**, including passing comparisons
without exact sample evidence. Each entry names its frozen CSV line, case,
repetition, symptom, and captured input/result hashes. Entries can overlap:
an unavailable pair and its failed underlying arm are distinct records.

`benchmarks/performance/issue_inventory.py` regenerates the JSON from the
captured report. It asserts complete coverage and a captured input receipt for
every entry. It never changes scientific eligibility or attributes a root cause
from a failed check alone. The separate performance list retains all **53**
complete configuration/reference comparisons above 1.20: 21 OLS/default,
19 PPML/default and 13 PPML/strict. These populations overlap and are not
53 distinct native configurations.

## Evidence-backed disposition at this checkpoint

| Issue family | Evidence and remaining work |
|---|---|
| Sparse-zero native failures | ADR 0230 repairs the exact original two-/three-FE diagnostic seed locally, with explicit integer sample and Poisson/Hessian oracles. Linux smoke is pending; all original four-seed configurations have not been rerun. |
| Other native timeouts | ADR 0228 repairs excessive Schwarz admission at high thread counts. The paired SCC block confirms large improvements at N983040; the original N3932160 timeout inputs still require remeasurement. |
| OLS fit/stopping differences | The bounded independent adjudication validates native small explicit designs. Large degree-two strict-reference replay remains open; a local reconstruction failed the literal input guard before any estimator ran. |
| OLS inference differences | Exact small rank checks establish two-FE and three-FE discrepancies. Stock `K.exact=TRUE` repairs the small two-FE example but is wrong by one on the three-FE example. No blanket reference correction is authorized by those results. |
| PPML coefficient differences | Original three strict-reference disagreements, and their default-reference counterparts, still require captured-input adjudication. A pair disagreement is not proof of a native bug. |
| PPML inference/objective differences | Pyfixest normalization and the remaining stock accuracy failures remain open. |
| PPML sample differences | Gglm sample/separation/singleton treatment remains open. |
| Gtools exact samples | 348 numerically passing pair records lack required exact sample evidence. Supported extraction or isolated instrumentation remains open; no timing ranks are assigned. |
| Reference execution failures | Warm-up, initialization, iteration-cap and command-timeout mechanisms remain separately visible in original receipts. Harness failure-stage/capacity improvements are committed, but they do not repair the underlying reference algorithms. |
| Unsupported surfaces and budget exclusions | Classified exclusions, not observed native failures. No unchanged expensive arm has been restarted. |
| Performance breaches | The original 53 comparisons remain historical targets. Memory admission has current paired evidence; OLS slope ownership is under fresh confirmation. Thread ceilings, larger categorical costs and the full 140-configuration sweep remain open. |

Relevant new evidence: [memory diagnosis](../performance-repair-diagnostics-2026-09-27/README.md),
[paired SCC rotation](../performance-repair-paired-scc-2026-09-27/progress-14/README.md),
[separation proof](../separation-coefficient-proof-local-2026-09-27/README.md),
[OLS reference adjudication](../ols-comparator-adjudication-local-2026-09-27/README.md).
These links describe their own exact sources and tested populations; they do
not retroactively change the original campaign or resolve every record in a
family. The full implementation objective remains incomplete.
