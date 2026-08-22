# StFixest implementation status

**Last updated:** 2026-08-22
**Branch / baseline:** `main` implementation and independent review through `adbc345` plus the sealed reference receipt below
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
| Rust source baseline | Complete | Formatting, warnings-as-errors Clippy, repository/portable checks, 122 Python tests, 170 core tests, the full workspace, and FFI tests pass through `adbc345`. The universal macOS x86_64+arm64 build, binary audit, and actual-plugin loader also pass. Re-run required after later estimator changes. |
| Model wire 2.1 | Complete | Versioned ordinary and absorbed-term model specification is merged. Wire 2.0 remains readable only with empty absorbed tables and registries. |
| Numeric sample preparation | Complete | Rust-native numeric loading/preparation is merged. |
| Developer OLS request-to-prepared-data boundary | Implemented and reviewed for the current continuous/numeric-factor, intercept-only numeric-FE, and unit/a/f/p-weight preparation surface; public estimators remain disabled | The guarded action validates before reads, constructs the compact sample/singleton fixed point and deduplicated borrowed FE topology, then optionally builds exact prepared Y/X and runs certified MAP before any diagnostic write. Empty explicit FE topology now invokes the pinned virtual one-level structural constant: the existing RHS is globally weighted-demeaned in one certified iteration with six fallible O(K) workspaces and no N-row constant codes or membership. The actual plugin reports `map_iterations=1` and `map_stop=certified`; Rust and C loader tests lock that contract. Failure remains atomic and the residualized batch is discarded. Unsupported strings, mixed/continuous interactions, FE interactions/slopes/saved outputs, clusters, numerical result posting, and estimator actions remain outside this boundary. Full repository tests and the universal x86_64+arm64 build/audit/loader pass through `adbc345`. |
| Core-only prepared OLS/WLS coefficient fit and covariance-coordinate bread | Implemented, certified, and independently reviewed for the current prepared intercept-only-FE surface; not plugin-enabled | Strict MAP plus deterministic scaled pivoted QR returns canonical statuses, compact residuals, and reference-scale inverse cross-product bread. Empty explicit FE topology uses the same virtual structural constant for default and `noconstant`; `_cons` is never a finite QR coordinate and is appended only when requested as `NormalizedAbsorbedIntercept = mean_w(y)-mean_w(X)b`. That reported coordinate adds no additional rank, while the virtual effect contributes `df_a=1`. Literal constants are FE-collinear and full factor-indicator spans retain L-1 finite directions. The reported bread uses `side=-B mean_w(X)` and `corner=1/N_ref+mean_w(X)'B mean_w(X)`. MAP, rank, score, full inverse, symmetry, formula, normalization, allocation, mutation, explicit-dummy, paired reporting, and sealed Stata tests pass. No variance scale/meat, FE recovery, SPI result write, or public estimator action was added. Final review approves the repaired series through `adbc345`. |
| Core-only prepared absorbed DoF | Implemented and independently reviewed for the current single-source intercept-only prepared FE surface; consumed by the core iid composite but not the plugin | Explicit FE rows preserve term/source/label identity and implement pinned `None`, `FirstPair`, and default `Pairwise` redundancy with exactness metadata and fallible level-scale union-find. A nonempty no-absorb model instead prepends a distinctly tagged `ImplicitNoAbsorb` row labeled `_cons`: one category, zero redundant, one exact coefficient, `nested=false`, and `df_a=1` under both default and `noconstant`, without invented `AbsorbTermId`/`SourceId` or N-row storage. Branch-specific exact-label/size assertions plus fail-at-every and undersize-at-every tests cover its row and `_cons` string reservations. Repeated-source, invariance, corruption, independent graph-search, and sealed `df_a=8/19/21` tests pass. The report alone does not compute finite rank, effective N, residual DoF, nesting, VCE, or fit statistics; no plugin/SPI/public estimator path changed. Final review approves the repaired series through `adbc345`. |
| Core-only prepared unadjusted OLS/WLS inference | Implemented, fully validated, and independently reviewed for the current prepared intercept-only-FE surface through `adbc345`; not plugin-enabled | The same-call `CertifiedPreparedOlsUnadjusted` composition now implements the pinned virtual one-level structural constant for empty explicit FE topology under both default and `noconstant`. Weighted global demeaning uses the existing RHS and six fallible O(K) buffers without N-row codes; `_cons` is a report-only normalized coordinate, and a tagged `ImplicitNoAbsorb` DoF row contributes `df_a=1` without invented wire/source IDs. Literal constants are FE-collinear and full `ibn.` spans have L-1 finite rank. Reference-scaled RSS/TSS/within TSS, both R-squared families, RMSE, Gaussian likelihoods, iid `RSS/df_r * bread`, and Wald `b' inv(V_slopes)b/df_m` are published only after independent primitive, DoF, covariance/SE, solve, and formula certificates pass; mutation coverage spans every statistic family. Unweighted/analytic effective N is physical N, frequency effective N is exact multiplicity, and positive pweight iid inference fails closed. Paired default/`noconstant`, explicit-dummy, random/permutation, extreme scaling, frequency expansion, weak-signal Wald, every-allocation/capacity, corruption, and sealed Stata 18 tests pass. The universal macOS build/audit/actual-loader contract also passes. No robust/cluster meat, nesting, plugin/SPI result write, result posting, or public estimator action was added. |
| Single-status-file rule | Complete | Commit `0ce3ad9` consolidated live project state into this file and repository checks reject duplicate status documents. |
| Independent OLS oracle | Complete | Commits `6520b50` and `aa3b67a` add an independent explicit-dummy WLS/FWL, rank, VCE, and failure oracle; a separate scientific review found and verified fixes for three P1 defects. |
| OLS reference-contract harness | Complete; baseline captured | Commits `01c4b560`, `178ca247`, `13cec3d`, `30d2841`, `10e96b6`, `db4bf0b`, and `3da43a4` provide an independently reviewed isolated reference closure, full semantic manifest, scientific-first performance gates, sealed evidence, estimator-only RSS measurement, and regression-tested semantic/private-mlib/benchmark-identity capture corrections. The sealed ARM64 macOS/Stata 18 reference receipt is `benchmarks/reports/captured/darwin-arm64/stata18/ols/20260821T030000Z-a5181f9` (`sha256=6e861febcedfc8cc76570ee627c2010d4738750c732c600bc2cf0f29e88e8002`): 40 semantic plus 10 comparator benchmark cases passed receipt validation. |
| GitHub Actions | Intentionally absent | Target-native local validation and recorded receipts are required. |

