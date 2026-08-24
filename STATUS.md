# StFixest implementation status

**Last updated:** 2026-08-24  
**Development branch:** `main` only  
**Latest accepted exact-green source under the current scientific policy:** `5a6729cbc33e90e0d7da36be70cdb503c8d4e250`
**Active milestone:** implement the frozen `slope_only_multiple` case
**Release state:** development only; all public estimators remain fail-closed

`STATUS.md` is the sole mutable project-progress record. Governing requirements
live in `AGENTS.md`; accepted architectural and scientific decisions live under
`docs/adr/`; immutable evidence lives under `.ci/`, `build_evidence/`, and
`benchmarks/reports/captured/`.

## Current state

- Development is consolidated on `main`; do not recreate historical feature,
  recovery, or Codex branches.
- The latest source accepted under the current option-A policy is
  `5a6729cbc33e90e0d7da36be70cdb503c8d4e250`. Its exact quick receipt is
  `.ci/stata/results/5a6729cbc33e90e0d7da36be70cdb503c8d4e250.json` and its exact full receipt is
  `.ci/stata/full-results/5a6729cbc33e90e0d7da36be70cdb503c8d4e250.json`.
  Both record Rust/source and licensed-Stata success with zero failures. The
  full profile passed all 26 configured fixtures plus the actual-Break gate
  (`tests_passed=27`).
- The accepted hidden OLS projection at that source executes 32 of the frozen
  40 semantic cases and retains eight named blockers. Its canonical receipt is
  `benchmarks/reports/captured/darwin-arm64/stata18/ols_hidden_numeric/20260824T153712Z-5a6729c`,
  payload SHA-256
  `5795efa5cff4adcaa7194b48de5d8fff2d4c7f64679a066529922b6362ada8a0`,
  file SHA-256
  `666b8f84ea644618c393f0a4003e52545356f6ac3b649b792dca8442650c7ffc`,
  and Git blob `3e4fe21d23ebef28a54dcb0540eb88e14f19648e` at mode `100644`.
  It records `public_activation=false` and no performance claim.
- Native categorical absorbed-FE interactions are substantially closed:
  all 12 sealed hidden-estimator differentials are exact-green at
  `dfd5ae0935f2b5a8925de14c316d699c5fff7e66`, and final-sample categorical
  rebasing is accepted at
  `0e2aa609f9cbb17f8a205261d80680e68a752cef`.
- A later experimental series made the 24-row absorbed-slope reference case
  executable by admitting signed residual degrees of freedom and negative iid
  covariance. Although that series produced green receipts, Johannes rejected
  that scientific policy and selected option A. Commits and receipts from that
  route, including `6bbe04d857a293f0fa1fb0f617683be9ad5b4ea1`, are historical
  evidence only and are not active acceptance evidence.
- Option A is exact-green. The signed result/statistics/covariance path and the
  negative-iid-diagonal exception are absent. The ordinary certified iid type
  requires `df_r > 0`. The deterministic 48-row fixture exactly duplicates the
  original FE topology, preserves omitted `x1`, reports `df_a=26` and
  `df_r=22`, and has positive `_cons` variance. Independent explicit-dummy
  evidence establishes actual model rank 22, mathematical residual DoF 26,
  residuals, and RSS `.04`.
- The exact pinned parent receipt is
  `benchmarks/reports/captured/darwin-arm64/stata18/ols/20260824T135913Z-830fad8`,
  payload SHA-256
  `9c4318e9996b41fae3de705ff97567a7be2a97136bb52a81090242fc86fb589e`
  and file SHA-256
  `fbb56dbd6abc23978dbf8563e6b05e57b188a8e71ef2be93e148d4beddde3f2b`.
  Relative to the prior accepted parent, only `intercept_and_slope` changes
  after timing fields are removed; the pinned references and all 10 benchmark
  scientific-evidence hashes are identical.
- The original 24-row over-absorbed construction returns `r(2001)` before any
  native result store, requested residual, covariance/statistics publication,
  or Stata transaction. Named and null prior estimation states are restored.
- `stfeols`, `stfepois`, public `estimate-ols`, and public `estimate-ppml`
  remain disabled and must continue to return `r(498)` without posting an
  estimate.
- Repository automation has been reduced to durable CI/reference workflows.
  Development and orchestration are now local; GitHub Actions is a test and
  evidence system, not a background coding agent.

## Milestone exit criteria

`stfeols` is complete only when the full OLS 1.0 surface in `AGENTS.md` is
implemented and independently validated. At minimum:

- every enabled frozen semantic case has exact sample/status/metadata parity and
  numerical agreement at the registered tolerances;
- failure paths remain atomic and do not silently weaken sample, weight, VCE,
  DoF, convergence, or certificate rules;
- fast and low-memory modes agree scientifically;
- the frozen benchmark suite has at least one cold and five warm repetitions at
  the required thread counts, geometric-mean speedup of at least 1.5x, no
  primary case slower than the reference, and no primary-case RSS regression;
- public parsing, prediction, saved outputs, postestimation, packaging, and
  cross-platform release gates are reviewed separately.

PPML, release signing, publication, and broad cross-platform certification are
outside the current OLS milestone.

## Completed, evidence-backed baseline

- Rust/plugin foundation, model wire 2.1, numeric sample preparation, compact
  categorical factorization, singleton pruning, certified MAP, prepared OLS
  fit/bread, iid/HC1/clustered inference, DoF, and private shadow transport are
  implemented for the currently accepted bounded surfaces.
- Hidden licensed-Stata result posting is transactionally guarded by private
  shadows, receipt-last completion, prior-estimate restoration, and actual
  Control-C rollback tests.
- The exact pinned offline `reghdfe`/`ftools`/`require` dependency closure is
  materializable and sealed.
- The categorical interaction differential and final-sample rebase gates are
  closed as described above.
- The current accepted frozen candidate is the 32-executable/8-blocked receipt
  at `5a6729cbc33e90e0d7da36be70cdb503c8d4e250`.

These are development checkpoints, not public-estimator availability claims.

## Active checkpoint — `slope_only_multiple`

Implement and qualify the next frozen blocker without weakening the option-A
positive-DoF policy. The gate requires an independent explicit-dummy oracle,
exact pinned-reference sample/status/DoF/VCE evidence, certified solver and
allocation behavior, receipt-last native storage, licensed rollback/Break, and
an exact-source hidden-candidate reseal. Public dispatch remains disabled.

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
performance, low-memory execution, packaging, and platform qualification remain
activation work even after these nine cases close.

## Next evidence to record

For `slope_only_multiple`, record the exact source SHA, independent oracle,
pinned comparator values, source-bound hidden receipt and plugin identity, full
Rust/source and licensed-Stata receipts, loader/store-failure sweeps,
rollback/Break results, and unchanged `public_activation=false` and performance
status.
