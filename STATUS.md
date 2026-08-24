# StFixest implementation status

**Last updated:** 2026-08-24
**Branch / baseline:** consolidated `main`; latest accepted exact-green source `0e2aa609f9cbb17f8a205261d80680e68a752cef`
**Active milestone:** expand and reseal the frozen hidden OLS projection after accepted final-sample categorical rebasing
**Release state:** development only — public estimators remain fail-closed; no production or cross-platform release claim

`STATUS.md` is the repository's only mutable progress record. Immutable command
receipts live under `build_evidence/` and `benchmarks/reports/captured/`.
Contracts and reference documentation record requirements, not live progress.

## Current consolidated main state

- Only `main` is used for StFixest development. The former categorical and OSX
  continuation lines were consolidated by `4bb5f1a38fca46b86cc7d52be9d8e013b173a0a4`;
  obsolete StFixest branches were then deleted.
- Exact-SHA full licensed qualification for source
  `0e2aa609f9cbb17f8a205261d80680e68a752cef` is green: overall,
  licensed-Stata, and Rust statuses are `success`, Stata returned zero, and
  all 25 full fixtures passed. The sanitized receipt is
  `.ci/stata/full-results/0e2aa609f9cbb17f8a205261d80680e68a752cef.json`.
  The preceding exact quick/full checkpoint at
  `dfd5ae0935f2b5a8925de14c316d699c5fff7e66` remains immutable provenance.
- Current main contains substantial hidden categorical-interaction support:
  collision-free compact tuple factorization; canonical partition reuse;
  final-sample topology; iterative singleton and frequency-weight behavior;
  certified MAP; prepared fit, absorbed DoF, iid, HC1, and clustered inference;
  hidden/native result transport; Stata-side hidden transport; independent
  explicit-dummy oracles; and a licensed-Stata categorical quick fixture.
- The exact pinned offline reference environment and licensed-Stata probe are
  source-bound and successful for source
  `0e2aa609f9cbb17f8a205261d80680e68a752cef`. Its sanitized 14-case reference
  receipt is
  `.ci/reference-probes/fe-interaction/0e2aa609f9cbb17f8a205261d80680e68a752cef.json`,
  sealed at payload SHA-256
  `fab875a998edc8f91bbeebfca8ca8ef92c09f43f6044fbbcef0b1e7da8fcf387`.
- All 12 receipt-bound hidden-estimator FE-interaction differential cases are
  executable and exact-green at source
  `dfd5ae0935f2b5a8925de14c316d699c5fff7e66`. They bind final samples,
  singleton and frequency-weight behavior, coefficient/status layouts, iid
  and clustered covariance, absorbed and residual DoF policies, interaction
  and cluster identity, nesting, score certificates, and observation
  residuals to the sealed pinned-`reghdfe` payload. Independent explicit-dummy
  Rust fit, iid, robust, clustered, and certificate oracles also pass. This
  closes the interaction differential only; it does **not** close
  `multi_fe_interactions` or establish full estimator parity before the frozen
  projection is resealed.
- Digest-bound full-history Git bundles for the pinned `reghdfe`, `ftools`,
  and `stata-require` commits are committed and were materialized offline by
  the successful source-bound probe.
- The unchanged frozen 40-case hidden OLS partition is resealed at source
  `d58310783150e81739ea87eee868fd6bc8334b33`: all 30 executable cases pass
  against the pinned comparator, the ten unsupported cases remain explicit
  blockers, and the identical staged plugin passed fake-SPI/actual-loader store
  failure sweeps plus licensed-Stata parity, rollback, and actual-Break gates.
  The canonical receipt is
  `benchmarks/reports/captured/darwin-arm64/stata18/ols_hidden_numeric/20260824T093209Z-d583107`.
- Final-sample categorical rebasing is active in the hidden OLS path at exact
  source `0e2aa609f9cbb17f8a205261d80680e68a752cef`. Native preparation rebuilds
  the ordinary categorical graph after singleton deletion; result wire 1.1
  separates candidate matrix capacity from the final logical graph; Stata
  posts only the final names and dimensions. The independent explicit-dummy
  oracle and exact-pinned default-base factor and interaction differentials
  pass. Explicit `ib...`/`ibn` and `fvset` base overrides remain deliberately
  fail-closed until the public parser can distinguish their intent. The sealed
  activation receipt is
  `.ci/staged-validation/final-sample-rebase/0e2aa609f9cbb17f8a205261d80680e68a752cef.json`.