## Active checkpoint — make `stfeols` executable

1. Use the sealed pinned `reghdfe` captures to implement prepared robust/cluster score
   meat, finite-sample corrections, cluster intersections/nesting, and
   postestimation contracts. Core-only unadjusted covariance, residual DoF, and
   fit statistics are now composed, certified, and independently reviewed.
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
  the strict borrowed MAP solver before the first diagnostic store. With empty
  explicit topology it performs the virtual structural-constant projection in
  one certified iteration and the actual loader requires that metadata. It discards
  the residualized batch and writes only the existing three preparation
  diagnostics plus small display metadata; that plugin path performs no
  coefficient/rank solve, VCE, DoF, fit-statistic calculation, result-matrix
  write, `e()` mutation, or public estimator execution. The separate core-only
  prepared fit now certifies finite coefficients, the reference-normalized
  reported `_cons`, compact residuals, and covariance-coordinate inverse
  cross-product bread. The `_cons` normalization is available from weighted
  Y/X means and finite coefficients; it does not require recovery of individual
  FE coefficients. Its reported coordinate adds no additional rank; the
  virtual no-absorb structural effect is a separate tagged DoF row contributing
  `df_a=1` under default and `noconstant`. The
  separate core-only absorbed-DoF report computes supported structural FE
  redundancy without consulting weights or finite regressors. The new
  same-call core-only iid composite internally combines both artifacts with
  reference weights/effective N, residual DoF, fit statistics, and conventional
  homoskedastic covariance; probability-weight iid inference is rejected.
  All three remain unreachable from the plugin. Robust/cluster score meat,
  finite-sample corrections, nesting, and public result metadata remain absent. Mixed
  interactions, string factorization, FE
  interactions, slopes, FE recovery/saved outputs, and cluster sources are
  also outside this request boundary.
- No candidate `stfeols` parity, benchmark timing/RSS comparison, or
  constrained-memory receipt exists yet. The newly sealed receipt is
  reference-only and correctly reports the performance gate as blocked because
  `stfeols` remains disabled.
- Existing FE/OLS/PPML Rust source has unit coverage but has not been certified
  as a Stata-facing estimator. The repaired prepared fit, implicit structural
  constant, absorbed DoF, and same-call iid inference series is independently
  reviewed through `adbc345`; robust/cluster VCE, plugin estimator integration,
  result posting, and estimator-level sealed parity remain gated.
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
  certificate scale, fatal nonconvergence, and explicit-dummy parity. The
  prepared-only empty-topology structural wrapper and its exact O(K)
  allocation sweep are subsequently reviewed through `adbc345`; the generic
  empty-design entry point retains its original identity contract.
