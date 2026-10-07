# Local benchmark

Four fixed-seed OLS/PPML models exercise one and two fixed effects. The `smoke`
tier uses 10,000 rows and one worker; `standard` uses 100,000 rows and 1/8 workers.
Each has a strict-tolerance reference, one warm-up and five fresh Stata processes.
Preparation is outside timing. This is a descriptive development benchmark,
not an acceptance campaign or an independent scientific oracle.

With Python 3.11+, NumPy and pandas installed, and the current plugin built:

```sh
python3 bench/run.py --tier smoke --stata "$STATA_BIN" --output target/bench-smoke
python3 bench/compare.py target/bench-smoke/run.json
python3 -m unittest discover -s bench/tests
```

The output directory must be new. Runtime/frontend hashes are checked throughout;
inputs are hash-bound and round-trip verified. Failed commands and partial runs
remain visible. Comparisons retain coefficient, covariance, scalar, exact sample,
and requested-output checks, with a bounded immutable-reference cache. Timings use
Stata's timer and report medians by workload; they cannot establish small changes
below its resolution. Dedicated performance decisions require a separately declared
bounded protocol under `docs/development_workflow.md`.

Generated captures belong under `target/` and can be deleted after inspection.
Historical campaigns and the legacy comparator are available only in Git history.

## Packed SCC comparison

`campaign.json` and `campaign.py` define the current bounded SCC comparison:
56 configurations and three repetitions, with a two-way worker/firm AKM baseline
at one million rows and 16 controls. Four workers are primary; 1/8/16-worker
variations and selected 16-worker clustered/saved-output cases remain on the
same node as their baseline. Stata comparators are included only at 1/4 workers.
The broader one-/two-/three-FE dense/sparse panel uses 100,000/1,000,000/4,000,000
rows. Clustering is by worker. No proprietary dataset is required.

`scc/setup.sh` builds one exact committed source, verifies the reused pinned
Stata closure, freezes executable/frontend/comparator/harness bytes and tests
the tooling. `scc/job.sh` is the compute entrypoint: 28 reserved physical cores
on exactly 28-core nodes, sequential fits, explicit affinity, three GB/core and
four-hour tasks. Run an independent small explicit-design smoke before the four
production bundles. Setup, smoke and production accounting, licensed sentinels,
manifest coverage and frozen identities must all be checked at collection.

The stopwatch covers estimation, inference and requested individual FE recovery.
For fixest this includes lazy extraction and observation-level materialization.
Data preparation/import, small initialization and disk export are excluded.
The PPML fixest profile disables its K correction, applies the declared robust
N/(N-1) correction inside timing, and preserves its original covariance alongside
the aligned covariance. Default and stricter convergence profiles remain separate.
Stock Stata covariance differences remain explicit failed comparisons.
Samples, all finite coefficient/covariance coordinates, objectives and saved
predictions are checked at SPEC tolerances. Only three valid paired repetitions
enter timing ratios; failed, timed-out, skipped and unattempted cells remain visible.
The small dummy-design oracle validates coefficients, full covariance and fitted
values independently; it does not adjudicate every large production discrepancy.

Generated inputs, row captures and logs remain in the remote run; collect compact
JSON/CSV summaries and the final PDF. This descriptive campaign establishes no
universal speed claim or platform qualification.