- `stfeols`, `stfepois`, public `estimate-ols`, and public `estimate-ppml`
  remain deliberately disabled.

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

The rows below are retained as checkpoint-specific provenance. They describe
the cited historical commits and are not the live current-main inventory; the
consolidated state above and active checkpoint below control current work.

| Gate | State | Evidence |
|---|---|---|
| Native/plugin foundation | Complete | Universal macOS plugin has `arm64` and `x86_64` slices; bridge, protocol rejection, diagnostics, and panic containment passed locally. |
| macOS loader and safety smoke | Complete | Native arm64 Stata 18 and Rosetta loader/reload fixtures passed; the Rosetta run is not native Intel certification. Sanitized receipts are under `build_evidence/local/`. |
| Rust/source validation baseline | Complete through the active candidate | The full workspace passes through `d731b327`, including 185 core unit tests, 41 plugin unit tests, and all oracle/integration suites; 139 Python tests and all 13 licensed-Stata fixtures pass. Repository/diff checks and the active receipt verifier pass. The candidate-bound universal macOS plugin contains x86_64+arm64, embeds `d731b327`, passes binary audit and the actual-plugin loader, and is shared identically by every receipt supporting gate. |
| Model wire 2.1 | Complete | Versioned ordinary and absorbed-term model specification is merged. Wire 2.0 remains readable only with empty absorbed tables and registries. |
| Numeric sample preparation | Complete | Rust-native numeric loading/preparation is merged. |
| Developer OLS request-to-prepared-data boundary | Implemented and reviewed for the current continuous/numeric-factor, intercept-only numeric-FE, and unit/a/f/p-weight preparation surface; public estimators remain disabled | The guarded action validates before reads, constructs the compact sample/singleton fixed point and deduplicated borrowed FE topology, then optionally builds exact prepared Y/X and runs certified MAP before any diagnostic write. Empty explicit FE topology now invokes the pinned virtual one-level structural constant: the existing RHS is globally weighted-demeaned in one certified iteration with six fallible O(K) workspaces and no N-row constant codes or membership. The actual plugin reports `map_iterations=1` and `map_stop=certified`; Rust and C loader tests lock that contract. Failure remains atomic and the residualized batch is discarded. Unsupported strings, mixed/continuous interactions, FE interactions/slopes/saved outputs, clusters, numerical result posting, and estimator actions remain outside this boundary. Full repository tests and the universal x86_64+arm64 build/audit/loader pass through `adbc345`. |
| Core-only prepared OLS/WLS coefficient fit and covariance-coordinate bread | Implemented, certified, and independently reviewed for the current prepared intercept-only-FE surface; not plugin-enabled | Strict MAP plus deterministic scaled pivoted QR returns canonical statuses, compact residuals, and reference-scale inverse cross-product bread. Empty explicit FE topology uses the same virtual structural constant for default and `noconstant`; `_cons` is never a finite QR coordinate and is appended only when requested as `NormalizedAbsorbedIntercept = mean_w(y)-mean_w(X)b`. That reported coordinate adds no additional rank, while the virtual effect contributes `df_a=1`. Literal constants are FE-collinear and full factor-indicator spans retain L-1 finite directions. The reported bread uses `side=-B mean_w(X)` and `corner=1/N_ref+mean_w(X)'B mean_w(X)`. MAP, rank, score, full inverse, symmetry, formula, normalization, allocation, mutation, explicit-dummy, paired reporting, and sealed Stata tests pass. No variance scale/meat, FE recovery, SPI result write, or public estimator action was added. Final review approves the repaired series through `adbc345`. |
| Core-only prepared absorbed DoF | Implemented and independently reviewed for the current single-source intercept-only prepared FE surface; consumed by the core iid, robust, and clustered composites but not the plugin | Explicit FE rows preserve term/source/label identity and implement pinned `None`, `FirstPair`, and default `Pairwise` redundancy with exactness metadata and fallible level-scale union-find. A nonempty no-absorb model instead prepends a distinctly tagged `ImplicitNoAbsorb` row labeled `_cons`: one category, zero redundant, one exact coefficient, `nested=false`, and `df_a=1` under both default and `noconstant`, without invented `AbsorbTermId`/`SourceId` or N-row storage. The clustered view tests explicit effects against every declared dimension before mobility redundancy, retains nested category counts separately, and never nests the virtual constant. Branch-specific exact-label/size assertions plus fail-at-every and undersize-at-every tests cover its rows and labels. Repeated-source, invariance, corruption, independent graph-search, sealed `df_a=8/19/21`, and clustered nesting/interaction tests pass. The base report alone does not compute finite rank, effective N, residual DoF, VCE, or fit statistics; the clustered composition adds and certifies its distinct residual-DoF views. No plugin/SPI/public estimator path changed. Final review approves the repaired DoF series through `adbc345`, its robust composition through `32008af`, and its clustered composition through `89132055`. |
| Core-only prepared unadjusted OLS/WLS inference | Implemented, fully validated, and independently reviewed for the current prepared intercept-only-FE surface through `adbc345`; not plugin-enabled | The same-call `CertifiedPreparedOlsUnadjusted` composition now implements the pinned virtual one-level structural constant for empty explicit FE topology under both default and `noconstant`. Weighted global demeaning uses the existing RHS and six fallible O(K) buffers without N-row codes; `_cons` is a report-only normalized coordinate, and a tagged `ImplicitNoAbsorb` DoF row contributes `df_a=1` without invented wire/source IDs. Literal constants are FE-collinear and full `ibn.` spans have L-1 finite rank. Reference-scaled RSS/TSS/within TSS, both R-squared families, RMSE, Gaussian likelihoods, iid `RSS/df_r * bread`, and Wald `b' inv(V_slopes)b/df_m` are published only after independent primitive, DoF, covariance/SE, solve, and formula certificates pass; mutation coverage spans every statistic family. Unweighted/analytic effective N is physical N, frequency effective N is exact multiplicity, and positive pweight iid inference fails closed. Paired default/`noconstant`, explicit-dummy, random/permutation, extreme scaling, frequency expansion, weak-signal Wald, every-allocation/capacity, corruption, and sealed Stata 18 tests pass. The universal macOS build/audit/actual-loader contract also passes. No robust/cluster meat, nesting, plugin/SPI result write, result posting, or public estimator action was added. |
| Core-only prepared HC1 robust OLS/WLS inference | Implemented, fully validated, and independently reviewed for the current prepared intercept-only-FE surface through `32008af`; not plugin-enabled | `CertifiedPreparedOlsRobust` composes fit, absorbed DoF, residual DoF, statistics, and HC1 covariance in one call. It rebuilds an ephemeral kept-coordinate `N x q` score design through strict MAP, uses the normalized absorbed-intercept coordinate without materializing a constant, and destroys the score design before return. `c=N_effective/df_r`; meat multipliers are one for unit weights, raw multiplicity for fweights, and squared normalized objective weights for aweights/pweights. The production influence Gram and independent meat-then-sandwich certificate agree; score inverse/intercept, HC1, symmetry/SE, PSD, robust-Wald, and statistic certificates all gate publication. Singular PSD covariance is allowed, singular slope covariance makes F undefined, and material indefiniteness fails closed. All allocations are fallible, including the O(1) reported-coordinate map. Explicit-dummy/random/permutation/frequency-expansion oracles cover default/`noconstant`, virtual constant, disconnected two-way FE, repeated identical FE spans, all four weight rules, and full covariance. Sealed one-FE robust and pweight covariances pass. Review of `a7a01a4` plus repair `32008af` found no P0/P1 issue. No legacy VCE, cluster logic, plugin/SPI/public posting, or estimator action was used. |
| Core-only prepared one-/multiway clustered OLS/WLS inference | Implemented, fully validated, and independently reviewed for the current prepared intercept-only numeric-FE surface through `b226d62` plus repair `89132055`; not plugin-enabled | Sample-bound numeric cluster preparation supports 1--10 declared dimensions, treats a declared source tuple as one interaction dimension, applies reason 6 before FE singleton deletion, never uses cluster counts to delete singletons, and factorizes final-sample interactions collision-free after pruning. `CertifiedPreparedOlsClustered` composes fit, cluster-aware absorbed DoF, reported/fit/correction DoF, statistics, score design, inclusion--exclusion meat, one common reghdfe correction, CGM repair, slope-only Wald, and independent certificates in one core call. Group scores are `sum(omega_i e_i z_i)` for unit/a/f/p reference weights; every required subset must have at least two groups. One-way covariance is strictly PSD with no repair. Multiway covariance follows the pinned two-stage transform exactly: when the raw full matrix is indefinite, clip the full eigensystem, independently clip the original raw slope block when a reported `_cons` and slopes exist, restore that slope block, and never reclip the final full matrix. Only that exact restored state may retain a diagnostic final negative eigenvalue; the pre-restore full matrix and restored slope block remain strictly certified PSD. Internal explicit-dummy/raw/transform/certificate comparisons are at most `2e-10`. Separate frozen DEFAULT-`reghdfe` receipt comparisons, after exact structural assertions, use absolute behavioral caps only of `1e-7` (one-way), `2e-6` (two-way), `4e-6` (three-way), and `1e-7` (one declared interaction), documenting the upstream MAP stopping artifact rather than weakening scientific tolerances. All allocations are fallible; subset aggregation holds one ordering and one P-vector at a time and never a G x P table. Mutation, undersize, allocation, nesting, weight, virtual-constant, disconnected/repeated-FE, default/`noconstant`, rank-zero, intercept-only, paired-cluster, and sealed receipt tests pass. No legacy VCE/DoF, plugin/SPI/public posting, or estimator action was used. |
| Hidden atomic prepared-OLS transport | Implemented and independently approved through final-sample rebasing source `0e2aa60`; developer-only and not a public estimator | Guarded action `estimate-ols-test` requires exact action protocol 1.1, result wire 1.1, loaded-plugin version, and `confirm=developer-estimation-test` before any observation read. It performs one native source-loading phase and one same-call certified iid, HC1, or clustered composition, then fallibly adapts the complete result into private shadow coefficient, covariance, status, fit, DoF, solver, FE, cluster, subset, certificate, receipt, sample, residual, and drop-reason destinations. Candidate coefficient capacity is distinct from the final logical K; Rust validates both axes and the entire N/K/K² logical result, including native omission markers, before the first store, then stores only the leading logical blocks. All 11 result-matrix names and dimensions are converted to borrowed C handles before commit. The receipt `COMPLETE` cell is the final operation. SPI cannot physically roll back earlier writes, so a failed commit may dirty only private shadow destinations; they have no logical meaning unless the final receipt is complete. Numeric/matrix store errors and their C reports are prepared before commit and allocate nothing after a store failure. Current candidate-bound sweeps fail each of 222 Fake stores and 219 actual-loader stores; every failure leaves `COMPLETE=0`. The universal macOS build/audit/loader passes. Public `estimate-ols`, `stfeols`, `estimate-ppml`, and `stfepois` remain `r(498)` and no public `e()` state is posted. |
| Hidden licensed-Stata OLS eclass transaction | Implemented and exact-green through final-sample rebasing source `0e2aa60`; developer-only and non-public | `_stfixest_estimate_ols_test` creates private matrix/variable shadows at candidate capacity, holds prior estimates with `_estimates hold ..., restore nullok varname()`, sends the complete guarded request as one canonical length-framed bundle, and makes exactly one substantive `estimate-ols-test` plugin call. Rust has already validated all N/K/K² logical results and omission markers; the helper validates the complete receipt, shrinks all coefficient-coordinate shadows to final logical K, resolves final-sample canonical names, and retains only constant-size table/transaction checks before posting. The K=0 branch posts no b/V. A whole-matrix Mata copy preserves the pinned two-/three-way CGM covariance, including its certified restored-state negative determinant, without Stata's nonsymmetry warning or numerical alteration. The helper posts the selected fit/DoF/diagnostic/e(sample)/weight/cluster mapping, then commits only in a `nobreak` residual rename plus `_estimates unhold ..., not` block. Deterministic failure stages, real scientific failures including unsupported explicit base policies, null/non-null prior estimates, and an actual PTY Control-C after the sole plugin call restore prior `e()` state and sample exactly and expose no requested residual. All 25 full licensed fixtures pass. Public commands/actions remain `r(498)`. |
| Hidden numeric OLS candidate projection | Implemented, candidate-bound, resealed at source `d58310783150e81739ea87eee868fd6bc8334b33`; developer-only and non-public | `_stfixest_ols_hidden_numeric_v1` projects exactly 30 executable cases from the frozen 40-case `ols_v1` semantic manifest through the hidden eclass helper. It captures result, sample, residual/error, certificate, store-failure, rollback, and actual-Break evidence while retaining ten explicit blockers. Every Stata capture and supporting gate loaded the identical isolated staged plugin (`sha256=f1e0cc0eb6f849e524cd8474e16de4e8b07b08bfe43d08b2b66f72464874605b`, embedded `d58310783150`); the external identity manifest binds the committed receipt path, payload/file hashes, git blob, and mode. Rust validates all N-, K-, and K²-scale outputs and native omission markers before stores; Stata performs only constant-size transaction and identity checks. The active receipt is `benchmarks/reports/captured/darwin-arm64/stata18/ols_hidden_numeric/20260824T093209Z-d583107`, payload seal `85680710585bba7f12ebae534295057348bc63e67919dad254b47fe7a01f76dd`, file SHA-256 `f61ed80ecbaf2bea021e9ed62cafd5be2820d6a1e834e0ab61954a96542ed20e`, git blob `89fab138f4dbcdba802e3d527f106417af9c44c9`, mode `100644`. Full debug/release Rust and independent-oracle tests, the licensed 30-case capture, universal binary audit/loader, failure-injection, rollback/Break, and warning audit pass. `public_activation=false`; performance is unmeasured and no speed/RSS claim is made. |
| Single-status-file rule | Complete | Commit `0ce3ad9` consolidated live project state into this file and repository checks reject duplicate status documents. |
| Independent OLS oracle | Complete | Commits `6520b50` and `aa3b67a` add an independent explicit-dummy WLS/FWL, rank, VCE, and failure oracle; a separate scientific review found and verified fixes for three P1 defects. |
| OLS reference-contract harness | Complete; baseline captured | Commits `01c4b560`, `178ca247`, `13cec3d`, `30d2841`, `10e96b6`, `db4bf0b`, and `3da43a4` provide an independently reviewed isolated reference closure, full semantic manifest, scientific-first performance gates, sealed evidence, estimator-only RSS measurement, and regression-tested semantic/private-mlib/benchmark-identity capture corrections. The sealed ARM64 macOS/Stata 18 reference receipt is `benchmarks/reports/captured/darwin-arm64/stata18/ols/20260821T030000Z-a5181f9` (`sha256=6e861febcedfc8cc76570ee627c2010d4738750c732c600bc2cf0f29e88e8002`): 40 semantic plus 10 comparator benchmark cases passed receipt validation. |
| GitHub Actions | Intentionally absent | Target-native local validation and recorded receipts are required. |

