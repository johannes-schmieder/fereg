# fereg implementation status

**Last updated:** 2026-08-24
**Development branch:** `main` only
**Latest accepted exact-green source:** `e4d807239e46974b3f5da6ff71f3ec5751eee3e1`
**Active milestone:** implement the frozen `slope_only_multiple` case
**Release state:** development only; public estimators remain fail-closed

`STATUS.md` is the mutable project-progress record. Governing requirements
live in `AGENTS.md`; accepted decisions live under `docs/adr/`; immutable
evidence lives under `.ci/`, `build_evidence/`, and
`benchmarks/reports/captured/`.

## Current accepted state

- The GitHub repository is the private `johannes-schmieder/fereg` repository.
  Development is consolidated on `main`; no compatibility branches or command
  aliases are maintained.
- The accepted source is
  `e4d807239e46974b3f5da6ff71f3ec5751eee3e1`. Its exact quick receipt is
  `.ci/stata/results/e4d807239e46974b3f5da6ff71f3ec5751eee3e1.json`; its exact
  full receipt is
  `.ci/stata/full-results/e4d807239e46974b3f5da6ff71f3ec5751eee3e1.json`.
  Both record `status=success`, `rust_status=success`,
  `stata_status=success`, and zero process, Rust, and Stata return codes. Quick
  passed 23/23 fixtures; full passed 27/27, including actual Break. Their
  `fereg_macos.plugin` SHA-256 values are respectively
  `97d10c4ef96614111b365bad33eb8f222344754c53f907ca7533260d56ed8dff`
  and
  `db06df9963e08ccb6a89e84b68e14569b4d34a7e18d0a321fca905ae5836d5be`.
- The repository rename/cleanup successor is
  `3ecf0eb80bfd62bce6a0e2918ec8c68f8ccb8f2f`. It also has exact-green quick
  and full receipts. The pre-rename cleanup checkpoint
  `69e8cd4038791db73c95e31439dd02b4b062ad2d` is rejected: its CI run was
  cancelled, no exact receipt exists, and its local Rust/source lane imports
  two deleted development-automation modules.
- The accepted hidden OLS projection executes 32 of 40 frozen semantic cases
  and retains eight named blockers. Its canonical receipt is
  `benchmarks/reports/captured/darwin-arm64/stata18/ols_hidden_numeric/20260824T180100Z-e4d8072`.
  The receipt has payload SHA-256
  `c6f0c06f2a4050976208a3df892546b7ef9d38978354f9a8c51bf6c346d72637`,
  file SHA-256
  `cd48f8150cb4ec706b71b1be6f511e2aabab8fc30474de7343335302f9ba8e79`,
  Git blob `5b3218badb9f8d49a0483865ab8a143bf078cb35` at mode `100644`, and source tree
  `74290da8a54318a3b7dc68d552577a5f8a3cd4ce`.
- The candidate plugin is `fereg_macos.plugin`, embeds
  `e4d807239e46`, and has SHA-256
  `a11fe8571f27cfdbee5f107ba1ff6153164ff1d182ec0dc17001abc6744be032`.
  All 32 comparator cases pass; the actual loader, fake-SPI store failures,
  licensed numerical parity, e-class rollback, and actual Break gates bind the
  same plugin. The receipt records `public_activation=false`, a clean warning
  audit, and `performance={status: unmeasured, claim: false}`.

## Option A absorbed-slope decision

- Successful OLS inference requires positive residual degrees of freedom.
  Signed residual-DoF result adapters, negative-IID covariance exceptions, and
  a Stata `e(df_r)` override are absent. Receipts from the rejected signed route
  remain immutable historical evidence only.
- `intercept_and_slope` uses the deterministic 48-row fixture. The pinned
  comparator reports `N=48`, conservative `df_a=26`, `df_r=22`, omitted `x1`,
  positive IID variance, and three DoF metadata rows. Certified MAP,
  coefficient/rank, score, and bread checks pass; the independent explicit-
  dummy oracle establishes actual model rank 22, mathematical residual DoF 26,
  residual parity, and RSS `.04`.
