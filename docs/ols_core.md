# OLS scientific core

The Stata-independent OLS core is intended to back `stfeols`. Public dispatch
remains disabled until the full semantic, transaction, output, performance, and
release gates close.

ADR 0006 governs absorbed slopes: successful inference requires positive
residual DoF. Any temporary signed-DoF or negative-iid-variance route is
superseded and must not be treated as an eligible result.

## Pipeline

```text
validate and compact candidate data
→ construct final FE/cluster topology
→ prune singletons and rebase categorical recipes
→ build or stream Y/X blocks
→ certified FE residualization
→ rank-aware finite regression
→ reconstruct and re-absorb residuals
→ score, bread, and inverse certificates
→ centralized DoF and fit statistics
→ iid/robust/clustered covariance
→ requested FE recovery and predictions
→ validate complete native result
→ transactionally post in Stata
```

Every failure is terminal for the attempted result. No recurrence or
change-in-iterate criterion substitutes for a final FE projection certificate.

## Prepared model and FE absorption

The accepted bounded preparation surface owns one authoritative compact sample,
shared categorical encodings, final-sample factor recipes, and deduplicated FE
topology. Repeated terms reuse topology sources and membership.

Intercept-only effects use a borrowed design without copying observation-scale
codes or membership. Empty explicit topology is treated by prepared OLS as a
virtual one-level structural constant and is projected by weighted global
demeaning.

A guarded categorical solver supports an intercept plus at most one continuous
slope channel for development. It borrows the final-sample slope source, solves
group 2-by-2 systems, rejects locally rank-deficient channels, preallocates all
workspaces before mutation, and directly certifies both intercept and slope
projections. This engineering does not itself enable slope inference or public
dispatch.

## Coefficients, rank, and bread

The prepared fit:

- materializes Y plus generated finite columns in canonical order for the
  current fast-style checkpoint;
- runs strict MAP before deterministic scaled column-pivoted Householder QR;
- preserves a status for every canonical coefficient slot;
- solves only on the identified finite subspace;
- reconstructs a raw residual, re-absorbs it, and checks all generated-column
  score moments;
- independently checks finite and reported inverse identities, symmetry,
  positive bread diagonals, and the normalized intercept formula.

A structural reported `_cons` is appended after finite QR as:

```text
_cons = mean_w(y) - mean_w(X) b
```

It is a normalization coordinate and adds no rank. Its bread extension follows
the pinned formula documented in `scientific_contracts.md`.

Allocation, nonfinite input, rank ambiguity outside policy, failed MAP, failed
score, or failed bread/intercept certificates return no fit object.

## Degrees of freedom and inference

Centralized DoF composes effective N, finite rank, absorbed rank, cluster
nesting, and the requested DoF policy. Successful ordinary OLS inference
requires:

```text
df_r = N_effective - model_rank > 0
```

The iid covariance and fit statistics are published only after independent
formula and consistency certificates. HC1 robust and one-/multiway clustered
compositions have distinct weight, correction, nesting, subset, PSD/repair,
Wald, and statistic certificates.

For absorbed slopes, conservative reference metadata may be reported as
inexact, but it cannot authorize a successful negative-DoF covariance. The
active option-A gate uses a positive-DoF fixture and retains the old
over-absorbed construction as a fail-closed test.

## Native result and Stata transaction

The hidden `estimate-ols-test` path remains developer-only. Rust validates the
complete logical N/K/K² result before any store and writes only private shadow
destinations. Receipt `COMPLETE=1` is the final native operation.

The hidden ado helper holds prior estimates, makes one substantive plugin call,
validates the complete receipt and bounded metadata, posts whole b/V matrices,
and commits only in its final protected block. Store-failure injection and
actual Control-C tests require prior `e()` state and requested output variables
to remain logically unchanged on failure.

## Memory boundary

Current prepared fitting is a fast-style checkpoint with a resident compact
Y/X batch and ephemeral score design where required. All large dimensions use
checked arithmetic and fallible reservation. It is not the completed
low-memory estimator. Blockwise generation, streaming cross-products/scores,
scratch-file lifecycle, fast/low equivalence, and measured peak RSS remain
separate gates.

See `memory_model.md` for detailed allocation contracts.

## Evidence boundary

Independent explicit-dummy oracles are required alongside pinned Stata
differentials. Exact samples, statuses, DoF policy, cluster identities, and
failure codes are not tolerance-based. Numerical tolerances and receipt rules
are centralized in `scientific_contracts.md`.

The active accepted candidate and exact remaining blockers are recorded only in
`STATUS.md`.