- The historical prepared-RHS/plugin review through `1fd8e7b` confirms
  exact Y/X ordering and omission semantics, recipe/source validation, checked
  and fallible allocation, borrowed weight/design identity, zero-write
  interruption/solver/certificate failure atomicity, and
  that no numerical result or public estimator path was enabled.
  Its old empty-design identity behavior is superseded for prepared OLS by
  `6c52e9e`: no-absorb mapcheck now reports one certified virtual-constant
  iteration, as locked by the actual-plugin loader repair `adbc345` and the
  final independent review.
- The core-only prepared coefficient/bread checkpoint is independently reviewed
  through the structural-constant/inference repair `6c52e9e` and loader contract
  `adbc345`. It uses no legacy fit/rank/VCE/DoF or owned-FE path, runs
  scaled deterministic pivoted Householder QR after strict MAP, reconstructs
  and re-absorbs residuals, recomputes all generated-column scores, and
  preserves exact canonical slots including reordered QR ties. It atomically
  keeps every structural constant out of finite QR and changes a requested
  report coordinate to `NormalizedAbsorbedIntercept`,
  evaluates the pinned weighted-mean formula without individual FE recovery,
  and returns canonical reference-scale inverse cross-product bread only after
  independent finite and full-reported left/right inverse, symmetry,
  positive-diagonal, formula, normalization, rank, MAP, and score certificates
  pass. Unit/a/p/f scales, literal frequency expansion, extreme common scaling,
  rank and DoF noninteraction, allocation/capacity/overflow, no-N-copy bounds,
  symmetric extension corruption, explicit-dummy/random/permutation cases, and
  sealed Stata 18 anchors pass. It exposes no plugin result or VCE. The new iid
  composite is its only eligible variance-scale consumer; robust/cluster meat
  and complete estimator parity remain gated before integration.
- The core-only prepared absorbed-DoF checkpoint is reviewed through
  the synthetic-row allocation repair `e53e9ea` and final loader contract
  `adbc345`: it uses final `PreparedOlsFeTopology` order, identities, levels,
  and memberships plus borrowed single-owned categorical codes; it does not
  use the legacy DoF/VCE/fit path, any RHS, model column, outcome, or weight.
  Its fallible iterative pairwise union-find performs no observation-scale
  copy, and checked totals certify the sum of per-effect coefficients. The
  sealed one-FE, `dof(none)`, and `dof(firstpair)` anchors (8, 19, and 21),
  independent randomized graph-search comparisons, repeated-source/order,
  policy/exactness, invariance, corruption, overflow, and allocation/capacity
  tests pass. Empty explicit topology adds the tagged one-category
  `ImplicitNoAbsorb` row and `df_a=1`, with complete row/label reservation
  failure coverage. Independent rereview found no remaining P0/P1 issue. Its supported
  rank is now composed with finite rank/intercept and effective frequency N by
  the iid checkpoint; nesting, robust/cluster VCE, atomic posting, and
  estimator-level parity remain later gates.
- The core-only prepared iid checkpoint and structural-constant repair are
  independently reviewed through `adbc345`. It makes fit plus absorbed-DoF
  artifacts in one call, rejects pweight iid inference, retains the initial
  certified MAP outcome norm, and computes F from the certified slope Wald
  quadratic. For reported dimension P and slope dimension q, inference reserves
  P² covariance plus P standard errors, q slope positions, q² Cholesky, and q
  solve storage: P²+q² square and P+2q vector entries, never N-scale storage.
  Independent certificate mutations, paired default/`noconstant`, explicit-
  dummy, randomized, permutation, frequency-expansion, extreme-scale,
  weak-signal Wald, edge/failure-injection, and sealed Stata tests pass. It does
  not call legacy fit/VCE/DoF code, recover FEs, write SPI results, or enable
  either public estimator.
- Continue the independently gated preparation roadmap for mixed ordinary
  interactions, string/interaction/slope/saved fixed effects, and clusters.
  None of these checkpoints may enable the public estimator or bypass the FE,
  solver-certificate, VCE, DoF, atomic-posting, and sealed parity gates.

## Historical qualification

Earlier documentation described a loader-selection defect and a runner that
accepted Stata's zero process status without a fixture terminal sentinel. The
merged repair selects macOS correctly for Stata's `Unix` report and requires
`END_FIXTURE|...|rc=0`; only the corrected receipt is accepted as evidence.
