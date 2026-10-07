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