## Active checkpoint — expanded frozen OLS projection after categorical rebasing

The exact pinned offline reference environment, final-sample differential, full
licensed-Stata profile, universal plugin audit/loader, debug/release Rust
suites, and independent explicit-dummy oracle are source-bound and successful
at `0e2aa609f9cbb17f8a205261d80680e68a752cef`. The activation receipt is sealed;
public activation and performance claims are both false. The unchanged frozen
30-executable/10-blocked projection remains resealed at exact historical source
`d58310783150e81739ea87eee868fd6bc8334b33`.

The next formally required gate is to expand the frozen executable projection
only for the now-supported categorical-interaction surface, rerun its complete
licensed/Rust/loader/rollback qualification, and reseal the resulting exact
partition. A manifest blocker may close only if the full frozen case—not just
the narrower differential fixture—is executable and exact-green.

Independent explicit-dummy Rust oracles remain mandatory. A successful
`reghdfe` comparison is necessary but cannot replace independent mathematical
checks. Public dispatch stays disabled throughout this gate.

## Current handoff / resume point

Expand and reseal the frozen hidden OLS projection for the accepted rebased
categorical-interaction surface. Do not advance to absorbed slopes, strings,
time-series materialization, iweights, saved outputs, or public postestimation
until that exact partition gate is green.