- The exact pinned parent receipt is
  `benchmarks/reports/captured/darwin-arm64/stata18/ols/20260824T161810Z-9f2494d`,
  with payload SHA-256
  `93b83a4ae7a6cfc0e6e79b37d3999647dbd6b37dff1d0ea3780a15bd644539d4`
  and file SHA-256
  `8ee6bba73322baefca41d064a02484fcf442e4754d11bb328c6c7c372d36e3ac`.
- The original 24-row over-absorbed construction is an atomic `r(2001)` test:
  zero completed stores, no covariance/statistics/receipt, no requested
  residual, and exact restoration of prior named or empty `e()` state.
- The guarded Option A migration script is deleted and repository checks reject
  its return. Useful intercept-plus-slope source preparation, compact topology,
  certified MAP, allocation bounds, and independent-oracle work remain.

## Public and package boundary

- `fereg, version`, `fereg, doctor`, and `fereg, selftest` are operational.
  Public `fereg` estimation and `feppml` return `r(498)` without posting a
  partial estimate. The legacy commands are absent from isolated installation.
- Active artifacts are `fereg_windows.plugin`, `fereg_linux.plugin`, and
  `fereg_macos.plugin`; the dispatcher is `fereg_dispatch`. Numeric protocol
  and package versions remain unchanged because their wire layouts did not
  change.
- Historical receipts, reference probes, captured reports, and build evidence
  remain immutable even where they retain the former project name.

## Milestone exit criteria

Public `fereg` OLS is complete only when the full OLS 1.0 surface in
`AGENTS.md` is implemented and independently validated. Every enabled frozen
case must retain exact sample/status/metadata parity and registered numerical
agreement; failures must remain atomic; fast and low-memory modes must agree;
the frozen benchmark speed/RSS gates must pass; and parsing, prediction, saved
outputs, postestimation, packaging, and cross-platform release gates must be
reviewed separately. PPML and release publication are later milestones.

## Completed, evidence-backed baseline

- Rust/plugin foundation, model wire 2.1, numeric sample preparation, compact
  categorical factorization, singleton pruning, certified MAP, prepared OLS
  fit/bread, IID/HC1/clustered inference, DoF, and private shadow transport are
  implemented for the accepted bounded surfaces.
- Hidden result posting is guarded by private shadows, receipt-last completion,
  exhaustive output-store failure tests, prior-estimate restoration, and
  actual Control-C rollback tests.
- The exact pinned offline `reghdfe`/`ftools`/`require` closure is materialized,
  resealed, and used by the active comparator.
- Repository/Python/static checks, formatting, Clippy with denied warnings,
  full Rust workspace and FFI tests, universal macOS build/audit, actual loader,
  licensed quick/differential/full profiles, and the frozen hidden projection
  are green for the accepted source chain.

These are development checkpoints, not public-estimator availability claims.

## Active checkpoint — `slope_only_multiple`

Implement and qualify multiple absorbed slope-only terms without weakening the
positive-DoF policy. The gate requires an independent explicit-dummy oracle,
exact pinned sample/status/DoF/VCE evidence, certified allocation and solver
behavior, receipt-last storage, licensed rollback/Break, and an exact-source
candidate reseal. Public dispatch remains disabled.

## Current blockers — frozen manifest

The accepted candidate executes 32 of 40 frozen semantic cases. The eight
remaining blockers, in order, are:

1. `slope_only_multiple` — multiple absorbed slope-only terms.
2. `string_fe` — string-source ingestion.
3. `time_series_lag` — Stata time-series lag materialization.
4. `time_series_lead_difference_gaps` — lead/difference semantics across gaps.
5. `iweight_probe` — importance-weight semantics.
6. `dof_continuous` — continuous-slope DoF policy and evidence.
7. `saved_outputs` — absorbed contribution and named FE recovery.
8. `postestimation_test_summarize` — public postestimation.

Broader factor/time-series parser behavior, `xb`/`xbd`, public syntax,
performance, low-memory execution, packaging, and cross-platform qualification
remain activation work after these cases close.

## Next evidence to record

For `slope_only_multiple`, record the exact source SHA, independent dummy
oracle, pinned sample/status/DoF/VCE evidence, allocation and solver
certificates, receipt-last storage, licensed rollback/Break, and a source-bound
candidate reseal that remains `public_activation=false` and performance
unmeasured.
