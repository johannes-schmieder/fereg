# Scientific contracts and OLS reference gate

Architecture, source presence, and native unit tests are not estimator
validation. `stfeols` and `stfepois` remain fail-closed and post no estimate.

| Contract | Current evidence | Enablement gate |
|---|---|---|
| Candidate vs effective sample | Numeric availability/policy precedence and compact row map are unit-tested. The sample-bound clustered preparation applies missing-cluster reason 6 after FE availability and before singleton deletion, never lets cluster counts delete singletons, and factorizes cluster interactions only on the final compact sample. | Composed plugin/Stata final-sample identity including FE, cluster, singleton, and weight rules |
| OLS weights | The licensed reference probes are sealed. Guarded preparation implements analytic, frequency, and probability input domains, and the core-only prepared coefficient checkpoint applies each as a positive least-squares objective, is invariant to common scaling, and matches independent WLS/row-expansion oracles. Its covariance-coordinate bread uses the pinned cross-product scale: unit weights for unweighted fits, raw frequency weights, and analytic/probability weights normalized to physical N. The same-call iid composite uses physical N for unweighted/analytic models, exact frequency multiplicity for frequency N and residual DoF, preserves raw supplied sums separately, and rejects pweight iid inference. The separate same-call robust composite accepts pweights and implements the pinned HC1 meat multipliers: one for unit weights, raw multiplicity for fweights, and squared normalized objective weight for aweights/pweights. Analytic/pweight inference is common-scale invariant and fweight inference matches literal expansion. The hidden eclass helper posts the selected bounded weight metadata after the certified native transport completes. | Broader estimator-level sample, coefficient, VCE, DoF, and metadata parity for every enabled public kind |
| Singleton pruning | Developer one-call preparation matches the sealed two-way cascade (`N=4`, five reason-8 drops), `keepsingletons`, analytic-weight identities, and pinned frequency-multiplicity rule; aligned sources are compacted and re-factorized | Independent review plus eventual estimator-level final-sample parity before posting |
| Fixed-effect absorption | Final post-singleton numeric FE codes produce a deduplicated structural topology and a fallible intercept-only borrowed view with no code or membership copy. The prepared-RHS bridge revalidates sample/model/recipe identities and constructs exact Y plus generated X order in one fallible checked allocation. An opt-in developer action runs strict borrowed MAP before its first diagnostic write, discards residuals, and leaves every diagnostic untouched on failure. Empty explicit topology performs the virtual structural-constant projection in one certified iteration. Explicit-dummy and actual-loader tests cover unit/a/f/p objectives, repeated/disconnected sources, random designs, permutations, scaling, and no-absorb behavior. Final independent review approves this repaired path through `adbc345`. | Remaining FE forms and estimator-level sealed parity before any numerical result posting |
| OLS/WLS coefficients and bread | The isolated prepared-data checkpoint uses strict borrowed MAP plus scaled deterministic pivoted Householder QR and preserves canonical kept/omitted/FE-collinear/rank-deficient slots. Empty explicit FE topology has a virtual one-level structural constant under both default reporting and `noconstant`; weighted global demeaning uses the existing RHS plus six fallible O(K) buffers and no N-row code or membership. `_cons` is appended only when requested and structural, as `NormalizedAbsorbedIntercept = mean_w(y) - mean_w(X)b`; it never enters finite QR and its reported coordinate adds no rank, while the virtual structural effect owns `df_a=1`. Literal constants are FE-collinear and full factor-indicator spans retain L-1 finite contrasts. The checkpoint returns canonical covariance-coordinate inverse cross-product bread, extending the finite within block with `side = -B mean_w(X)` and `corner = 1/N_ref + mean_w(X)' B mean_w(X)`. It returns only after MAP, rank, all-column score, full inverse, symmetry, intercept-formula, and normalization certificates pass. Paired default/`noconstant`, explicit-dummy, unit/a/f/p, permutation, extreme-scale, allocation, corruption, and sealed Stata 18 tests pass. The bare fit remains not VCE, FE recovery, result posting, or a public command; only the certified same-call iid, HC1, and clustered compositions may consume it. | Remaining model/FE forms, broader sealed estimator-level parity, and public activation review |
| Prepared absorbed DoF | A separate core-only report consumes final topology plus borrowed categorical codes and implements pinned `None`, `FirstPair`, and default `Pairwise` structural redundancy for the supported intercept-only FE surface. For nonempty no-absorb models it emits a distinct tagged `ImplicitNoAbsorb` `_cons` row with one exact coefficient and `df_a=1` under both default and `noconstant`, without inventing wire/source IDs. Explicit rows retain wire identity and exactness metadata. The clustered same-call view tests nesting against every declared cluster dimension before mobility redundancy, fully removes a nested explicit effect from absorbed rank, retains its category count separately, and never treats the synthetic virtual constant as nested. It exposes unclustered, reported, fit-statistic, and correction-denominator DoF separately. Fallible iterative union-find creates no N-scale copy; branch-specific row/label fail-at-every and undersize-at-every tests cover the synthetic path. Sealed `df_a=8/19/21` and clustered nesting/interaction anchors plus independent graph-search oracles pass. | Atomic posting of each distinct DoF meaning and sealed estimator parity |
| VCE, residual DoF, and fit statistics | `CertifiedPreparedOlsUnadjusted` constructs fit plus absorbed DoF internally and returns positive residual DoF, iid `RSS/df_r * bread`, centered structural RSS/TSS/within TSS, both R-squared families, RMSE, Gaussian likelihoods, and reference-compatible iid Wald F on explicit undefined policies. `CertifiedPreparedOlsRobust` separately rebuilds an ephemeral kept-coordinate score design and returns pinned HC1 for unit/a/f/p weights. `CertifiedPreparedOlsClustered` adds sample-bound 1--10-way numeric clusters, treats each declared source tuple as one dimension, aggregates `sum(omega_i e_i z_i)` by every nonempty subset, applies inclusion--exclusion and one common `((N_effective-1)/correction_denominator_dof) * G_min/(G_min-1)` correction, and requires at least two groups per dimension. Reported `df_r=min(N_effective-df_a-df_m,G_min-1)` remains distinct from fit-statistic and correction DoF. Q=1 is strict PSD without repair. For Q>1 the pinned CGM policy clips an indefinite full matrix, separately clips the original raw slope block when `_cons` and slopes are reported, restores that block, and never reclips final V. Only that exact restored state may have a diagnostic negative final full eigenvalue; the pre-restore full matrix, restored slope block, final symmetry/finite/nonnegative-diagonal/SE rules, and slope-only Wald are certified independently. Internal explicit-dummy/raw/transform/certificate comparisons are no looser than `2e-10`. Separate frozen DEFAULT-`reghdfe` receipt caps of `1e-7`, `2e-6`, `4e-6`, and `1e-7` for one-, two-, three-way and declared-interaction cases apply only after exact structural assertions and characterize upstream MAP stopping, not scientific tolerance. Review approves `b226d62` plus `89132055`. These scientific objects do not post public `e()` state; the hidden candidate may adapt only a complete selected object. | Remaining blocked model forms, complete sealed candidate parity, and reviewed public activation |
| Native shadow-result transport | Hidden `estimate-ols-test` requires exact action 1.1/result wire 1.1/plugin identity and developer confirmation, loads sources once, runs one selected certified iid/HC1/clustered composition, and fallibly adapts coefficients, full V, canonical statuses, fit, distinct DoF, solver/FE/cluster/subset/certificate metadata, sample, residual, and reasons. Rust completes every N/K/K² output validation and native omission marker before stores. All 11 matrix names/layouts are prevalidated and C-prepared. Wire 1.1 separates the candidate-sample physical coefficient capacity from the final-sample logical graph; the latter alone is stored in the leading shadow block and posted. Receipt `COMPLETE` is last; SPI has no rollback, so partial physical shadow writes remain logically invalid. Preallocated/static store errors add no post-failure allocation. Candidate-bound Fake/live failure injection passes at 222/219 ordinals through `d731b327`. A canonical count/byte-length bundle carries the hidden licensed request and is rejected in full before handshake/read on malformed input. | Preserve this receipt-last boundary in every future public activation path |
| Hidden licensed eclass transaction | `_stfixest_estimate_ols_test` holds named or null prior estimates, creates private shadows, makes one bundled plugin call, performs only constant-size receipt/table identity and transaction checks after Rust's complete result validation, and posts the selected bounded result. Whole-matrix Mata copies preserve exact q2/q3 CGM restored-state covariance and negative determinant without Stata's nonsymmetry warning. K=0 posts no b/V. Deterministic failure stages, scientific failure, and actual Control-C restore prior `e()`/`e(sample)` and expose no requested residual. Licensed iid/HC1/cluster, unit/a/f/p, default/`noconstant`, omission, sample/residual, and q2/q3 fixtures pass through candidate hardening `d731b327`. Public actions remain `r(498)`. | Remaining blocked semantic cases plus reviewed public-dispatch activation across the full supported surface |
| Hidden numeric OLS candidate receipt | `_stfixest_ols_hidden_numeric_v1` executes exactly 31 cases and records nine blockers from the frozen 40-case manifest. The full `multi_fe_interactions` case now binds absorbed categorical tuples, ordinary factor interactions, final-sample rebasing, omission/status axes, DoF, iid covariance, and reference-compatible `e(indepvars)`. Every capture and supporting gate uses the same isolated plugin (SHA-256 `6630e8837b96d05cfe858e21e111c4f97c5a3a33830f46587f81e7337136d6e2`, embedded `f61fee923db2`). The active immutable receipt is externally bound by path, payload/file hashes, git blob, and mode; certificates, q2/q3 anchors, store failures, rollback, actual Break, and the independent explicit-dummy oracles pass. `public_activation=false`; performance is unmeasured. | Add the nine remaining blocked cases in manifest order, next `intercept_and_slope`, before any public activation |
| OLS outputs | The hidden eclass helper posts bounded b/V/status/fit/DoF/diagnostics/e(sample)/optional-residual results transactionally. Reference cases additionally request `xb`, `xbd`, absorbed sum, named FEs, `test`, and `estat summarize`; those remaining outputs and public postestimation are disabled. | Observation-ID-aligned broader licensed parity and remaining output/postestimation gates |
| PPML | Outside Checkpoint 1A; public command disabled | Later IRLS, separation, VCE, and postestimation gates |

