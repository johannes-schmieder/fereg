# StFixest implementation status

**Last updated:** 2026-08-21
**Branch / baseline:** `main` implementation through `1fd8e7b` plus the sealed reference receipt below
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
| Rust source baseline | Complete | Formatting, warnings-as-errors Clippy, full workspace tests, and FFI tests pass through `1fd8e7b`. Re-run required after later estimator changes. |
| Model wire 2.1 | Complete | Versioned ordinary and absorbed-term model specification is merged. Wire 2.0 remains readable only with empty absorbed tables and registries. |
| Numeric sample preparation | Complete | Rust-native numeric loading/preparation is merged. |
| Developer OLS request-to-prepared-data boundary | Implemented through plain continuous, numeric factor-main-effect, numeric all-categorical-interaction, and unnamed single-source intercept-only numeric FE inputs with one optional analytic, frequency, or probability weight; singleton, prepared-topology, borrowed-adapter, strict borrowed MAP, and prepared Y/X plus opt-in plugin integration are independently reviewed through `1fd8e7b` | Commit `93193b7` added the guarded continuous, unweighted, no-FE `prepare-ols-test` path; `0fa48ca` added analytic weights; `c726c7c` added a mutually exclusive frequency-weight binding; `2b5b57c` tightened its missing/nonfinite boundary after independent review; `8d17c7c` added conservative probability-weight preparation; `c6e1888` added numeric ordinary-factor main-effect preparation; `6b9e3f6` added arbitrary-arity numeric all-categorical interaction recipes; `8761050` added model-wire 2.1 plus bounded numeric FE-source preparation; `f59ac84` added the supported numeric-FE singleton fixed point; `b1a8b85` repaired its allocation boundary after review; `f211acb` added a structural prepared FE topology on the final compact codes; `2047c34` added a strictly intercept-only borrowed design view; `86ef394` repaired the independent-review finding that repeated wire terms re-ran observation-scale validation; `6b84e21` added a separate core-only strict borrowed-intercept MAP path; and `2616598` added the exact prepared Y/X bridge plus guarded plugin check. Binding role 3 remains generic numeric storage, so ordinary and repeated FE recipes share one SPI read and compact column. After reasons 1--5, the internal boundary iteratively deletes supported FE singletons, assigns reason 8 without overwriting earlier reasons, fallibly compacts every aligned source, and re-factorizes final categorical codes and ordinary recipes. The topology preserves absorbed-term order and labels, deduplicates repeated FE sources, owns one fallibly constructed compressed membership index per unique final source, validates exact membership and the requested singleton policy, and preserves zero-level sources on an empty final sample. The repaired adapter preserves wire order, borrows the final categorical code slices and existing membership indices, reuses both pointers for repeated sources, validates each unique source exactly once, and constructs each wire effect with constant-time identity, dimension, and pointer checks. It checks its term-slot total and performs no observation-scale code or level copy. Its O(U) source-certificate table and O(G) effect table are fallibly reserved and capacity-guarded with deterministic failure tests; instrumented scale coverage proves one N-row validation call for 128 terms sharing one source. The strict MAP path consumes a nonempty dense RHS, uses structural unit weights or borrowed supplied weights, fallibly reserves all six O(K) workspaces before mutation, allocates nothing during projection/certification, rejects nonfinite or nonrepresentable inputs, and returns only a passing recomputed FE certificate; max-iteration and failed-certificate paths are fatal. The new bridge revalidates the sample axis, retained-row map, weights, categorical encodings, coefficient/generated-column mapping, and every supported recipe/source identity. It checks dimensions and builds exact Y plus generated-X order—including absent syntax omissions, vanished-level zero columns, and the last-position intercept—in one fallibly reserved row-major allocation without intermediate observation-scale columns. The hidden `mapcheck` option defaults false; when selected, it borrows existing weights/codes/membership, runs before the first diagnostic store, discards residuals, and posts no numerical result. Empty input, injected allocation/capacity failure, corruption, interruption, nonconvergence, and certificate failure leave SPI diagnostics and display untouched; default empty preparation remains unchanged and nonempty no-FE input returns the certified identity. Independent explicit-dummy tests cover the integrated layout and MAP result for multiple RHSs and unweighted, analytic-, frequency-, and probability-weight objectives, plus repeated sources, disconnected components, random designs, permutations, and weight scaling. Unweighted, analytic-, and probability-weighted requests count physical rows; frequency-weighted requests use exact positive-integer multiplicities. `keepsingletons` is an exact no-delete path. Unsupported string FEs, FE interactions, slopes, and saved-FE requests fail before observation reads or diagnostic writes. The 119-test Python suite, portable/C checks, warnings-as-errors Clippy, complete Rust/FFI suite including 127 core, 27 plugin non-FFI, and 29 plugin FFI tests, audited universal macOS build and actual-plugin loader, and all 11 licensed Stata fixtures pass. Public estimators remain disabled. |
| Single-status-file rule | Complete | Commit `0ce3ad9` consolidated live project state into this file and repository checks reject duplicate status documents. |
| Independent OLS oracle | Complete | Commits `6520b50` and `aa3b67a` add an independent explicit-dummy WLS/FWL, rank, VCE, and failure oracle; a separate scientific review found and verified fixes for three P1 defects. |
| OLS reference-contract harness | Complete; baseline captured | Commits `01c4b560`, `178ca247`, `13cec3d`, `30d2841`, `10e96b6`, `db4bf0b`, and `3da43a4` provide an independently reviewed isolated reference closure, full semantic manifest, scientific-first performance gates, sealed evidence, estimator-only RSS measurement, and regression-tested semantic/private-mlib/benchmark-identity capture corrections. The sealed ARM64 macOS/Stata 18 reference receipt is `benchmarks/reports/captured/darwin-arm64/stata18/ols/20260821T030000Z-a5181f9` (`sha256=6e861febcedfc8cc76570ee627c2010d4738750c732c600bc2cf0f29e88e8002`): 40 semantic plus 10 comparator benchmark cases passed receipt validation. |
| GitHub Actions | Intentionally absent | Target-native local validation and recorded receipts are required. |

