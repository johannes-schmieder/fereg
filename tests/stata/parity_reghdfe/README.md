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

`estimate_ols_categorical_interaction_cascade_parity.do` consumes
`tuple_second_fe_cascade`. It checks a non-collinear continuous slope through a
four-observation singleton cascade, with `obs_id` 101--108 retained and
109--112 deleted, plus the retained interaction topology and DoF, the full iid
covariance matrix, and retained-sample residuals.

`estimate_ols_categorical_interaction_fweight_parity.do` consumes
`fweight_frequency_cascade`. It checks frequency-mass singleton semantics: a
frequency of two on the last physical row prevents the cascade, all 12 physical
rows remain while effective N and `sumweights` are 13, and coefficient,
covariance, DoF, weight metadata, and residual output match the sealed payload.
Finite-coordinate first-order conditions are certified directly from the same
pre-QR FWL Gram and RHS cross-products used by the solve, while the independent
FE certificate continues to govern the absorbed span. The normalized reported
intercept retains its raw score moment and all injected score mutations still
fail closed.

`estimate_ols_categorical_interaction_missing_parity.do` consumes
`missing_interaction_components`. It checks that missing outcomes and missing
components of `firm#year` are removed before final topology construction even
under `keepsingletons`: observations 201--208 and 212 remain, observations
209--211 are absent from `e(sample)`, residuals are missing outside the sample,
and the retained coefficient, covariance, DoF, and interaction identity match
the sealed payload.

`estimate_ols_categorical_interaction_repeated_parity.do` consumes
`repeated_identical_interaction`. It checks that two requested `firm#year`
terms retain their separate canonical rows while reusing one numerical
partition: the second row contributes the identified interaction span, the
third is structurally fully redundant, and sample, coefficients, covariance,
and residuals remain identical to the single-interaction case.

`estimate_ols_categorical_interaction_remaining_iid_parity.do` consumes the
`missing_interaction_components`, `repeated_identical_interaction`,
`interaction_order_year_firm`, `dof_none`, and `dof_firstpair` cases. It binds
missing-component sample exclusion, repeated and reordered interaction identity,
and the exact none/first-pair absorbed-DoF policies.

`estimate_ols_categorical_interaction_cluster_parity.do` consumes the
`nested_simple_cluster`, `identical_interaction_cluster`, and
`unrelated_interaction_cluster` cases. It binds cluster identity, nesting,
cluster-aware absorbed and residual DoF, covariance, sample, and residual
transport for simple and declared-interaction clusters.

`estimate_ols_categorical_final_sample_rebase.do` binds the exact-pinned
reference behavior when singleton deletion removes an entire ordinary factor
level. It checks final-sample base/name rebasing for a factor main effect and a
factor interaction, native status and transaction invariants, and fail-closed
explicit/FV-set base policies. The independent explicit-dummy Rust oracle is
the mathematical authority for the corresponding compact fit.

Each committed fixture consumes the sealed receipt and does not invoke `reghdfe`; this executable differential set does not close `multi_fe_interactions`.
Python contracts independently verify the receipt seal and every oracle constant
used by the Stata fixtures. The remaining accepted receipt cases still need
differential coverage, and the frozen semantic projection must be resealed
before the status gate can close.

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