A successful `stfixest, selftest` proves only its documented native smoke
tests. A successful OLS reference capture proves only what the pinned reference
did; it is not evidence that StFixest reproduced it.

## Pinned absorbed-intercept and bread contract

The normalized absorbed-model intercept is fixed by pinned `reghdfe` 6.14.1
commit `29b2b203f534413369d675d0c0f674d61291f536`. That source saves the weighted
means of Y and X, assigns finite rank before extending the reported constant,
and implements:

```text
b_cons = mean_w(y) - mean_w(X) b
side   = -mean_w(X) B
corner = 1 / N_ref + mean_w(X) B mean_w(X)'
```

Here `B` is the inverse within-X cross-product on canonical kept finite
coordinates. `N_ref` is physical N for unweighted, analytic-, and
probability-weighted fits and the exact frequency sum for frequency weights.
The reference cross-product weights are one when unweighted, raw frequency
weights for frequency weighting, and raw analytic/probability weights rescaled
to sum to physical N. Consequently common analytic/probability-weight scaling
does not change the bread, while multiplying every frequency weight by an
integer divides the bread by that integer, exactly as literal row replication
does.

The reported `_cons` is a normalization coordinate. Recovering it does not
recover, save, or identify individual fixed-effect coefficients, and it adds
neither finite rank nor absorbed rank. The bare fit artifact is only inverse
cross-product bread: it must not be called `e(V)` or combined with an older
variance routine. The separately certified same-call iid, HC1, and clustered
composites are the only current eligible consumers. The hidden transport may
adapt one selected complete composite, but neither the core objects nor the
transport posts `e()`.