## Current blockers — exact frozen manifest inventory

The historical sealed candidate executed 30 of the frozen 40 semantic cases.
Current main has exact-green hidden categorical-interaction implementation and
final-sample rebasing, but that larger surface has not yet been admitted to a
new frozen projection receipt; these ten cases therefore remain formally open:

1. `multi_fe_interactions` — hidden implementation, all 12 original interaction differentials, final-sample categorical rebasing, independent explicit-dummy oracles, and a 14-case pinned reference probe are exact-green; the full frozen case and expanded projection reseal remain required.
2. `intercept_and_slope` — absorbed intercept plus slope.
3. `slope_only_multiple` — multiple absorbed slope-only terms.
4. `string_fe` — string-source ingestion.
5. `time_series_lag` — Stata time-series materialization.
6. `time_series_lead_difference_gaps` — time-series materialization across gaps.
7. `iweight_probe` — importance-weight semantics.
8. `dof_continuous` — absorbed continuous-slope DoF.
9. `saved_outputs` — recovered/saved fixed-effect outputs.
10. `postestimation_test_summarize` — public postestimation.

Ordinary factor support is present only for the bounded numeric surface; broader
ordinary factor/interactions and their public parser/display contract remain
activation work alongside the listed blockers. Prediction (`xb`/`xbd`), saved
absorbed contributions, and the full public syntax/postestimation surface also
remain gated. Public `stfeols`, `stfepois`, `estimate-ols`, and `estimate-ppml`
all continue to return `r(498)`. No real Linux/Windows or native Intel-host
macOS validation exists.

