# OLS parity tests

`stfeols` remains disabled. This directory documents the licensed reference
boundary; executable StFixest-vs-reference parity fixtures belong here only
after the first sealed reference receipt is adjudicated.

The current reference workflow is:

```bash
python3 scripts/ols_reference.py validate
python3 scripts/ols_reference.py preflight --reference-root /absolute/external/reference-root
python3 scripts/ols_reference.py capture \
  --reference-root /absolute/external/reference-root \
  --receipt benchmarks/reports/captured/<platform>/<stata>/ols-v1/receipt.json
python3 scripts/ols_reference.py verify \
  --receipt benchmarks/reports/captured/<platform>/<stata>/ols-v1/receipt.json
```

The external root must contain clean exact-commit `reghdfe/`, `ftools/`, and
`require/` checkouts. The runner checks every locked tracked-file SHA, stages
committed bytes, compiles the generated ftools Mata library only in the private
stage, isolates Stata system ado paths, and executes
`benchmarks/stata/baseline_reghdfe.do`. It never invokes StFixest. Missing or
stale closure members fail before Stata launches.

Raw Stata logs and the machine capture stream are temporary licensed artifacts
and must not be copied into the repository. Only a sanitized, read-only,
SHA-256-sealed receipt may be committed after review. Every successful Stata
and machine stream requires its unique terminal `END_FIXTURE|...|rc=0`
sentinel; process exit status alone is never sufficient.
