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

The external root must contain clean exact-commit `reghdfe/` and `ftools/`
checkouts. The runner stages committed bytes, isolates Stata system ado paths,
and executes `benchmarks/stata/baseline_reghdfe.do`. It never invokes StFixest.

At present `references.lock.toml` marks the exact `ftools` commit unresolved,
so preflight must fail before Stata launches. Do not replace that gate with an
installed `ftools`, a version string, a floating branch, or an invented hash.

Raw Stata logs and the machine capture stream are temporary licensed artifacts
and must not be copied into the repository. Only a sanitized, read-only,
SHA-256-sealed receipt may be committed after review. Every successful Stata
and machine stream requires its unique terminal `END_FIXTURE|...|rc=0`
sentinel; process exit status alone is never sufficient.
