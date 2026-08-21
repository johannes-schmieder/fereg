# StFixest implementation status

**Last updated:** 2026-08-21
**Branch / baseline:** `main` through `01c4b560`
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
| Single-status-file rule | Complete | Commit `0ce3ad9` consolidated live project state into this file and repository checks reject duplicate status documents. |
| Independent OLS oracle | Complete | Commits `6520b50` and `aa3b67a` add an independent explicit-dummy WLS/FWL, rank, VCE, and failure oracle; a separate scientific review found and verified fixes for three P1 defects. |
| OLS reference-contract harness | Implemented; capture pending | Commit `01c4b560` freezes the semantic/benchmark manifest, isolated preflight, capture renderer, and sealed receipt verifier. It deliberately refuses to run without an exact `ftools` provenance pin. |
| GitHub Actions | Intentionally absent | Target-native local validation and recorded receipts are required. |

## Active checkpoint — make `stfeols` executable

1. Resolve and verify the exact transitive `ftools` reference, then run and seal
   the pinned `reghdfe` semantic captures.
2. Use the frozen captures to settle weight, DoF, VCE, fit-statistic, and
   postestimation contracts.
3. Complete the data-plane, FE, OLS/VCE/DoF, and Stata one-call
   integration gaps before enabling `estimate-ols`.
4. Enable `stfeols` only after the scoped parity and atomic-result tests pass.
5. Optimize only after scientific correctness is certified.

## Current blockers

- `stfeols` is deliberately disabled and returns `r(498)`; no successful
  Stata estimator invocation, result posting, or reference parity evidence
  exists yet.
- The `reghdfe` capture harness is ready but cannot launch: the required
  transitive `ftools` source/installed-file provenance is intentionally
  unresolved and fail-closed. No pinned reference capture, benchmark timing,
  peak-RSS receipt, or constrained-memory receipt has been recorded.
- Existing FE/OLS/PPML Rust source has unit coverage but has not been certified
  as a Stata-facing estimator. Independent audit and integration work are in
  progress.
- No real Linux/Windows validation or native Intel-host macOS validation exists.

## Next evidence to record

- Exact `ftools` and `reghdfe` installation/capture receipt, including
  executable, source and installed-file hashes, commands, seed, `e()` output,
  samples, timing, and RSS.
- Executable request-to-`PreparedOlsData` design and adversarial test suite,
  with no public estimator enablement.

## Historical qualification

Earlier documentation described a loader-selection defect and a runner that
accepted Stata's zero process status without a fixture terminal sentinel. The
merged repair selects macOS correctly for Stata's `Unix` report and requires
`END_FIXTURE|...|rc=0`; only the corrected receipt is accepted as evidence.
