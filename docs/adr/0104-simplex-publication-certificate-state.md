# ADR 0104: publish the actual simplex certificate state

**Status:** Accepted for local development
**Date:** 2026-09-22

## Decision

Apply ADR 0017's existing comprehensive-IR closure policy at the Stata result
validator. A simplex execution skipped for memory/unsupported scope, or
returned as uncertified, honestly contributes a false value to the method's
aggregate certificate flag. That does not invalidate a fit whose complete
requested separation chain has been certified by the core.

Validate the flag against the status mask. Only simplex may contain unresolved
status bits; exhausted iterations remain fatal. Require a requested, executed,
certified IR summary with a later terminal trace row and the final receipt's
generation. Retain all existing count, trace, generation, drop-reason, aggregate,
hash and final-estimation checks. The core's full ordered-trace validation and
same-generation replay seal remain authoritative: bounded aggregates alone
cannot establish every historical generation's closure.

No detector, memory cap, sample, weight, rank, score, Hessian or estimator
certificate is changed. Do not relabel a skipped simplex attempt as certified
and do not substitute IR when the caller requested simplex alone.

## Evidence

The [local report](../../benchmarks/reports/development/ppml-simplex-publication-local-2026-09-22/README.md)
captures the original mismatch: simplex status mask 16, certificate flag zero,
followed by certified IR on generation one. A 7,680-row warmup dropped 60
zero-only FE observations before reaching this valid terminal chain; Stata
incorrectly rejected it solely because the simplex flag was zero.

Two licensed fixtures pass: explicit-dummy/rank/inference checks across
unit/FW/PW and T1/T4, plus validator acceptance and tampering checks for
unresolved simplex states. Simplex-only still fails atomically with r(909).
The validator fixture's obsolete result-wire minor is corrected to the active
1.3 schema. Twenty-one focused existing Rust tests pass, including the rule
that an IR deletion cannot retroactively close a previous generation.

The original large case and one fresh bounded case now pass stock pairwise
checks. The fresh case also passes independent separation, Poisson and
covariance oracles. Positive-output exports remain identical. This repairs
publication compatibility with the accepted scientific contract; it makes no
performance improvement or qualification claim.