## Next evidence to record

Current order of operations: expand the frozen executable projection for the
accepted rebased categorical-interaction surface and reseal it at one exact
source; only then implement the remaining frozen blockers and public
parser/prediction/saved-output/postestimation surface in manifest order.
Historical bullets below remain useful detail but do not supersede this order.

- The active 30/10 candidate receipt now binds source `d58310783150`, the
  identical staged plugin, the complete frozen partition, failure/allocation
  sweeps, rollback/Break fixtures, and the external receipt identity. It
  continues to state `public_activation=false` and `performance=unmeasured`.
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
  sealed Stata 18 anchors pass. It exposes no plugin result or VCE. The
  certified iid, HC1, and clustered compositions are its only eligible
  variance-scale consumers; complete estimator parity remains gated before
  integration.
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
  the iid and robust checkpoints, and with pre-mobility cluster nesting plus
  distinct reported/fit/correction DoF by the clustered checkpoint. Atomic
  posting and estimator-level parity remain later gates.
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
- The core-only prepared HC1 checkpoint is independently reviewed through
  `a7a01a4` plus hot-path and composition-oracle repair `32008af`. It constructs
  fit plus absorbed DoF in one call, accepts unit/a/f/p objectives, uses
  `N_effective/df_r`, and applies pinned score-meat multipliers `1`, raw
  frequency multiplicity, and squared normalized analytic/probability weight.
  Its only added observation-by-coordinate object is an ephemeral kept-score
  `N x q` matrix consumed by strict MAP and destroyed before return; it creates
  no N-row constant and retains no second residual or weight vector. A direct
  fallibly allocated reported-position map gives O(1) covariance-loop lookup.
  The production bread-transformed influence Gram must agree with an independent
  meat-then-sandwich reconstruction, and separate score inverse/intercept,
  HC1, symmetry/SE, PSD, Wald, and statistic certificates must pass. Exact-fit
  and singular PSD covariance are accepted, singular slope covariance makes F
  undefined, and material indefiniteness is fatal. Full mutation and allocation
  sweeps pass. Independent explicit-dummy cases cover virtual/no-absorb,
  disconnected two-way, repeated identical, random/permuted, all weight types,
  and frequency expansion; sealed one-FE robust and pweight covariance anchors
  pass. No legacy VCE, plugin/public/posting path, FE recovery, or cluster logic
  is used.
