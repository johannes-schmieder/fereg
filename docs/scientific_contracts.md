# Scientific contracts and OLS reference gate

Architecture, source presence, and native unit tests are not estimator
validation. `stfeols` and `stfepois` remain fail-closed and post no estimate.

| Contract | Current evidence | Enablement gate |
|---|---|---|
| Candidate vs effective sample | Numeric availability/policy precedence and compact row map are unit-tested | Composed Stata/Rust final-sample identity including FE, cluster, singleton, and weight rules |
| OLS weights | The licensed reference probes are sealed. Guarded preparation implements analytic, frequency, and probability input domains, and the core-only prepared coefficient checkpoint applies each as a positive least-squares objective, is invariant to common scaling, and matches independent WLS/row-expansion oracles. Its covariance-coordinate bread uses the pinned cross-product scale: unit weights for unweighted fits, raw frequency weights, and analytic/probability weights normalized to physical N. The same-call iid composite uses physical N for unweighted/analytic models, exact frequency multiplicity for frequency N and residual DoF, preserves raw supplied sums separately, and rejects pweight iid inference because the reference requires robust VCE. No weight kind is plugin-enabled or posted. | Exact estimator-level sample, coefficient, VCE, DoF, and metadata parity for every enabled kind |
| Singleton pruning | Developer one-call preparation matches the sealed two-way cascade (`N=4`, five reason-8 drops), `keepsingletons`, analytic-weight identities, and pinned frequency-multiplicity rule; aligned sources are compacted and re-factorized | Independent review plus eventual estimator-level final-sample parity before posting |
| Fixed-effect absorption | Final post-singleton numeric FE codes produce a deduplicated structural topology and a fallible intercept-only borrowed view with no code or membership copy. The prepared-RHS bridge revalidates sample/model/recipe identities and constructs exact Y plus generated X order in one fallible checked allocation. An opt-in developer action runs strict borrowed MAP before its first diagnostic write, discards residuals, and leaves every diagnostic untouched on failure. Empty explicit topology performs the virtual structural-constant projection in one certified iteration. Explicit-dummy and actual-loader tests cover unit/a/f/p objectives, repeated/disconnected sources, random designs, permutations, scaling, and no-absorb behavior. Final independent review approves this repaired path through `adbc345`. | Remaining FE forms and estimator-level sealed parity before any numerical result posting |
| OLS/WLS coefficients and bread | The isolated prepared-data checkpoint uses strict borrowed MAP plus scaled deterministic pivoted Householder QR and preserves canonical kept/omitted/FE-collinear/rank-deficient slots. Empty explicit FE topology has a virtual one-level structural constant under both default reporting and `noconstant`; weighted global demeaning uses the existing RHS plus six fallible O(K) buffers and no N-row code or membership. `_cons` is appended only when requested and structural, as `NormalizedAbsorbedIntercept = mean_w(y) - mean_w(X)b`; it never enters finite QR and its reported coordinate adds no rank, while the virtual structural effect owns `df_a=1`. Literal constants are FE-collinear and full factor-indicator spans retain L-1 finite contrasts. The checkpoint returns canonical covariance-coordinate inverse cross-product bread, extending the finite within block with `side = -B mean_w(X)` and `corner = 1/N_ref + mean_w(X)' B mean_w(X)`. It returns only after MAP, rank, all-column score, full inverse, symmetry, intercept-formula, and normalization certificates pass. Paired default/`noconstant`, explicit-dummy, unit/a/f/p, permutation, extreme-scale, allocation, corruption, and sealed Stata 18 tests pass. The bare fit remains not VCE, FE recovery, plugin result posting, or a public command. | Prepared robust/cluster score meat and corrections, remaining model/FE forms, one-call atomic result posting, and complete estimator-level sealed parity |
| Prepared absorbed DoF | A separate core-only report consumes final topology plus borrowed categorical codes and implements pinned `None`, `FirstPair`, and default `Pairwise` structural redundancy for the supported intercept-only FE surface. For nonempty no-absorb models it emits a distinct tagged `ImplicitNoAbsorb` `_cons` row with one exact coefficient and `df_a=1` under both default and `noconstant`, without inventing wire/source IDs. Explicit rows retain wire identity and exactness metadata. Fallible iterative union-find creates no N-scale copy; branch-specific row/label fail-at-every and undersize-at-every tests cover the synthetic path. Sealed `df_a=8/19/21` anchors and an independent graph-search oracle pass. | Cluster nesting, robust/cluster corrections, atomic posting, and sealed estimator parity |
| VCE, residual DoF, and fit statistics | `CertifiedPreparedOlsUnadjusted` constructs fit plus absorbed DoF internally and returns positive residual DoF, iid `RSS/df_r * bread`, centered structural RSS/TSS/within TSS, both R-squared families, RMSE, Gaussian likelihoods, and reference-compatible Wald `b' inv(V_slopes) b / df_m` on explicit undefined policies. Independent certificates recompute primitives, DoF identities, covariance/SE scales, Wald solve residuals, and every statistic formula without reusing the builder; mutation tests cover each family. Unweighted/analytic effective N is physical N, frequency effective N is exact multiplicity, and positive pweight iid inference is rejected. Paired default/`noconstant`, explicit-dummy/random/permutation/extreme-weight/frequency-expansion, weak-signal Wald, every-allocation, corruption, and sealed Stata 18 anchors pass. For reported dimension P and slope dimension q, fallible storage is P² covariance plus P standard errors and q slope positions plus q² Cholesky plus q solve entries: P²+q² square and P+2q vector elements, with no N copy. Final independent review approves the repair series through `adbc345`. | Prepared robust/cluster meat, corrections and nesting; then posted `e(V)`/DoF parity |
| OLS outputs | Reference cases request sample, residual, `xb`, `xbd`, absorbed sum, named FEs, `test`, and `estat summarize` | Observation-ID-aligned parity and atomic failure tests |
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
variance routine. The separately certified same-call iid composite is the only
current eligible consumer, and it still performs no result posting.

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
