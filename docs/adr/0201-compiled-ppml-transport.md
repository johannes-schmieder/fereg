# ADR 0201: compiled bounded PPML result transport

- Status: accepted for local development; not release qualification
- Date: 2026-09-25
- Scope: Stata PPML validation and scalar publication
- Evidence: [source-bound report](../../benchmarks/reports/development/ppml-compiled-transport-local-2026-09-25/README.md)

## Decision

Evaluate the existing bounded PPML result predicates in compiled Mata functions
and publish the same e() scalars through Mata. Keep these operations in the Stata
control plane. The estimator, scientific certificates, observation-sized result
checks, output stripes, prior-estimate hold, error handling and final protected
publication stay at their existing boundaries. There remains one substantive
native call. No estimation or observation/design construction moves into Mata.

`_fereg_ppml_result_mata.ado` is the sole maintained helper source. Its validator
loads sixteen bounded result matrices into local variables once, checks exact
layouts before indexing, then evaluates the existing predicates in their existing
order. It returns the same error codes and coefficient count. The independent
transport check remains independent of the native producer. The scalar publisher
preserves names, values and missing/absent distinctions after `ereturn post`,
inside the existing rollback transaction. Neither function retains result state
or relies on a previous fit. `mata clear` allows normal automatic object reload.

Ship three package-owned `.mo` objects compiled by **Stata 18**, the oldest
supported compiler. The source remains reviewable; an exact source/object hash
receipt and staging check reject stale or mismatched assets. Rebuild with
`scripts/build_ppml_mata.py`. Compiler release 18 is checked both inside the
fixture and in the invocation result; a source `version 18.0` statement in a newer
compiler does not establish readable older object code. The build requires a
successful application status and terminal sentinel, replaces objects, and writes
the receipt last. An interrupted inconsistent generation fails staging. Local
rebuilds reproduced identical bytes. Objects were executed under Stata 18 and 19
on macOS; this is not Windows/Linux compatibility qualification.

The [Mata object documentation](https://www.stata.com/manuals/m-3matamosave.pdf)
describes automatic object loading. StataCorp's
[compiler compatibility explanation](https://www.stata.com/statalist/archive/2009-02/msg00649.html)
explains the older-compiler requirement. There is no runtime compilation fallback:
missing helpers fail inside the transaction and restore the preceding public state.

## Scientific and transactional authority

This changes how transport predicates execute, not which predicates apply or
when they run. Layout, wire identity, sample/count/rank/DoF consistency, finite
coefficient/covariance evidence, solver/runtime/separation evidence and COMPLETE
checks all remain. There is no certificate scheduling change. Current-weight
projection, independent model-space/score/objective/reconstruction, and exact
final q*mu Hessian acceptance remain native and unchanged.

The Stata fixture retains malformed-runtime checks and adds automatic reload.
A separately captured old validator agrees on 3,496 bounded cell mutations,
including extended missing and extreme values. Separate scalar-publication
fixtures compare all names and values for full, missing and signed tables. These
are focused regression evidence, not an exhaustive proof of all input states.
Independent Poisson/Hessian/weight/separation fixtures and fresh public-state
audits pass. Missing-validator and missing-publisher tests preserve prior e(b),
e(V), e(sample), data signature and output absence; restoring the missing object
makes the identical fit succeed. Licensed interruption/rollback also passes.

## Performance, memory and limits

Retain final artifact **controlplane3**, with 564 captured inputs and the unchanged
slopetopology1 native plugin. Fresh seed434 has 35 paired warm wins, four losses
and one tie; all eight medians improve 1–5 ms. The first six cases win every pair.
This is useful shared overhead removal, not completion of the multi-FE target.
Narrow and disconnected low categorical fits still need 11.0 and 21.7 ms to reach
0.90 times comparable stock; T8 low slopes need 2.9 ms. T8 fast slopes cross only
at the median and remain provisional.

Reject runtime source compilation: first-fit medians gained approximately
183–187 ms of overhead. Precompilation removes that penalty but final cold
medians still lose two ms, and one candidate outlier remains visible. Instrumented
cold clones show large pre-plugin first-use delays in both arms; this does not
establish the cause of the uninstrumented outlier. Final OLS controls lose one
and three ms on three-FE and slope cases despite unchanged called source/binary.
Keep those and the earlier OLS sensitivity unresolved. No overall non-regression
claim follows.

Objects total 129,124 file bytes; loaded code/runtime overhead is not measured.
Local matrix copies add bounded O(K^2) transport storage during validation and
are released on return. There is no observation-by-regressor bank or native
allocation change. Native tracked peak/scratch deltas are zero in captured pairs;
selected telemetry varies by at most 144 bytes. This does not measure Mata RSS or
repair the documented static-inventory drift and physical-cap gaps.

Retire the interpreted production predicate/posting copies. Preserve them only
as immutable experimental oracles. Continue with demonstrated native lifecycle
work; do not tune packaging thresholds or resume rejected recurrence experiments.