- The core-only prepared clustered checkpoint is independently reviewed
  through `b226d62` plus test-gate repair `89132055`. It owns numeric cluster
  preparation on the authoritative final sample, reason-6 precedence,
  collision-free declared interactions, cluster-aware absorbed DoF, all
  nonempty inclusion--exclusion subsets for Q=1--10, a single common pinned
  finite-sample correction, and an independently reconstructed covariance.
  One-way output is strict PSD without repair. Multiway output preserves the
  exact pinned two-stage CGM behavior: clip an indefinite full covariance,
  separately clip the original raw slope block around a reported `_cons`,
  restore that block, and never reclip the final full covariance. In that exact
  restored state only, final full indefiniteness is diagnostic while the
  pre-restore full covariance and restored slope block remain certified PSD;
  final symmetry, finiteness, nonnegative diagonal/SE, and slope-Wald policy
  remain mandatory. Internal explicit-dummy, raw-meat, transform, and
  certificate comparisons are no looser than `2e-10`. The frozen DEFAULT-
  `reghdfe` receipt alone uses scoped absolute behavior caps of `1e-7`, `2e-6`,
  `4e-6`, and `1e-7` for the one-, two-, three-way, and declared-interaction
  cases after exact sample/count/subset/DoF/coordinate/PSD assertions; these
  caps record upstream MAP stopping-path parity and never relax the scientific
  core. The sealed interaction anchor is N=24, G=16, `df_a=18`, all three
  residual-DoF views and the correction denominator equal 5, axis
  `[x1,_cons]`, and strict one-way/no-adjustment state. Fail-at-every,
  undersize-at-every, mutation, weights, nesting, default/`noconstant`, virtual
  constant, disconnected/repeated FE, paired clusters, rank-zero and
  intercept-only tests pass. No legacy VCE/DoF, plugin/public/posting path, FE
  recovery, or public action is used.
