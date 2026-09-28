# Original zero-outcome PPML discrepancy: independent adjudication

Bounded development evidence, 2026-09-28. No timing rank or qualification.
The existing ADR0236 repair fixes the remaining original zero-outcome native
coefficient discrepancy. This milestone adds an independent diagnostic, not
another production estimator change. Historical receipts and NR stay unchanged.

## Bound input and scientific authority

Original campaign `20260926-systematic-light-58284f76`, cell
`ppml-two_fe_dense-n61440-k10-t4-zeros/warm-4`, seed31415927: N61440, K10,
two categorical FEs, unit weights, robust inference. Exact regenerated DTA
SHA256 `2ea62fa38b1d33c03b2bae8ba0dfed4da0b307b73ae16a6f05e0034a06d8f15c`
and every column hash match the captured input. Registrations bind every original
command receipt, reference runtime and controller; no comparator is rerun.

An independent integer recession witness identifies 106 all-zero first-FE
groups and 530 excluded zero rows. Its direction is zero on all retained and
positive-outcome rows and strictly negative on every excluded row. Surviving
levels have at least 5/67 rows, excluding singleton ambiguity. The independently
derived 60,910 retained original IDs have SHA256
`ecb9393bee58ec72c8ced77744610fb5118097390c7cd348e6b6046fb2dd2d0e`,
matching all five comparable commands' stored sample hash and row count.

A grounded full sparse-design Newton solve proposes the finite optimum;
the separately implemented 60-digit outward certificate encloses each finite
coefficient with radius **8.1911e-15** (rounded upward). A finite stationary
point excludes further separation on the retained sample. Total rank is 12806,
deviance 56764.66295875462. The independent robust sandwich supplies point VCE
and SE checks; no optimum-covariance exception is granted.

[ADR0237](../../../../docs/adr/0237-nonnegative-sample-ppml-adjudication.md)
explains the nonnegative-outcome extension, finite-optimum argument and separate
sample authority. Positive-only entry points, bounds and tolerances stay closed
and unchanged. Eight interval tests and connected/disconnected scalar dummy
Poisson/Hessian checks pass locally and in both SCC attempts, including separated
sample refusal at vanishing means, invalid inputs and memory refusal.

## Results

| Export/artifact | Maximum coefficient error | Certified coefficients | Other checks |
|---|---:|---|---|
| Original native | 4.13512e-10 | **Fail, x4** | Pass |
| Prior-current native fast T4 | 4.13512e-10 | **Fail, x4** | Pass |
| Current native fast T4 | 1.16227e-16 | Pass | Pass |
| Current native low T4 | 1.14754e-12 | Pass | Pass |
| Original default fixest | 1.31846e-13 | Pass | Pass |
| Original strict fixest | 6.38379e-16 | Pass | Pass |
| Original ppmlhdfe | 4.57967e-16 | Pass | Pass |
| Original pyfixest | 1.26219e-14 | Pass | Pass |

Other checks comprise point covariance and standard errors, objective, exact
sample/count, exact model rank and reported convergence. The strict original
fixest export uses `glm.tol=1e-10`, `fixef.tol=1e-11`. Original gglm remains
**incomparable**: N61440 versus60910 and no sample, convergence or objective
evidence. It receives no numerical acceptance or timing rank here.

Current source is commit **8a645481**, computational snapshot
`f2e37c75932a30d857efe3192394bac305decca68845412db186a6b24044f231`, Linux
plugin `7c01f4514454c1fb7f9804abb08ea3917ac98a67ef65d69ed9c3ea4083e792ec`.
Prior-current plugin is
`a7e95e6712c6b05b0217a9ddffa5b8f14c0f8d95443c2b983ce459f16a0e4596`.
All 211 computational inputs are checked before the diagnostic runs.

Fast current rejects the iteration-four strict checkpoint, resumes and accepts
iteration six: 129→186 reported inner iterations versus prior-current. Low
accepts at four outer/158 inner iterations. Fast modeled memory and tracked
peak are unchanged versus prior-current (297,626,267 and112,815,233 bytes).
Low uses a different memory route: modeled145,931,338, tracked112,576,062,
scratch4,872,800 bytes. These are diagnostic fit observations, not physical
memory qualification. Clocks include diagnostic tracing and shared-node execution;
**do not rank them or compare them with historical timings**.

## Attempts and receipts

- **7763438:** failed=0, exit_status=1, four slots, 50s, maxvmem950.410M.
  All small tests, sample witness and optimum enclosure completed. The harness
  then erroneously tried to reread `sample.csv`, which the standard runner hashes,
  counts and deletes. No new native fit ran. Partial outputs and failure receipt
  are preserved in [attempt01](attempt01/receipt.json).
- **7763447:** one narrowly corrected attempt; failed=0, exit_status=0, four
  slots, 43s, maxvmem1.286G. It uses the source-bound runner's sample hash/count
  against the independently derived IDs. All intended outputs and both completion
  sentinels exist; candidate science passes, baseline failure remains visible.
  [Receipt](attempt02/receipt.json), [comparison](attempt02/comparison.json),
  [sample witness](attempt02/sample-witness.json),
  [optimum certificate](attempt02/optimum-certificate.json).

Frozen harnesses, registrations, deployment digests and submissions retain both
attempts. Collected UTF-8 files match remotely computed SHA256 values. Raw licensed
logs, scratch-path requests and the binary anchor stay remote; their relevant
hashes are recorded. No production compilation, new comparator campaign, push
or broader qualification was performed.

Together with the prior positive-input adjudication, the three original strict
PPML coefficient discrepancies are now attributed to native premature stopping
and have current-source repair evidence. The separate fresh weak-d3 SE mismatch
remains unadjudicated. The accuracy repair's measured speed cost remains in the
[preceding cost screen](../ppml-coefficient-stop-scc-2026-09-28/fresh03/README.md).
Repeated strict recovery/finalization is the next performance target; this
diagnostic adds no new speed claim and does not close the broader repair plan.
