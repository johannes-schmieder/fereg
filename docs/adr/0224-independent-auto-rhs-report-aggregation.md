# ADR 0224: Aggregate independently selected automatic RHS solvers

Date: 2026-09-26. Status: accepted development correction.

Prepared low-memory OLS solves the outcome and control blocks under the same
model and weight frame, but its automatic pilots depend on each block's RHS.
A certified MAP completion, direct CG solve, LSMR solve, and certified LSMR-to-CG
continuation may therefore coexist. The report merger previously allowed only
the last two forms to differ. It rejected the lighter SCC campaign's slope
warm-up after numerical absorption had succeeded.

Accept these automatic report combinations without changing their solves or
certificates. Missing preconditioner metadata means that block did not enter
LSMR. When both blocks have LSMR metadata, their requested and used
preconditioners must still agree. Partial metadata, inconsistent explicit solver
requests, and uncertified solves remain errors. Keep the existing explicit CMG
fallback rules and PPML canonical checkpoint normalization.

Aggregate deterministically: a certified CG continuation takes priority over
LSMR, which takes priority over direct CG, which takes priority over MAP.
Retain any LSMR preconditioner metadata; OR actual fallback flags rather than
inventing a fallback for independent pilot choices. Preserve existing maximum
certificate/iteration/workspace fields and sums of work/timing counters. This
priority keeps the aggregate valid for subsequent merges in any block order.
Error messages now include the incompatible solver/preconditioner tuples.

No projection, rank, stopping tolerance, weight, inference, allocation or
publication policy changes. This is not a repair of the separately deferred
nonretaining-CG span problem. The benchmark warm-up also exposes the actual
native error in its log, outside the timed command.

Validation and the original failed diagnostic are recorded in the
[development report](../../benchmarks/reports/development/ols-auto-report-merge-2026-09-26/README.md).