## Active checkpoint — make `stfeols` executable

1. Use the sealed pinned `reghdfe` captures to settle weight, DoF, VCE,
   fit-statistic, and
   postestimation contracts.
2. Expand the audited request-to-prepared-data boundary
   to the remaining source, solver-adapter, and cluster roles without weakening its
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
  inputs with at most one supported weight, including the bounded numeric-FE
  singleton fixed point and a deduplicated structural topology over final FE
  codes. A structural intercept-only adapter now borrows those final codes and
  membership indices without copying them. The hidden, default-off `mapcheck`
  path now constructs exact prepared Y/X in one fallible allocation and runs
  the strict borrowed MAP solver before the first diagnostic store. It discards
  the residualized batch and writes only the existing three preparation
  diagnostics plus small display metadata: there is still no coefficient or
  rank solve, VCE, DoF, fit-statistic calculation, result matrix, `e()` state,
  or public estimator execution. Mixed interactions, string factorization, FE
  interactions, slopes, saved outputs, and cluster sources are also outside
  this request boundary.
- No candidate `stfeols` parity, benchmark timing/RSS comparison, or
  constrained-memory receipt exists yet. The newly sealed receipt is
  reference-only and correctly reports the performance gate as blocked because
  `stfeols` remains disabled.
- Existing FE/OLS/PPML Rust source has unit coverage but has not been certified
  as a Stata-facing estimator. The prepared-RHS/plugin checkpoint is reviewed
  through `1fd8e7b`; further integration work remains gated.
- No real Linux/Windows validation or native Intel-host macOS validation exists.

## Next evidence to record

- The borrowed-adapter rereview is complete through `1b1954f`: it confirms
  exact wire ordering, one structural validation per unique topology source,
  constant-time repeated-term construction and pointer reuse without a code or
  membership copy, fallible O(U + G) table allocation, checked term-slot
  totals, corruption rejection, and the zero-row/zero-level contract.
- The strict borrowed MAP review is complete through `ac6e7d5`: it confirms
  the borrowed-only API boundary, validation before mutation, all six fallible
  workspace allocations, absence of iteration/certificate allocation, checked
  dimensions and products, empty/nonfinite/weight-underflow rejection, final
  certificate scale, fatal nonconvergence, and explicit-dummy parity.
- The prepared-RHS/plugin review is complete through `1fd8e7b`: it confirms
  exact Y/X ordering and omission semantics, recipe/source validation, checked
  and fallible allocation, borrowed weight/design identity, empty/no-FE
  behavior, zero-write interruption/solver/certificate failure atomicity, and
  that no numerical result or public estimator path was enabled.
- Continue the independently gated preparation roadmap for mixed ordinary
  interactions, string/interaction/slope/saved fixed effects, and clusters.
  None of these checkpoints may enable the public estimator or bypass the FE,
  solver-certificate, VCE, DoF, atomic-posting, and sealed parity gates.

## Historical qualification

Earlier documentation described a loader-selection defect and a runner that
accepted Stata's zero process status without a fixture terminal sentinel. The
merged repair selects macOS correctly for Stata's `Unix` report and requires
`END_FIXTURE|...|rc=0`; only the corrected receipt is accepted as evidence.
