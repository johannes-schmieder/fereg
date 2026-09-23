# OLS scientific core

The Stata-independent OLS core backs public `fereg` 0.5.0. ADR-0006 requires
positive residual DoF; signed DoF and negative IID variance are ineligible.

## Pipeline

```text
validate and compact candidate data
→ factorize final FE/cluster topology
→ prune singletons and rebase recipes
→ build fast or scratch-backed low representation
→ certified FE residualization
→ rank-aware finite regression
→ reconstruct and re-absorb residuals
→ score, bread, and inverse certificates
→ centralized DoF and fit statistics
→ iid/robust/multiway-cluster covariance
→ residual, absorbed, and normalized saved-FE outputs
→ validate the complete native result
→ transactionally post in Stata
```

Every failure is terminal. No change-in-iterate or recurrence stop substitutes
for fresh projection, normal-equation, inverse, covariance, and fit
certificates.

## Absorption, rank, and intercept

The prepared model owns one compact sample, shared numeric/string categorical
codes, rebased factor recipes, and deduplicated FE/cluster topology. It supports
intercept, slope-only, intercept-plus-slope, multiple slope channels,
disconnected components, and empty/no-regressor models.

Finite regression preserves a canonical status for every coefficient slot.
Both fast and scratch-backed low execution first attempt guarded
diagonal-pivoted Cholesky, including widths above 64 under ADRs 0030 and 0111.
Every pivot must clear the stronger square-root rank guard, and the result
must pass the independent score and inverse checks. Weak or rank-deficient
designs use deterministic scaled column-pivoted Householder QR, which owns
omission decisions. The low trial uses its existing compensated moments and
does not mutate scratch or the response. Low fits still reconstruct residuals
from original columns and independently project and certify them. No dense
within matrix or cross-generation cache is introduced.
Only the identified subspace is solved. The normalized reported `_cons`, when structurally
present, is

```text
mean_w(y) - mean_w(X) b
```

It is a normalization coordinate, not an additional rank direction. Pure
all-slope-only absorbed models therefore do not report a structural constant.

For QR segments of at least 65,536 entries, reflector norms use a fixed
maximum scale and compensated squared-value sum, and serial reflector dot
products use compensated accumulation. Short checkpoint arithmetic and
pre-absorption reference scales remain unchanged. Both memory layouts reject
nonfinite transformed values, including the leading entry. Inverse-certificate
failures report rank, both errors and the unchanged tolerance.

Scratch-backed QR computes each next pivot norm from the resident transformed
target after its successful write, using the same stable norm and observation
order as a read-back. The initial and final independent scratch scans remain;
no norm downdate or additional buffer is used. See
[ADR 0109](adr/0109-low-memory-qr-target-norms.md).

For raw matrices wider than 64 columns, ADR 0031 groups adjacent columns in
blocks of eight when enough blocks exist for the native worker team. Raw
maxima, scaled norms and compensated means retain their prior per-column
arithmetic and observation order. Scratch is fixed-size worker stack; narrower
raw matrices retain the ADR 0012 row-partial algorithm.

For prepared numeric models with 8–64 generated columns and at least 4,096
rows, RHS construction and independent raw residual reconstruction bind source
columns once and process 128-row tiles. The RHS reuses its fallibly reserved
allocation; initialization and wide finite-value scans use the installed
worker pool. General validation before pool installation stays serial. Residual
updates preserve generated-column order and fused multiply-add arithmetic for
each observation. Source, result and overflow checks remain in force; no extra
observation-scale matrix or solver-route change is introduced. Other recipes
and widths retain the general evaluator.

Categorical absorbed-DoF construction may reuse the topology check from its
complete prepared-data validation for one immutable single-source owner. All
numeric, recipe, slope, cluster-nesting and rank-policy checks still run.
Eligible unit-weight, fast one-FE IID and robust fits with at most 16 generated
columns, at least 16,384 active rows and at least eight workers may run the
allocation-free numeric/topology validation prefix beside fitting. Its errors
remain deferred until the original DoF boundary; metadata checks and report
allocation remain there. Interaction topologies retain ordinary validation.

