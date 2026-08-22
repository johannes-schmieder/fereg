# StFixest implementation status

**Last updated:** 2026-08-21
**Branch / baseline:** `main` through `8761050` plus the sealed reference receipt below
**Active milestone:** working and scientifically tested `stfeols` 1.0 on native Apple Silicon macOS
**Release state:** development only — not a production or cross-platform release

`STATUS.md` is the repository's only mutable progress record. Immutable command
receipts live under `build_evidence/` and `benchmarks/reports/captured/`.
Contracts and reference documentation record requirements, not live progress.

## Milestone exit criteria

The milestone is complete only when `stfeols` implements the full OLS 1.0
surface specified in `AGENTS.md`, passes independent numerical and
Stata-reference parity tests, and meets all of the following on the frozen
benchmark manifest against the pinned `reghdfe` reference:

- At least five warm repetitions per primary case; median suite speedup of at
  least 1.5x, with no primary case slower than the reference.
- Peak observed RSS no greater than the reference for every primary case.
- Fast and low-memory modes have the same sample/statuses and meet the fixed
  numerical tolerances in the scientific contract.
- Failure paths never post a partial estimate or silently weaken a requested
  numerical, sample, VCE, DoF, or convergence rule.

PPML, cross-platform release validation, signing, packaging, and publication
are explicitly deferred. `stfepois` must remain fail-closed until its own
scientific gates are met.

## Completed, evidence-backed baseline

| Gate | State | Evidence |
|---|---|---|
| Native/plugin foundation | Complete | Universal macOS plugin has `arm64` and `x86_64` slices; bridge, protocol rejection, diagnostics, and panic containment passed locally. |
| macOS loader and safety smoke | Complete | Native arm64 Stata 18 and Rosetta loader/reload fixtures passed; the Rosetta run is not native Intel certification. Sanitized receipts are under `build_evidence/local/`. |
| Rust source baseline | Complete | Formatting, warnings-as-errors Clippy, full workspace tests, and FFI tests pass through `8761050`. Re-run required after later estimator changes. |
| Model wire 2.1 | Complete | Versioned ordinary and absorbed-term model specification is merged. Wire 2.0 remains readable only with empty absorbed tables and registries. |
| Numeric sample preparation | Complete | Rust-native numeric loading/preparation is merged. |
| Developer OLS request-to-prepared-data boundary | Implemented through plain continuous, numeric factor-main-effect, numeric all-categorical-interaction, and unnamed single-source intercept-only numeric FE inputs with one optional analytic, frequency, or probability weight; latest slice awaits independent review | Commit `93193b7` added the guarded continuous, unweighted, no-FE `prepare-ols-test` path; `0fa48ca` added analytic weights; `c726c7a` added a mutually exclusive frequency-weight binding; `2b5b57c` tightened its missing/nonfinite boundary after independent review; `8d17c7c` added conservative probability-weight preparation; `c6e1888` added numeric ordinary-factor main-effect preparation; `6b9e3f6` added arbitrary-arity numeric all-categorical interaction recipes; and `8761050` added model-wire 2.1 plus bounded numeric FE-source preparation. Binding role 3 remains generic numeric storage, so ordinary and repeated FE recipes share one SPI read, compact column, and deterministic encoded-factor table. Missing ordinary/shared sources remain reason 3; FE-only missing sources are reason 5 after weight reason 4. FE-only numeric levels may be negative or fractional. Unsupported string FEs, FE interactions, slopes, and saved-FE requests fail before observation reads or diagnostic writes. The slice creates only small prepared FE recipes; it does not create `FeDesign`, delete singletons, solve, or enable an estimator. The 115-test Python suite, portable/C checks, warnings-as-errors Clippy, complete Rust/FFI suite, universal macOS build/audit/actual-plugin loader, and all 11 licensed Stata fixtures pass. Public estimators remain disabled. |
| Single-status-file rule | Complete | Commit `0ce3ad9` consolidated live project state into this file and repository checks reject duplicate status documents. |
| Independent OLS oracle | Complete | Commits `6520b50` and `aa3b67a` add an independent explicit-dummy WLS/FWL, rank, VCE, and failure oracle; a separate scientific review found and verified fixes for three P1 defects. |
| OLS reference-contract harness | Complete; baseline captured | Commits `01c4b560`, `178ca247`, `13cec3d`, `30d2841`, `10e96b6`, `db4bf0b`, and `3da43a4` provide an independently reviewed isolated reference closure, full semantic manifest, scientific-first performance gates, sealed evidence, estimator-only RSS measurement, and regression-tested semantic/private-mlib/benchmark-identity capture corrections. The sealed ARM64 macOS/Stata 18 reference receipt is `benchmarks/reports/captured/darwin-arm64/stata18/ols/20260821T030000Z-a5181f9` (`sha256=6e861febcedfc8cc76570ee627c2010d4738750c732c600bc2cf0f29e88e8002`): 40 semantic plus 10 comparator benchmark cases passed receipt validation. |
| GitHub Actions | Intentionally absent | Target-native local validation and recorded receipts are required. |

## Active checkpoint — make `stfeols` executable

1. Use the sealed pinned `reghdfe` captures to settle weight, DoF, VCE,
   fit-statistic, and
   postestimation contracts.
2. Expand the audited request-to-prepared-data boundary
   to the remaining source, FE-design, singleton, and cluster roles without weakening its
   one-call, thread-confinement, validation-before-read, or deferred-write
   guarantees.
3. Complete FE, OLS/VCE/DoF, result-posting, and Stata one-call integration
   before enabling `estimate-ols`.
4. Enable `stfeols` only after the scoped parity and atomic-result tests pass.
5. Optimize only after scientific correctness is certified.

## Current blockers

- `stfeols` is deliberately disabled and returns `r(498)`; no successful
  Stata estimator invocation, result posting, or reference parity evidence
  exists yet.
- The developer preparation action covers plain continuous, numeric factor
  main-effect, numeric all-categorical-interaction, and simple numeric FE-source
  inputs with at most one supported weight. Mixed interactions, string
  factorization, general FE construction, FE interactions/slopes/saved outputs,
  cluster sources, singleton deletion, estimator execution, and result posting
  are not connected to this request boundary yet.
- No candidate `stfeols` parity, benchmark timing/RSS comparison, or
  constrained-memory receipt exists yet. The newly sealed receipt is
  reference-only and correctly reports the performance gate as blocked because
  `stfeols` remains disabled.
- Existing FE/OLS/PPML Rust source has unit coverage but has not been certified
  as a Stata-facing estimator. Independent audit and integration work are in
  progress.
- No real Linux/Windows validation or native Intel-host macOS validation exists.

## Next evidence to record

- Independent review of commit `8761050`, especially model-wire 2.1
  cardinalities, reason-3/reason-4/reason-5 precedence, shared-source
  deduplication, and validation-before-read/write behavior. Only after that
  review should a separately gated `FeDesign`/singleton or cluster-preparation
  checkpoint begin. No review may enable the public estimator or bypass the FE,
  singleton, VCE, DoF, posting, and parity gates.

## Historical qualification

Earlier documentation described a loader-selection defect and a runner that
accepted Stata's zero process status without a fixture terminal sentinel. The
merged repair selects macOS correctly for Stata's `Unix` report and requires
`END_FIXTURE|...|rc=0`; only the corrected receipt is accepted as evidence.
