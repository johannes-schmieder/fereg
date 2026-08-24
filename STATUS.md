# StFixest implementation status

**Last updated:** 2026-08-24  
**Development branch:** `main` only  
**Latest accepted exact-green source under the current scientific policy:** `f61fee923db2bd3544d457dfe50de8117d27adda`  
**Active milestone:** apply and qualify option A for the frozen `intercept_and_slope` case  
**Release state:** development only; all public estimators remain fail-closed

`STATUS.md` is the sole mutable project-progress record. Governing requirements
live in `AGENTS.md`; accepted architectural and scientific decisions live under
`docs/adr/`; immutable evidence lives under `.ci/`, `build_evidence/`, and
`benchmarks/reports/captured/`.

## Current state

- Development is consolidated on `main`; do not recreate historical feature,
  recovery, or Codex branches.
- The latest source accepted under the current option-A policy is
  `f61fee923db2bd3544d457dfe50de8117d27adda`. Its exact full qualification
  receipt records Rust/source success, licensed-Stata success, and 25 passing
  full-profile fixtures:
  `.ci/stata/full-results/f61fee923db2bd3544d457dfe50de8117d27adda.json`.
- The accepted hidden OLS projection at that source executes 31 of the frozen
  40 semantic cases and retains nine named blockers. Its canonical receipt is
  `benchmarks/reports/captured/darwin-arm64/stata18/ols_hidden_numeric/20260824T104412Z-f61fee9`.
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
- Option-A preparation is present on `main`: the deterministic 48-row fixture
  `tests/fixtures/ols/ols_intercept_slope_positive_dof.csv`, its contract test,
  and the guarded migration script
  `ci/source-transforms/apply_option_a_absorbed_slope.py`. The migration has not
  yet been accepted by an exact-SHA Rust/licensed-Stata receipt.
- The original 24-row over-absorbed construction must remain a fail-closed
  insufficient-observations test. It must never become a successful result with
  negative variance.
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
- The current accepted frozen candidate is the 31-executable/9-blocked receipt
  at `f61fee923db2bd3544d457dfe50de8117d27adda`.

These are development checkpoints, not public-estimator availability claims.

## Active checkpoint — option A absorbed slopes

Implement the accepted decision in
`docs/adr/0006-positive-residual-dof-for-absorbed-slopes.md`:

1. Review and apply the guarded migration in
   `ci/source-transforms/apply_option_a_absorbed_slope.py`; do not accept it
   merely because the transform runs.
2. Remove the superseded signed-residual-DoF result path and restore the
   ordinary invariant `df_r > 0` for successful OLS inference.
3. Keep the useful guarded intercept-plus-one-slope preparation, topology,
   certified MAP, allocation, and explicit-dummy work.
4. Bind the frozen `intercept_and_slope` command to the dedicated 48-row
   positive-DoF fixture while leaving every unrelated semantic case unchanged.
5. Preserve the original 24-row design as a fail-closed test with no completed
   covariance, statistics, output variable, or Stata transaction.
6. Recapture and reseal the exact pinned reference; then run targeted tests,
   the full Rust/source lane, full licensed Stata, rollback/Break, loader and
   store-failure gates, and the frozen hidden projection.
7. Advance this file only after a reachable source SHA has exact-green evidence.

Do not proceed to public activation as part of this checkpoint.

## Current blockers — frozen manifest

The accepted candidate executes 31 of 40 frozen semantic cases. The nine
remaining blockers, in order, are:

1. `intercept_and_slope` — option-A positive-DoF fixture, source correction,
   pinned recapture, and exact-green reseal.
2. `slope_only_multiple` — multiple absorbed slope-only terms.
3. `string_fe` — string-source ingestion.
4. `time_series_lag` — Stata time-series lag materialization.
5. `time_series_lead_difference_gaps` — lead/difference semantics across gaps.
6. `iweight_probe` — importance-weight semantics.
7. `dof_continuous` — continuous-slope DoF policy and evidence.
8. `saved_outputs` — absorbed contribution and named FE recovery.
9. `postestimation_test_summarize` — public postestimation.

Broader factor/time-series parser behavior, `xb`/`xbd`, public syntax,
performance, low-memory execution, packaging, and platform qualification remain
activation work even after these nine cases close.

## Next evidence to record

For the option-A source checkpoint, record:

- the full source SHA and exact `.ci/stata/results/<SHA>.json` receipt;
- targeted independent explicit-dummy slope results;
- the new pinned positive-DoF reference receipt and payload/file identities;
- exact sample, coefficient/status, residual, RSS, covariance, absorbed-DoF,
  residual-DoF, solver-certificate, and transaction comparisons;
- the fail-closed 24-row insufficient-observations result;
- an updated frozen candidate receipt and external identity manifest;
- full licensed-Stata fixture count, Rust/source outcome, plugin identity,
  loader/store-failure sweeps, rollback/Break results, and public activation
  remaining false.

The next implementation family after an accepted option-A checkpoint is
`slope_only_multiple`.