- The hidden atomic prepared-OLS transport is independently approved through
  `7f61650`, repair `0623241`, and candidate hardening `d731b327`.
  Action/result/plugin handshakes, complete
  result adaptation, one-load/one-call iid/HC1/cluster selection, exact shadow
  layouts, receipt-last ordering, no work after `COMPLETE`, and exhaustive fake
  and actual-loader output-store failures pass. All 11 C matrix names are
  prepared before commit and reused by pointer; failure errors and reports are
  preallocated/static. This is logical atomicity over private shadow outputs,
  not physical SPI rollback.
- The hidden licensed-Stata eclass transaction is independently approved through
  `3473f49` and candidate hardening `d731b327`. Its canonical count/byte-length
  bundle is fully decoded before the
  handshake or SPI reads; ordinary argv remains available to the mock and C
  harnesses. `_estimates hold ..., restore nullok varname()` protects prior
  state; Rust completes N/K/K² validation and omission markers before stores,
  while Stata retains constant-size receipt/table/transaction checks. Only the
  final `nobreak` residual rename plus unhold makes the estimate logical. A
  whole-Mata b/V copy preserves the adjudicated multiway CGM restored state
  exactly. Seven deterministic rollback stages plus an actual Control-C after
  the one plugin call restore named and null prior estimates and leave no
  requested residual; all licensed hidden parity/transaction fixtures pass.
- The hidden StFixest versus pinned-`reghdfe` differential for native numeric
  categorical FE interactions is sealed across all 12 accepted cases at
  `dfd5ae0935f2b5a8925de14c316d699c5fff7e66`, and the frozen projection is
  resealed at `d58310783150e81739ea87eee868fd6bc8334b33`. Continue with the
  final-sample categorical rebase, then with
  independently gated string, absorbed-slope, broader ordinary-factor/time-
  series, saved-output, and postestimation work. No checkpoint may enable the
  public estimator or bypass the FE, solver-certificate, VCE, DoF,
  transaction, and sealed-parity gates.

## Historical qualification

Older hidden numeric receipts remain immutable historical artifacts. They are
not active candidate evidence and must not be used in place of the external
identity manifest's current `20260822T132813Z-d731b32` target.

Earlier documentation described a loader-selection defect and a runner that
accepted Stata's zero process status without a fixture terminal sentinel. The
merged repair selects macOS correctly for Stata's `Unix` report and requires
`END_FIXTURE|...|rc=0`; only the corrected receipt is accepted as evidence.