## Checkpoint 1A reference protocol

The OLS-v1 manifest is the sole command and input inventory. It freezes:

- each source CSV or deterministic generator by SHA-256;
- the reference-only Stata entrypoint by SHA-256;
- seed, model command, expected success policy, capture fields, and case order;
- one cold plus five warm repetitions at both one and eight threads for every
  primary benchmark design of at least ten million observations;
- a closed coverage vocabulary spanning model construction, samples, all
  weight probes, VCE/DoF, requested outputs, and errors.

The runner rejects unknown schema fields, unsafe command control syntax,
missing coverage, duplicate case IDs, stale hashes, or an input that changes
during the run. It accepts no user-supplied `.dta` cache. Fixture bytes are
copied into a private directory; large benchmark data are regenerated once in
a dedicated, unmeasured licensed-Stata preparation process, sealed by SHA-256,
made read-only, and then loaded by every fresh measured estimator process.

Reference discovery is also closed. The only eligible layout is an absolute
external root containing direct-child `reghdfe`, `ftools`, and `require` git
checkouts. All three must match the exact 40-character commits in
`references.lock.toml`, have
clean worktrees, contain the locked tracked files, and resolve from the exact
staged ado directories. PERSONAL, PLUS, OLDPLACE, SITE, and the working
directory are isolated so a globally installed command cannot satisfy the
preflight.

The dependency closure is reghdfe 6.14.1 at `29b2b203...`, ftools 2.50.0 at
`d6e24d66...`, and require 1.3.1 at `f4760b86...`. Each required tracked source
file is separately SHA-256 locked and verified after `git archive` staging.
`lftools.mlib` is not a source input: it is generated from the locked ftools
sources inside the temporary capture directory. The licensed reference receipt
is sealed and validated, but it proves reference behavior only; it is not
StFixest parity evidence.

