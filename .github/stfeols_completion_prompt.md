# StFixest `stfeols` completion contract

You are the implementation agent for the private repository `johannes-schmieder/StFixest`.
Work only in the checked-out `main` worktree. Do not create branches. Do not edit or enable
`stfepois`, public PPML dispatch, or unrelated estimator surfaces.

## Governing objective

Complete the public `stfeols` OLS 1.0 estimator specified by `AGENTS.md`, the scientific
contracts, the frozen OLS manifest, and `STATUS.md`. Completion means the public command is
actually enabled and all required scientific, compatibility, failure-atomicity, exact-SHA,
licensed-Stata, frozen-projection, and performance/RSS gates pass. A hidden implementation,
a partial fixture set, or a plan is not completion.

## Non-negotiable rules

1. Treat current `main`, recent history, immutable receipts, `AGENTS.md`, and source as
   authoritative. `STATUS.md` may lag and must be updated only after evidence exists.
2. Preserve all registered estimator semantics, sample/drop precedence, coefficient status
   meanings, VCE/DoF rules, solver certificates, result atomicity, and fail-closed behavior.
   Do not loosen a tolerance, skip an assertion, replace an independent oracle with a
   self-comparison, or make a benchmark easier to pass.
3. Keep the implementation pure Stata/Mata/Rust as already architected. Do not introduce a
   plugin in another language, a live network dependency, or a runtime dependency on
   `reghdfe`.
4. The pinned `reghdfe` closure and sealed receipts are reference evidence only. Normal tests
   must remain offline and receipt-driven.
5. Use the licensed Stata/MP 18 installation on this runner for executable parity tests.
6. Make small coherent commits. Do not rewrite history. The enclosing supervisor performs
   qualification and pushes; you may commit locally, but do not create branches.
7. If a genuinely scientific/econometric/API choice is not determined by governing contracts
   or pinned evidence, write `SCIENTIFIC_QUESTION.md` with the exact alternatives and stop.
   Do not guess. Ordinary implementation and debugging decisions are yours.
8. Never claim completion based only on unit tests. Completion must be encoded in a strict
   machine-verifiable `ci/check_stfeols_completion.py` gate that independently checks the
   public command, full frozen projection, exact-green licensed-Stata/Rust receipt, release
   fail-closed boundaries for PPML, and the registered performance/RSS evidence.

## Current work sequence

Inspect the repository before acting and resume at the first incomplete gate. The expected
sequence is:

1. Close the absorbed-intercept/fweight interaction score issue without changing registered
   tolerances. Complete executable receipt-driven coverage of all accepted FE-interaction
   probe cases.
2. Reseal the frozen OLS projection with the newly executable interaction surface and activate
   the already staged final-sample categorical rebase only through its validation contract.
3. Implement and independently test the remaining frozen blockers: absorbed intercept+slope,
   multiple slope-only effects, continuous-slope DoF, numeric/string FE ingestion, Stata
   time-series materialization, iweight semantics, saved/recovered outputs and prediction,
   and public postestimation (`test`, `summarize`) behavior.
4. Complete public `stfeols` syntax, parser/display/result posting, rollback/Break behavior,
   weight/VCE/DoF combinations, fast/low-memory equivalence, and failure paths. Keep all
   public estimator actions disabled until the full activation transaction is qualified.
5. Execute and seal all 40 frozen semantic cases, independent numerical oracles, complete
   licensed-Stata reference parity, full Rust/C/source checks, actual-plugin loader and binary
   identity checks, rollback/Break tests, and receipt verification.
6. Run the registered frozen benchmarks with at least five warm repetitions per primary case.
   Require median suite speedup >=1.5x, no primary case slower than the pinned reference, and
   peak observed RSS no greater than the reference for every primary case. Diagnose and fix
   failures; do not waive them.
7. Enable `stfeols` only in the final activation commit after every preceding gate is green.
   Keep `stfepois`, `estimate-ppml`, and any unsupported public surface fail-closed.
8. Update `STATUS.md` to the exact accepted source/receipt and run the strict completion gate.

## Working behavior for this invocation

Do not return a roadmap. Inspect files and implement the largest coherent next milestone that
can be fully tested in this invocation. Run focused tests while developing, then the relevant
repository and licensed-Stata gates. Repair failures before stopping. Leave the worktree clean
or with coherent tested changes ready for the supervisor. In your final message, state exact
files changed, tests run, remaining machine-verifiable blockers, and whether
`ci/check_stfeols_completion.py` passes. Never say `stfeols` is complete unless that strict gate
passes.
