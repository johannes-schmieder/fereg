# ADR 0108: Reuse OLS factor names for an unchanged sample

Status: accepted for local development, 2026-09-22.

## Decision

After the native OLS receipt passes the existing publication validation, reuse
the parser's canonical coefficient names when the receipt's candidate and
retained physical-row counts are equal. The retained sample is a subset of
that same candidate sample; equal counts prove that no observation was removed.
This applies equally to unrestricted and `if`/`in` candidate samples.

If the counts differ, ordinary categorical components continue to trigger
`fvexpand` on the final sample. Plain continuous terms retain their existing
name reuse. Coefficient-count agreement, omission-marker processing, complete
result validation and transaction rollback remain unchanged. There is no
new scientific policy, observation pass, cache, wire field or native allocation.
This decision applies only to OLS publication, not PPML sample generations.

## Evidence and limits

The [generated-surface report](../../benchmarks/reports/development/generated-surface-local-2026-09-22/README.md)
binds the unchanged native artifact and exact ado overlay. Separate diagnostic
timers attribute 48 ms to repeated final expansion in the wide interaction case.
Matched public-command pairs improve about 15.5%, with 12.1% on the predeclared
fresh case. Narrow gains are smaller; individual losses remain recorded.
Native scientific exports are byte-identical in all 36 paired comparisons.

Independent dummy-OLS and output oracles, candidate/sample boundaries,
unit/AW/FW/PW inference, categorical level rebasing and transaction fixtures
pass. A baseline ordinary-absorb failure when missing outcomes remove an entire
factor level is preserved separately; this change does not repair it. The fresh
three-FE stock coefficient/RSS discrepancy remains unranked. No native rebuild,
memory-policy change, constrained-memory claim or release qualification follows
from this metadata-only optimization.