## Deterministic capture fields

Successful cases capture selected `e()` scalars and every requested matrix
cell using Stata hexadecimal numerical formatting, canonical matrix names,
selected macros with delimiter escaping, return code, timings, and requested
observation outputs keyed by stable `obs_id`. Error cases capture their exact
return code and no stale `e()` state. A case-begin/end pair and a unique final
`END_FIXTURE|ols_v1_reference_capture|rc=0` record are mandatory.

Requested evidence may not be recorded as absent. Verification requires
canonical Stata hex values, positive unique matrix coordinates forming complete
rectangles, and exactly one value per requested field and unique observation ID.
Benchmark evidence additionally seals sample and singleton identities, peak RSS,
host/Stata/thread metadata, and remains explicitly blocked while `stfeols` is
disabled. A future pass requires at least 1.5x geometric-mean speedup, no slower
primary case, and no worse primary-case RSS.

Performance is evaluated only after scientific parity. Each arm must provide
coefficient names and kept/omitted status, coefficient values, the full VCE,
DoF, fit statistics, objective, and converged normal-equation, FE-orthogonality,
and VCE-symmetry certificates at the frozen tolerances. A scientific mismatch
blocks the gate before timing or memory is inspected. Every benchmark
repetition runs in a fresh OS process; its receipt records whether RSS came from
POSIX `wait4().ru_maxrss` or Windows `GetProcessMemoryInfo`, with values
normalized to bytes and an exact `fresh_estimator_process_prepared_inputs`
scope. Reference resolution, dependency checks, private `ftools` compilation,
and deterministic dataset generation finish in separate processes before RSS
measurement begins. Evidence whose high-water includes any of those preparation
steps is rejected. Warm repetitions perform one untimed estimator warm-up inside
their otherwise fresh measured estimator process.

Matrix dimensions are emitted as separate records before matrix cells. Receipt
verification requires every coordinate in the declared rectangle exactly once;
dimensions are never inferred from the largest observed coordinate.

The capture intentionally excludes display text, raw `ereturn list` output,
host paths, executable paths, usernames, license/serial text, and the raw Stata
log. Those licensed/raw artifacts live only in a temporary directory.

## Sealed receipt contract

An eligible receipt contains a canonical JSON payload and a SHA-256 seal over
that payload. The payload binds the manifest hash, all input hashes, exact
reference commits and git-tree identities, sanitized Stata environment fields,
case order, status, timings, and semantic results. Creation uses exclusive
write and removes write permission. Verification fails if:

- the seal or read-only mode is missing;
- the manifest, a fixture, generator, capture entrypoint, or reference commit
  changed;
- the case inventory differs;
- a local absolute path, raw log, username, license, or serial field appears.

Receipts are immutable evidence. Corrections require a new manifest version
and a new receipt, never an in-place edit.

The hidden numeric candidate adds a second, receipt-external identity layer:
`benchmarks/manifests/ols_hidden_numeric_candidate_receipt_v1.json` binds the
one active committed receipt's path, canonical payload seal, complete file
SHA-256, git object format/blob, and mode. Verification also binds the complete
committed source tree and the exact staged plugin identity shared by the Stata
capture and every supporting gate. A payload that is edited and validly
resealed therefore remains ineligible unless the independently reviewed
external identity is deliberately replaced. Historical receipts remain
immutable but are not active evidence.

## Numerical adjudication

Once a reference receipt exists, well-conditioned parity uses the governing
targets: coefficients `rtol=1e-7, atol=1e-10`; standard errors
`rtol=1e-6, atol=1e-10`; residuals/fitted values `rtol=1e-7, atol=1e-9`.
Ill-conditioned and nonunique-FE cases are adjudicated on fitted values,
residuals, identified combinations, objective, and certificates rather than an
arbitrary FE normalization. Exact equality remains mandatory for success/error
status, final sample, omitted columns, requested VCE/DoF policy, cluster counts,
and singleton accounting.

## Locked numeric-preparation contract

Numeric preparation has no default scientific policy. Candidate exclusion
precedes all checks. Otherwise the current mechanical order is outcome
availability/policy, regressor availability, weight availability/policy, then
adjustment availability/policy. Explicit missing masks and nonfinite `f64`
values are unavailable; retained bits and original row order remain exact.
Estimator-specific rules, not this primitive, decide whether empty output,
negative values, or a weight-domain boundary is a scientific error.