Low-memory categorical projection reuses the fixed LSMR operator and
preconditioner across response/regressor batches under ADR 0032. Every batch
still receives its independent FE certificate. Retained LSMR state is
released before finite QR and output recovery, or before any CG continuation.

Fresh low-memory outcome and regressor blocks use the existing initial
absorption entry point. Eligible one-FE automatic projection omits the raw
certificate, retaining the independent projected-value certificate. The
outcome consumes its one-column matrix storage; within norms reuse certified
values only when the frame and projector use the same weight normalization.
Raw reference norms and warm residual handling remain unchanged; see
[ADR 0051](adr/0051-low-memory-fresh-block-projection.md).

## DoF, inference, and outputs

Centralized DoF combines effective N, finite rank, absorbed rank, nesting, and
the requested default/none/firstpair/continuous policy. Successful inference
requires `df_r > 0`. IID, HC1 robust, and one-/multiway clustered paths have
independent score, correction, subset, PSD/repair, Wald, and statistic
certificates.

Scratch score-Gram and robust covariance certificates update independent
matrix coordinates in contiguous compensated rows. Per-coordinate products
and observation order remain unchanged; each private row rejects any nonfinite
product, sum or correction before continuing. The two certificates retain
separate fresh traversals and existing buffers. See
[ADR 0110](adr/0110-compensated-scratch-certificate-rows.md).

Scratch-backed production robust covariance uses the same compensated row
helper for its upper-triangle influence outer product. It retains the scalar
compensated bread-times-score calculation and applies the complete estimator
multiplier before the outer product. Existing workspaces, per-coordinate order
and independent certificates remain unchanged; see
[ADR 0112](adr/0112-scratch-influence-covariance-rows.md).

Eligible fast intercept-FE fits with at least 16,384 rows, at most 64 model
columns and at least eight workers overlap independent source weight/outcome
statistics with fitting. Reuse requires the same immutable sources and exact
fitted weight-scale bits; a mismatch uses the original fitted-view scan.
Only source borrows and scalar results are retained, and each error remains
at its original inference boundary. Unit, AW, PW and FW semantics and all
certificates remain unchanged; see
[ADR 0084](adr/0084-weighted-independent-statistics-overlap.md).

Fast robust/clustered OLS packs candidate score values up to 32 MiB. Larger
eligible Cholesky fits retain the standardized solve allocation through the
existing strided view, with stable raw reference moments and independent score
checks. QR fits that cannot retain valid score storage use the existing certified
reconstruction path. This is a layout/cost cutoff, not a rank or memory-admission
policy; see [ADR 0043](adr/0043-ols-inference-strided-cache-policy.md).

Requested residual, absorbed, combined-prediction and saved-FE outputs use a
separate final projection target of at most 1e-14, independent of whether fast
or low execution retains FE coefficients. Initial absorption and rank/reference
policies keep their configured precision. A terminal CMG correction releases its
hierarchy before admitting certified CG; generic projection admits its actual
solver and any continuation. See
[ADR 0063](adr/0063-ols-derived-output-final-precision.md).

Rust returns complete b/V/status, fit, DoF, solver, FE, cluster, subset,
certificate, sample, residual, absorbed, saved-FE, resource, and timing
structures. The public ado makes one substantive plugin call and posts only
after the receipt is complete. `predict`, `test`, and `estat summarize`
consume the posted public result.

## Memory and evidence

Fast and low modes implement the same scientific contract; low uses bounded
scratch-backed columns and scores. Checked dimensions, fallible allocation,
cleanup, and injected failure paths are described in `memory_model.md`.

Independent explicit-dummy/mathematical oracles are required alongside exact
pinned Stata differentials. Samples, statuses, errors, singleton identities,
DoF/VCE policy, and outputs are exact; registered tolerances apply only to
eligible numerical comparisons. Qualification state is recorded in
`STATUS.md`.
