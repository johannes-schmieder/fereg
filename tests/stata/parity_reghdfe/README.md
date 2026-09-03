# OLS parity tests

`fereg` is public. This directory contains source-bound differential fixtures
and evidence contracts; each receipt supports only the exact historical
checkpoint it names.

The first categorical-interaction reference gate is the successful,
source-bound receipt:

- source: `e81b942c0dc0b122258489a60c956cdbe0121512`
- receipt: `.ci/reference-probes/fe-interaction/e81b942c0dc0b122258489a60c956cdbe0121512.json`
- sealed payload: `1fba54c0adce40ec52964e1edd1b3e57283bc803cb07009a84933b241e630955`
- pinned `reghdfe`: `29b2b203f534413369d675d0c0f674d61291f536`

`estimate_ols_categorical_interaction_parity.do` consumes the committed
`target_default_firm_year` case from that sealed receipt. It checks the hidden
fereg sample after iterative singleton pruning, coefficient/status layout,
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
the mathematical authority for the corresponding compact fit. The accepted
rebasing checkpoint is source `0e2aa609f9cbb17f8a205261d80680e68a752cef`
and consumes
the sanitized reference receipt
`.ci/reference-probes/fe-interaction/0e2aa609f9cbb17f8a205261d80680e68a752cef.json`,
whose sealed payload is
`fab875a998edc8f91bbeebfca8ca8ef92c09f43f6044fbbcef0b1e7da8fcf387`.
The exact-source licensed full-profile receipt records 25 passing fixtures at
`.ci/stata/full-results/0e2aa609f9cbb17f8a205261d80680e68a752cef.json`.

Each committed fixture consumes the sealed receipt and does not invoke `reghdfe`; this executable differential set does not close `multi_fe_interactions`.
Python contracts independently verify the receipt seal and every oracle constant
used by the Stata fixtures. The accepted rebasing gate does not expand the
frozen semantic projection or activate a public estimator.

The active frozen-projection gate at source
`6bbe04d857a293f0fa1fb0f617683be9ad5b4ea1` executes both the complete
`multi_fe_interactions` manifest case and the guarded `intercept_and_slope`
case. Its sealed 32-case/8-blocker receipt is
`benchmarks/reports/captured/darwin-arm64/stata18/ols_hidden_numeric/20260824T114506Z-6bbe04d`.
That historical receipt makes no current public-surface or performance claim.

`estimate_ols_intercept_slope_parity.do` consumes the frozen
`intercept_and_slope` comparator case on its dedicated topology-preserving
48-row fixture. It binds the hidden omitted coefficient, positive iid
covariance, positive `df_r=22`, three-row extended absorbed-DoF table, solver
and transaction
certificates, the reference inexact slope-count flag, and the then-current
public-failure condition. The independent Rust explicit-dummy oracle remains
the mathematical authority for projection rank, residuals, RSS, bread, and
covariance formula;
the licensed comparator supplies pinned Stata-facing metadata.

`estimate_ols_intercept_slope_overabsorbed_fail_closed.do` retains the former
disconnected 24-row construction solely as a failure oracle. Its nonpositive
conservative residual DoF returns `r(2001)`, exposes no residual or completed
receipt, and restores both populated and null prior `e()` state. It is not an
executable success case.

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
never invokes fereg. Missing or stale closure members fail before Stata
launches.

Raw Stata logs and the machine capture stream are temporary licensed artifacts
and must not be copied into the repository. Only a sanitized, read-only,
SHA-256-sealed receipt may be committed after review. Every successful Stata
and machine stream requires its unique terminal `END_FIXTURE|...|rc=0`
sentinel; process exit status alone is never sufficient.
