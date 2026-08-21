# StFixest implementation status

**Last updated:** 2026-08-21
**Branch / baseline:** `main` at `a47137f`
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
| Rust source baseline | Complete | Formatting, warnings-as-errors Clippy, debug/release unit tests, and FFI tests passed at the repaired baseline. Re-run required after estimator changes. |
| Model wire 2.0 | Complete | Versioned lossless Stata model specification is merged. |
| Numeric sample preparation | Complete | Rust-native numeric loading/preparation is merged. |
| Single-status-file rule | Complete | Commit `6783775` consolidated live project state into this file and repository checks reject duplicate status documents. |
| GitHub Actions | Intentionally absent | Target-native local validation and recorded receipts are required. |

## Active checkpoint — make `stfeols` executable

1. Freeze pinned `reghdfe` reference captures and the OLS benchmark manifest.
2. Audit and complete the data-plane, FE, OLS/VCE/DoF, and Stata one-call
   integration gaps before enabling `estimate-ols`.
3. Enable `stfeols` only after the scoped parity and atomic-result tests pass.
4. Optimize only after scientific correctness is certified.

## Current blockers

- `stfeols` is deliberately disabled and returns `r(498)`; no successful
  Stata estimator invocation, result posting, or reference parity evidence
  exists yet.
- No pinned `reghdfe` capture, benchmark timing, peak-RSS receipt, or
  constrained-memory receipt has been recorded.
- Existing FE/OLS/PPML Rust source has unit coverage but has not been certified
  as a Stata-facing estimator. Independent audit and integration work are in
  progress.
- No real Linux/Windows validation or native Intel-host macOS validation exists.

## Next evidence to record

- Exact pinned `reghdfe` installation/capture receipt, including executable,
  reference hash, commands, seed, `e()` output, samples, timing, and RSS.
- A dependency-ordered OLS integration audit identifying the first safely
  enableable implementation slice and its tests.

## Historical qualification

Earlier documentation described a loader-selection defect and a runner that
accepted Stata's zero process status without a fixture terminal sentinel. The
merged repair selects macOS correctly for Stata's `Unix` report and requires
`END_FIXTURE|...|rc=0`; only the corrected receipt is accepted as evidence.
