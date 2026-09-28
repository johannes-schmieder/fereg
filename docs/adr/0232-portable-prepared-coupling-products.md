# ADR 0232: portable prepared coupling products

Status: accepted for bounded development, 2026-09-27.

## Decision

Keep the existing coefficient Schur solver, topology, current-weight geometry,
column stopping and memory ownership. Change only multiplication/accumulation
inside applications of its optional prepared dense or compact coupling operator.
Targets that expose hardware FMA at compile time retain fused arithmetic.
Portable targets use inline multiplication followed by addition. There is no
runtime CPU dispatch, unsafe code, new solver or allocation.

The [Linux CPU profile](../../benchmarks/reports/development/ols-slope-owner-local-2026-09-27/linux-perf05/README.md)
found 41.5--57.1% of sampled cycles in prepared coupling application, plus
9.5--14.4% in out-of-line libm FMA. The latter calls execute a hardware FMA;
they are not software emulation. Call overhead and register spills are the
specific hypothesis. Sampled CPU proportions do not establish a wall-time gain.

This revises the scalar arithmetic implementation in ADRs 0126 and 0185.
It does not reopen the rejected paired-RHS storage or recurrence of ADR 0181.
The existing operator reaches admitted OLS two-partition slopes, PPML slopes
and certain low-memory higher-dimensional categorical PPML directions. Tests
and performance controls must cover that shared scope.

## Scientific boundary

Numerical QR geometry, its measured Gram and coupling construction, fresh
original-observation products, curvature dot products and cancellation guards
retain their previous arithmetic. Only repeated speculative applications change
on portable targets. The compact cooperative product shares those same helpers
and retains its deterministic worker merge order.

The curvature guard remains a routing diagnostic, not an accuracy certificate.
Rejected curvature still invokes the original-observation operator and squared
norm. Iterative residuals only schedule reconstruction. Every accepted solve
must reconstruct from the original RHS and pass the unchanged full current-weight
certificate. Finite scores, rank, model space, objective, strict reconstruction,
final Hessian, covariance, sample, DoF and transactional publication keep their
existing independent authority. Persistent numerical failure returns no result.

Unfused products need not reproduce the old recurrence bitwise. Independent
analytic projection checks cover changing and extreme weight scales, deficient
and duplicate slopes, disconnected components, dense/compact representations,
multiple RHS widths and threads. Additional checks construct a weighted constant
in the eliminated intercept's null space, verify that fact with the independent
observation operator, and require prepared-product refusal for null/near-null,
zero and nonfinite directions. Licensed explicit-design WLS and Poisson/Hessian/
sandwich oracles remain required on the actually changed portable target.

## Admission

No additional vector, factor, descriptor or persistent state is introduced.
Generation invalidation, budget refusal, allocation failure and fallback
ownership do not change. Acceptance requires source-bound Linux science and
alternating complete-command measurements; passing on ARM alone does not test
the changed arithmetic branch. Existing allocation-inventory drift and unrelated
constrained-fit/CG failures remain open.

## Evidence and limits

[Linux 7762056](../../benchmarks/reports/development/portable-coupling-scc-2026-09-28/linux-validation01.json)
passes 79 focused Rust tests on the changed portable branch, shared smoke and
four independent licensed fixtures. Mac universal build, ABI/panic checks and
four licensed fixtures also pass. The initial 24-fit block and
[fresh 100-fit OLS block](../../benchmarks/reports/development/portable-coupling-scc-2026-09-28/fresh02/README.md)
pass every registered scientific comparison. Six fresh new-path OLS paired
medians improve 9.3–22.9%; the low-memory T8 case still takes 1.277 times stock.
The larger initial baseline was variable; its apparent 60% gain is not a stable
claim. The excluded three-FE low route has broad control distributions, including
an initial 11.1% loss; no causal gain is assigned to that unchanged route.

[PPML 7762256](../../benchmarks/reports/development/portable-coupling-scc-2026-09-28/ppml03/README.md)
passes 45 applications and 30 scientific pairs. Four slope paired medians
improve 12.4–34.1%, with all four below 0.90 of strict stock on these particular
many-group panels. The larger fast case has a worse marginal median despite
its better paired median; three repetitions do not establish a stable gain.
The categorical control remains 1.233 times stock. Robust inference is matched;
optional saved means are outside this block. These ratios cannot be transferred
to the earlier small-group unbalanced or saved-mean cases.

The source is c6c1df25, frozen computational snapshot e07c7ec3…, Linux plugin
e9d9f706…. The change adds no numerical memory. Fresh T8/T4 OLS scaling remains
a concern requiring nine alternating pairs. Acceptance retains the useful
portable kernel; it does not assert universal nonregression, the requested
1.20 stock ceiling, or completion of the larger performance plan.
