# ADR 0164: close the two-FE Schur and coefficient-storage experiment

Status: rejected for production; development decision, 2026-09-24.

## Decision

Retain ADR 0163's accepted computation. Do not adopt the standalone multi-FE
coefficient-backed row view, dense two-FE checkpoint extension, optional low
two-FE LSMR admission, or accompanying connected-topology shortcut from this
experiment. Preserve their sources, tests and failures in the
[source-bound report](../../benchmarks/reports/development/ppml-two-fe-schur-storage-local-2026-09-24/README.md).
Do not tune guards on the consumed fresh seed202609376 or retain favorable
subcases without a materially different computational organization.

## Evidence

The coefficient view reconstructs identical values but repeatedly pays for
original-minus-FE arithmetic. Wide T8 moments improve, while row scans roughly
double; serial wide moments and disconnected work also lose. Smaller resident
storage alone does not justify replacing canonical scratch storage.

The dense two-FE extension uses the accepted centered Schur mathematics,
current-call weighted factors, independent column stopping, original-input
reconstruction and unchanged complete certificates. Frozen projection gains
are real, but whole-public gains are insufficient. Fresh wide low PPML improves
123 to 122 ms; unbalanced fast loses 73 to 78, unbalanced low loses 89 to 97,
and disconnected loses 78 to 81. Every unbalanced/disconnected pair loses.
The narrow development gain is not confirmed. Modeled wide low storage rises
by about 39 MB in fresh confirmation; optional refusal preserves the old route
but does not make the added headroom free.

The final candidate passes 198 public applications and public-state checks,
66 native and 66 direct stock comparisons, 68 distinct focused Rust tests and
seven licensed fixture invocations. Independent frozen projection and separate
Poisson/Hessian/sandwich oracles pass. Earlier integration/test failures remain
archived, including six publication-validator refusals on the preceding artifact.
Rejection is a performance/complexity decision, not a scientific discrepancy.

## Consequences

All 546 accepted computational inputs are restored; no numerical, routing,
memory or public-policy change follows. Candidate tests remain with their
captured source, rather than asserting rejected behavior in production.
Existing exact-source OLS evidence remains unchanged. This does not repair
allocation-inventory drift, constrained-fit failures or platform gates.

The next larger opportunity is shared joint reconstruction and canonical
product ownership. The joint engine already has reduced finite/FE blocks;
another generic wrapper is not a new algorithm. Compare elimination of repeated
passes through admitted projected panels with algebraic canonical products
from raw moments and certified FE coefficients. The latter needs cancellation,
rank, residual and independent-oracle safeguards before it can be accepted.
Neither candidate may substitute a recurrence or prior-weight result for fresh
current-weight authority. Any changed verification procedure needs its own
explicit proof, successor decision and adversarial tests.

The report gives per-family millisecond budgets: the largest current comparable
two-FE losses require roughly 25–40% whole-command reductions to reach 0.90
times stock. Preserve existing weak, low-three-FE and slope wins; measure complete
commands early, cap the candidate search, and reject kernel-only improvements.
The broad performance goal remains unfinished.
