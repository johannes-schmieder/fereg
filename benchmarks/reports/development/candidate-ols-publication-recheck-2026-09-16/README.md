# OLS publication timing recheck

The original saved-output slowdown does not reproduce in this bounded check.
Three alternating baseline/candidate pairs give command medians of
**0.461/0.440 seconds (-21 ms, -4.56%)**. Three interleaved controls using the
same baseline artifact give **0.423/0.437 seconds (+14 ms, +3.31%)**.
Neither median comparison crosses both the 5% and 10 ms review thresholds.

The controls still vary substantially: their third pair is 0.464/0.547 seconds
(+83 ms, +17.89%) despite identical source, binary, input and options. This
precludes attributing the original flag to the prototype or claiming a speed
gain. The original failed receipt remains unchanged. The automated summary's
`slowdown_not_reproduced_in_controlled_check` describes the new medians;
the broader attribution remains indeterminate, as recorded in `validation.json`.
The prototype remains uncommitted and unadopted. No further fit is scheduled.

| Pair | Baseline (s) | Candidate (s) | Difference (ms) | Control left (s) | Control right (s) |
|---|---:|---:|---:|---:|---:|
| 1 | 0.461 | 0.440 | -21 | 0.423 | 0.437 |
| 2 | 0.469 | 0.530 | +61 | 0.422 | 0.419 |
| 3 | 0.397 | 0.388 | -9 | 0.464 | 0.547 |

`plan.json` fixes the interleaving, AB/BA/AB ordering, thresholds and no-retry
rule before execution. The case has 30,720 rows, analytic weights, one FE,
eight numeric controls plus factor interactions, saved FE/residual output,
four threads and low memory. Each command starts in a fresh licensed Stata
process. Both arms reuse the original isolated Linux artifacts without a
rebuild. All 571 baseline/candidate source-file bindings and both artifacts
are checked before execution. The input and reference-lock hashes are retained;
no external reference or German estimation is rerun.

SCC job **7590027** ran on `scc-gr4`, with four slots and four-core process
affinity. All 12 commands have rc=0, the fixture sentinel and exact scientific
comparisons. Coefficients, covariance, rank/status, DoF, FE diagnostics,
certificates, saved columns, sample/order and non-time solver fields match.
Scheduler failed=0 and exit_status=0, and the terminal sentinel is
`OLS_PUBLICATION_CONTROL_CHECK_COMPLETE`. Job wall time is 19 seconds;
scheduler maxvmem is 1.089G, a job-level virtual-memory measure rather than an
estimator allocation peak. The previous original screen ran on `scc-tb4`;
its one-pair traced diagnostic ran on `scc-ei3`. These host differences limit
cross-job timing comparisons.

| Median metric | Baseline | Candidate |
|---|---:|---:|
| Preflight (ms) | 0.454 | 0.462 |
| Input (ms) | 7.776 | 7.779 |
| Native execution (ms) | 294.830 | 304.558 |
| Plugin through pre-store (ms) | 301.884 | 317.314 |
| Remaining complete-command time (ms) | 131.501 | 122.686 |
| Reported native allocation (bytes) | 8,998,688 | 8,998,688 |
| Reported scratch (bytes) | 5,898,240 | 5,898,240 |
| Sampled process-tree peak RSS (bytes) | 69,853,184 | 74,280,960 |

Medians are computed independently and need not sum. Plugin-through-pre-store
time rises 15.4 ms / 5.1%; native time rises 9.7 ms / 3.3%. Neither identifies
the cause of the conflicting whole-command results. All twelve arms have
identical reported allocation and scratch. Sampled RSS ranges are
69,644,288–70,295,552 bytes for baseline and 70,639,616–86,560,768 for candidate;
unchanged-artifact controls span 69,525,504–79,396,864. These sampled process
figures do not establish whole-action byte coverage. Reported fallback/retry
fields are zero; discarded-attempt time is not separately observed and remains
null. Phase receipts and every arm are retained rather than selecting fast runs.

Fresh repository, OLS v20 inventory and action v3 inventory checks pass. All
162 native files still match the previously tested snapshot. This check does
not rerun or enlarge the earlier 189 native / 40 Python test set. No estimator
source, numerical policy, installed binary or historical receipt changed.

Whole-action coverage remains incomplete, including OLS preparation/inference
parents, slope recovery, compiler/topology metadata and opaque dependency/pool
capacities. Its ratio remains undefined. The Mac/Linux review candidate is
still blocked on that work and the remaining platform/freeze/package/install
gates. This is development evidence, not formal qualification. No commit,
push, tag, release or installed-package replacement was made by this check.

`identity.json`, `artifacts.json`, `harness-hashes.json` and
`raw-receipt-hashes.json` bind source, binaries, harness and collected evidence.
`phase-results.json` and `phase-medians.json` are derived from the untouched
`partial-results.json`. Licensed Stata logs and data remain outside Git.
