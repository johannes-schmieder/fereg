# Scientific contracts and OLS reference gate

Architecture, source presence, and native unit tests are not estimator
validation. `stfeols` and `stfepois` remain fail-closed and post no estimate.

| Contract | Current evidence | Enablement gate |
|---|---|---|
| Candidate vs effective sample | Numeric availability/policy precedence and compact row map are unit-tested | Composed Stata/Rust final-sample identity including FE, cluster, singleton, and weight rules |
| OLS weights | Frozen probes and capture protocol exist; no licensed capture is sealed | Per-kind reference receipt, independent WLS oracle, and exact sample parity |
| Singleton pruning | Native source/tests exist; reference cascade/`keepsingletons` cases are manifest-bound | Exact retained IDs and drop counts against the sealed reference |
| Fixed-effect absorption | Native solver source/tests exist | Explicit-dummy fitted/residual parity and strict FE certificate through the one-call path |
| OLS/WLS coefficients | Native source/tests exist; public command disabled | Independent oracle plus sealed `reghdfe` parity at fixed tolerances |
| VCE and DoF | Native source/tests exist; reference cases cover iid, robust, 1–3 way clustering, interactions, nesting, and `dof()` policies | Score/meat/correction oracle and posted `e(V)`/DoF parity |
| OLS outputs | Reference cases request sample, residual, `xb`, `xbd`, absorbed sum, named FEs, `test`, and `estat summarize` | Observation-ID-aligned parity and atomic failure tests |
| PPML | Outside Checkpoint 1A; public command disabled | Later IRLS, separation, VCE, and postestimation gates |

A successful `stfixest, selftest` proves only its documented native smoke
tests. A successful OLS reference capture proves only what the pinned reference
did; it is not evidence that StFixest reproduced it.

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
copied into a private directory; large benchmark data are regenerated there in
the licensed Stata process.

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
sources inside the temporary capture directory. No licensed capture has yet
been sealed, so reference readiness is not a parity claim.

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
normalized to bytes. Warm repetitions perform one untimed in-process warm-up
inside their otherwise fresh measured process.

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
