# OLS parity tests

`stfeols` remains disabled. This directory contains developer-only differential
fixtures and evidence contracts; none of them enable the public estimator.

The first categorical-interaction reference gate is the successful,
source-bound receipt:

- source: `e81b942c0dc0b122258489a60c956cdbe0121512`
- receipt: `.ci/reference-probes/fe-interaction/e81b942c0dc0b122258489a60c956cdbe0121512.json`
- sealed payload: `1fba54c0adce40ec52964e1edd1b3e57283bc803cb07009a84933b241e630955`
- pinned `reghdfe`: `29b2b203f534413369d675d0c0f674d61291f536`

`estimate_ols_categorical_interaction_parity.do` consumes the committed
`target_default_firm_year` case from that sealed receipt. It checks the hidden
StFixest sample after iterative singleton pruning, coefficient/status layout,
covariance, absorbed and residual degrees of freedom, interaction DoF identity,
and observation residuals.

`estimate_ols_categorical_interaction_keepsingletons_parity.do` consumes the
companion `target_keepsingletons` case. It checks the all-observation sample,
zero singleton deletion, the expanded worker and interaction topology, iid
coefficient/covariance output, and all 24 residuals.

Neither fixture invokes `reghdfe`; Python contracts independently verify the
receipt seal and every oracle constant used by the Stata fixtures. These two
executable cases do not close `multi_fe_interactions`. The remaining accepted
receipt cases still need differential coverage, and the frozen semantic
projection must be resealed before the status gate can close.

The general licensed reference workflow is:

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
stage, isolates Stata system ado paths, and executes the reference do-file. It
never invokes StFixest. Missing or stale closure members fail before Stata
launches.

Raw Stata logs and the machine capture stream are temporary licensed artifacts
and must not be copied into the repository. Only a sanitized, read-only,
SHA-256-sealed receipt may be committed after review. Every successful Stata
and machine stream requires its unique terminal `END_FIXTURE|...|rc=0`
sentinel; process exit status alone is never sufficient.
